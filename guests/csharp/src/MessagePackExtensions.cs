using System.Buffers.Binary;
using System.Text;

namespace Tarantool.Wasm;

public static partial class MessagePack
{
    private static void Extension(Stream output, sbyte type, byte[] payload)
    {
        switch (payload.Length)
        {
            case 1: output.WriteByte(0xd4); break;
            case 2: output.WriteByte(0xd5); break;
            case 4: output.WriteByte(0xd6); break;
            case 8: output.WriteByte(0xd7); break;
            case 16: output.WriteByte(0xd8); break;
            default:
                if (payload.Length <= byte.MaxValue) Number(output, 0xc7, (uint)payload.Length, 1);
                else if (payload.Length <= ushort.MaxValue) Number(output, 0xc8, (uint)payload.Length, 2);
                else Number(output, 0xc9, (uint)payload.Length, 4);
                break;
        }

        output.WriteByte(unchecked((byte)type)); output.Write(payload);
    }

    private static bool TryWriteExtension(Stream output, object value, int depth, HashSet<object> active)
    {
        switch (value)
        {
            case MessagePackExtension ext:
                ArgumentNullException.ThrowIfNull(ext.Data);
                Extension(output, ext.Type, ext.Data); return true;

            case Guid uuid:
                Extension(output, 2, uuid.ToByteArray(bigEndian: true)); return true;

            case TarantoolDecimal number:
                if (string.IsNullOrEmpty(number.Digits) || number.Digits.Length > 38
                    || number.Digits.Any(c => c < '0' || c > '9')) throw Invalid("decimal requires 1..38 digits");
                using (var payload = new MemoryStream())
                {
                    Signed(payload, number.Scale);
                    var digits = number.Digits + (number.Negative ? "d" : "c");
                    if ((digits.Length & 1) != 0) digits = "0" + digits;
                    for (var i = 0; i < digits.Length; i += 2)
                    {
                        var lo = digits[i + 1] switch { 'c' => 12, 'd' => 13, var c => c - '0' };
                        payload.WriteByte((byte)(((digits[i] - '0') << 4) | lo));
                    }
                    Extension(output, 1, payload.ToArray());
                }
                return true;

            case TarantoolDateTime datetime:
                if (datetime.Nanoseconds < 0 || datetime.Nanoseconds >= 1_000_000_000) throw Invalid("datetime nanoseconds out of range");
                var time = new byte[datetime.Nanoseconds == 0 && datetime.TzOffsetMinutes == 0 && datetime.TzIndex == 0 ? 8 : 16];
                BinaryPrimitives.WriteInt64LittleEndian(time, datetime.Seconds);
                if (time.Length == 16)
                {
                    BinaryPrimitives.WriteInt32LittleEndian(time.AsSpan(8), datetime.Nanoseconds);
                    BinaryPrimitives.WriteInt16LittleEndian(time.AsSpan(12), datetime.TzOffsetMinutes);
                    BinaryPrimitives.WriteInt16LittleEndian(time.AsSpan(14), datetime.TzIndex);
                }
                Extension(output, 4, time); return true;

            case TarantoolInterval interval:
                if ((int)interval.Adjust < 0 || (int)interval.Adjust > 2) throw Invalid("invalid interval adjust");
                var fields = new[] { interval.Years, interval.Months, interval.Weeks, interval.Days,
                    interval.Hours, interval.Minutes, interval.Seconds, interval.Nanoseconds, (long)interval.Adjust };
                using (var payload = new MemoryStream())
                {
                    payload.WriteByte((byte)fields.Count(n => n != 0));
                    for (var i = 0; i < fields.Length; ++i)
                        if (fields[i] != 0) { payload.WriteByte((byte)i); Signed(payload, fields[i]); }
                    Extension(output, 6, payload.ToArray());
                }
                return true;

            case TarantoolErrorValue error:
                if (error.Stack is null || error.Stack.Count == 0) throw Invalid("empty error stack");
                var attributes = new MessagePackMap();
                if (error.Extra is not null)
                    foreach (var pair in error.Extra)
                    {
                        if (pair.Key == 0) throw Invalid("reserved error attribute");
                        attributes.Add(pair.Key, pair.Value);
                    }
                var frames = new List<object?>();
                foreach (var frame in error.Stack)
                {
                    var map = new MessagePackMap {
                        { 0UL, frame.Type }, { 1UL, frame.File }, { 2UL, frame.Line }, { 3UL, frame.Message },
                        { 4UL, frame.Errno }, { 5UL, frame.Code },
                        { 6UL, frame.Fields is null ? new Dictionary<string, object?>() : new Dictionary<string, object?>(frame.Fields) }
                    };
                    if (frame.Extra is not null)
                        foreach (var pair in frame.Extra)
                        {
                            if (pair.Key < 7) throw Invalid("reserved error frame attribute");
                            map.Add(pair.Key, pair.Value);
                        }
                    frames.Add(map);
                }
                attributes.Add(0UL, frames);
                using (var payload = new MemoryStream())
                {
                    Write(payload, attributes, depth + 1, active);
                    Extension(output, 3, payload.ToArray());
                }
                return true;
            default: return false;
        }
    }

    private static long Integer(object? value) => value switch
    {
        long signed => signed,
        ulong unsigned when unsigned <= long.MaxValue => (long)unsigned,
        _ => throw Invalid("expected signed integer")
    };

    private static Dictionary<ulong, object?> NumericMap(object? value)
    {
        if (value is not MessagePackMap map) throw Invalid("expected numeric map");

        var result = new Dictionary<ulong, object?>();
        foreach (var pair in map)
        {
            var key = pair.Key switch
            {
                ulong unsigned => unsigned,
                long signed when signed >= 0 => (ulong)signed,
                _ => throw Invalid("invalid numeric map key")
            };
            if (!result.TryAdd(key, pair.Value)) throw Invalid("duplicate numeric map key");
        }

        return result;
    }

    private static object ReadExtension(sbyte type, ReadOnlySpan<byte> payload, int depth)
    {
        if (depth > MaxDepth) throw Invalid("nesting limit exceeded");

        switch (type)
        {
            case 2:
                if (payload.Length != 16) throw Invalid("UUID length");
                return new Guid(payload, bigEndian: true);

            case 1:
                var number = new Reader(payload);
                var scale = Integer(number.Read(depth + 1));
                if (scale < int.MinValue || scale > int.MaxValue) throw Invalid("decimal scale");
                var bcd = number.Take(number.Remaining);
                if (bcd.IsEmpty) throw Invalid("decimal digits");
                var sign = bcd[^1] & 15;
                if (sign < 10) throw Invalid("decimal sign");
                var digits = new StringBuilder();
                for (var i = 0; i < bcd.Length; ++i)
                {
                    var hi = bcd[i] >> 4; var lo = bcd[i] & 15;
                    if (hi > 9 || (i < bcd.Length - 1 && lo > 9)) throw Invalid("decimal digit");
                    digits.Append((char)('0' + hi));
                    if (i < bcd.Length - 1) digits.Append((char)('0' + lo));
                }
                var text = digits.ToString().TrimStart('0');
                if (text.Length == 0) text = "0";
                if (text.Length > 38) throw Invalid("decimal precision");
                return new TarantoolDecimal(text, (int)scale, sign == 11 || sign == 13);

            case 4:
                if (payload.Length != 8 && payload.Length != 16) throw Invalid("datetime length");
                var nanos = payload.Length == 16 ? BinaryPrimitives.ReadInt32LittleEndian(payload[8..]) : 0;
                if (nanos < 0 || nanos >= 1_000_000_000) throw Invalid("datetime nanoseconds");
                return new TarantoolDateTime(BinaryPrimitives.ReadInt64LittleEndian(payload), nanos,
                    payload.Length == 16 ? BinaryPrimitives.ReadInt16LittleEndian(payload[12..]) : (short)0,
                    payload.Length == 16 ? BinaryPrimitives.ReadInt16LittleEndian(payload[14..]) : (short)0);

            case 6:
                var interval = new Reader(payload);
                var count = interval.Byte();
                if (count > 9) throw Invalid("interval count");
                var values = new long[9]; var seen = new bool[9];
                for (var i = 0; i < count; ++i)
                {
                    var field = interval.Byte();
                    if (field >= 9 || seen[field]) throw Invalid("invalid or duplicate interval field");
                    seen[field] = true; values[field] = Integer(interval.Read(depth + 1));
                }
                if (interval.Remaining != 0 || values[8] < 0 || values[8] > 2) throw Invalid("interval payload");
                return new TarantoolInterval(values[0], values[1], values[2], values[3], values[4], values[5], values[6], values[7], (IntervalAdjust)values[8]);

            case 3:
                var reader = new Reader(payload);
                var attributes = NumericMap(reader.Read(depth + 1));
                if (reader.Remaining != 0) throw Invalid("trailing error bytes");
                if (!attributes.Remove(0, out var stack) || stack is not object?[] frames || frames.Length == 0) throw Invalid("missing error stack");
                var result = new List<TarantoolErrorFrame>();
                foreach (var frame in frames)
                {
                    var map = NumericMap(frame);
                    string Text(ulong key) => map.TryGetValue(key, out var value) && value is string s ? s : throw Invalid("missing error frame string");
                    uint UInt(ulong key)
                    {
                        if (!map.TryGetValue(key, out var value)) return 0;
                        var n = Integer(value);
                        return n >= 0 && n <= uint.MaxValue ? (uint)n : throw Invalid("error frame integer");
                    }
                    Dictionary<string, object?>? fields = null;
                    if (map.TryGetValue(6, out var fieldValue))
                        fields = fieldValue as Dictionary<string, object?> ?? throw Invalid("error fields map");
                    var extras = map.Where(pair => pair.Key >= 7).ToDictionary(pair => pair.Key, pair => pair.Value);
                    result.Add(new TarantoolErrorFrame(Text(0), Text(3), UInt(5), Text(1), UInt(2), UInt(4), fields, extras));
                }
                return new TarantoolErrorValue(result, attributes);
            default: return new MessagePackExtension(type, payload.ToArray());
        }
    }
}
