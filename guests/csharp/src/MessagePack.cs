using System.Buffers.Binary;
using System.Collections;
using System.Text;

namespace Tarantool.Wasm;

/// <summary>Reflection-free MessagePack with Tarantool extensions. Invalid or trailing bytes are rejected.</summary>
public static partial class MessagePack
{
    private const int MaxDepth = 128;
    private static readonly UTF8Encoding Utf8 = new(false, true);

    public static byte[] Pack(object? value)
    {
        using var output = new MemoryStream();
        Write(output, value, 0, new HashSet<object>(ReferenceEqualityComparer.Instance));

        return output.ToArray();
    }

    public static object? Unpack(ReadOnlySpan<byte> bytes)
    {
        var reader = new Reader(bytes);
        var value = reader.Read(0);
        if (reader.Remaining != 0) throw Invalid("trailing bytes");

        return value;
    }

    private static FormatException Invalid(string message) => new("MessagePack: " + message);

    private static void Number(Stream output, byte marker, ulong value, int size)
    {
        output.WriteByte(marker);
        Span<byte> bytes = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64BigEndian(bytes, value);
        output.Write(bytes[(8 - size)..]);
    }

    private static void Unsigned(Stream output, ulong value)
    {
        if (value < 128) output.WriteByte((byte)value);
        else if (value <= byte.MaxValue) Number(output, 0xcc, value, 1);
        else if (value <= ushort.MaxValue) Number(output, 0xcd, value, 2);
        else if (value <= uint.MaxValue) Number(output, 0xce, value, 4);
        else Number(output, 0xcf, value, 8);
    }

    private static void Signed(Stream output, long value)
    {
        if (value >= 0) Unsigned(output, (ulong)value);
        else if (value >= -32) output.WriteByte(unchecked((byte)value));
        else if (value >= sbyte.MinValue) Number(output, 0xd0, unchecked((ulong)value), 1);
        else if (value >= short.MinValue) Number(output, 0xd1, unchecked((ulong)value), 2);
        else if (value >= int.MinValue) Number(output, 0xd2, unchecked((ulong)value), 4);
        else Number(output, 0xd3, unchecked((ulong)value), 8);
    }

    private static void ArrayHeader(Stream output, int size)
    {
        if (size < 16) output.WriteByte((byte)(0x90 | size));
        else if (size <= ushort.MaxValue) Number(output, 0xdc, (uint)size, 2);
        else Number(output, 0xdd, (uint)size, 4);
    }

    private static void MapHeader(Stream output, int size)
    {
        if (size < 16) output.WriteByte((byte)(0x80 | size));
        else if (size <= ushort.MaxValue) Number(output, 0xde, (uint)size, 2);
        else Number(output, 0xdf, (uint)size, 4);
    }

    private static void Write(Stream output, object? value, int depth, HashSet<object> active)
    {
        if (depth > MaxDepth) throw Invalid("nesting limit exceeded");

        switch (value)
        {
            case null: output.WriteByte(0xc0); return;
            case bool flag: output.WriteByte(flag ? (byte)0xc3 : (byte)0xc2); return;
            case byte n: Unsigned(output, n); return;
            case ushort n: Unsigned(output, n); return;
            case uint n: Unsigned(output, n); return;
            case ulong n: Unsigned(output, n); return;
            case sbyte n: Signed(output, n); return;
            case short n: Signed(output, n); return;
            case int n: Signed(output, n); return;
            case long n: Signed(output, n); return;
            case float n: Number(output, 0xca, BitConverter.SingleToUInt32Bits(n), 4); return;
            case double n: Number(output, 0xcb, BitConverter.DoubleToUInt64Bits(n), 8); return;

            case string text:
                var bytes = Utf8.GetBytes(text);
                if (bytes.Length < 32) output.WriteByte((byte)(0xa0 | bytes.Length));
                else if (bytes.Length <= byte.MaxValue) Number(output, 0xd9, (uint)bytes.Length, 1);
                else if (bytes.Length <= ushort.MaxValue) Number(output, 0xda, (uint)bytes.Length, 2);
                else Number(output, 0xdb, (uint)bytes.Length, 4);
                output.Write(bytes); return;

            case byte[] binary:
                if (binary.Length <= byte.MaxValue) Number(output, 0xc4, (uint)binary.Length, 1);
                else if (binary.Length <= ushort.MaxValue) Number(output, 0xc5, (uint)binary.Length, 2);
                else Number(output, 0xc6, (uint)binary.Length, 4);
                output.Write(binary); return;

            case decimal n: Write(output, TarantoolDecimal.FromDecimal(n), depth, active); return;
        }

        if (!active.Add(value)) throw Invalid("cyclic value");

        try
        {
            if (TryWriteExtension(output, value, depth, active)) return;

            if (value is MessagePackMap pairs)
            {
                MapHeader(output, pairs.Count);
                foreach (var pair in pairs) { Write(output, pair.Key, depth + 1, active); Write(output, pair.Value, depth + 1, active); }
            }
            else if (value is IDictionary map)
            {
                MapHeader(output, map.Count);
                foreach (DictionaryEntry pair in map) { Write(output, pair.Key, depth + 1, active); Write(output, pair.Value, depth + 1, active); }
            }
            else if (value is IReadOnlyList<object?> list)
            {
                ArrayHeader(output, list.Count);
                foreach (var item in list) Write(output, item, depth + 1, active);
            }
            else if (value is IList array)
            {
                ArrayHeader(output, array.Count);
                foreach (var item in array) Write(output, item, depth + 1, active);
            }
            else throw new ArgumentException($"Unsupported MessagePack value {value.GetType().FullName}.", nameof(value));
        }
        finally { active.Remove(value); }
    }

    private ref struct Reader
    {
        private readonly ReadOnlySpan<byte> bytes;
        private int position;

        public Reader(ReadOnlySpan<byte> bytes) { this.bytes = bytes; position = 0; }

        public int Remaining => bytes.Length - position;

        public ReadOnlySpan<byte> Take(int count)
        {
            if (count < 0 || count > Remaining) throw Invalid("truncated input");

            var result = bytes.Slice(position, count); position += count; return result;
        }

        public byte Byte() => Take(1)[0];

        public ulong Number(int count)
        {
            ulong value = 0;
            foreach (var item in Take(count)) value = (value << 8) | item;

            return value;
        }

        private int Length(int count)
        {
            var size = Number(count);
            if (size > (ulong)Remaining) throw Invalid("declared length exceeds input");

            return checked((int)size);
        }

        private object?[] Array(int size, int depth)
        {
            if (size > Remaining) throw Invalid("array exceeds input");

            var values = new object?[size];
            for (var i = 0; i < size; ++i) values[i] = Read(depth + 1);

            return values;
        }

        private object Map(int size, int depth)
        {
            if (size > Remaining / 2) throw Invalid("map exceeds input");

            var pairs = new MessagePackMap();
            var strings = new Dictionary<string, object?>();
            var stringKeys = true;

            for (var i = 0; i < size; ++i)
            {
                var key = Read(depth + 1); var value = Read(depth + 1);
                pairs.Add(key, value);
                if (key is not string text || !strings.TryAdd(text, value)) stringKeys = false;
            }

            return stringKeys ? strings : pairs;
        }

        private object Extension(int size, int depth)
        {
            var type = unchecked((sbyte)Byte());
            return ReadExtension(type, Take(size), depth);
        }

        public object? Read(int depth)
        {
            if (depth > MaxDepth) throw Invalid("nesting limit exceeded");

            var marker = Byte();
            if (marker < 0x80) return (ulong)marker;
            if (marker >= 0xe0) return (long)unchecked((sbyte)marker);
            if ((marker & 0xe0) == 0xa0) return Utf8.GetString(Take(marker & 31));
            if ((marker & 0xf0) == 0x90) return Array(marker & 15, depth);
            if ((marker & 0xf0) == 0x80) return Map(marker & 15, depth);

            return marker switch
            {
                0xc0 => null, 0xc2 => false, 0xc3 => true,
                0xc4 => Take(Length(1)).ToArray(), 0xc5 => Take(Length(2)).ToArray(), 0xc6 => Take(Length(4)).ToArray(),
                0xc7 => Extension(Length(1), depth), 0xc8 => Extension(Length(2), depth), 0xc9 => Extension(Length(4), depth),
                0xca => BitConverter.UInt32BitsToSingle((uint)Number(4)), 0xcb => BitConverter.UInt64BitsToDouble(Number(8)),
                0xcc => Number(1), 0xcd => Number(2), 0xce => Number(4), 0xcf => Number(8),
                0xd0 => (long)unchecked((sbyte)Number(1)), 0xd1 => (long)unchecked((short)Number(2)),
                0xd2 => (long)unchecked((int)Number(4)), 0xd3 => unchecked((long)Number(8)),
                0xd4 => Extension(1, depth), 0xd5 => Extension(2, depth), 0xd6 => Extension(4, depth),
                0xd7 => Extension(8, depth), 0xd8 => Extension(16, depth),
                0xd9 => Utf8.GetString(Take(Length(1))), 0xda => Utf8.GetString(Take(Length(2))), 0xdb => Utf8.GetString(Take(Length(4))),
                0xdc => Array(Length(2), depth), 0xdd => Array(Length(4), depth),
                0xde => Map(Length(2), depth), 0xdf => Map(Length(4), depth),
                _ => throw Invalid($"invalid marker 0x{marker:x2}")
            };
        }
    }
}
