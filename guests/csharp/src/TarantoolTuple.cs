using System.Collections;

namespace Tarantool.Wasm;

/// <summary>Detached tuple values, safe to retain after a host call or iterator is disposed.</summary>
public sealed class TarantoolTuple : IReadOnlyList<object?>
{
    private readonly object?[] fields;

    internal TarantoolTuple(byte[] data) => fields = MessagePack.Unpack(data) as object?[]
        ?? throw new FormatException("A Tarantool tuple must be a MessagePack array.");

    public int Count => fields.Length;

    public object? this[int index] => fields[index];

    public IEnumerator<object?> GetEnumerator() => ((IEnumerable<object?>)fields).GetEnumerator();
    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

    public object?[] ToArray() => (object?[])fields.Clone();

    /// <summary>Reads a field, allowing checked conversions between integer types without floating point.</summary>
    public T Get<T>(int index)
    {
        var value = fields[index];
        if (value is T typed) return typed;

        if (value is long or ulong)
        {
            checked
            {
                if (typeof(T) == typeof(long)) return (T)(object)(value is long s ? s : (long)(ulong)value);
                if (typeof(T) == typeof(ulong)) return (T)(object)(value is ulong u ? u : (ulong)(long)value);
                if (typeof(T) == typeof(int)) return (T)(object)(value is long s ? (int)s : (int)(ulong)value);
                if (typeof(T) == typeof(uint)) return (T)(object)(value is ulong u ? (uint)u : (uint)(long)value);
                if (typeof(T) == typeof(short)) return (T)(object)(value is long s ? (short)s : (short)(ulong)value);
                if (typeof(T) == typeof(ushort)) return (T)(object)(value is ulong u ? (ushort)u : (ushort)(long)value);
                if (typeof(T) == typeof(byte)) return (T)(object)(value is ulong u ? (byte)u : (byte)(long)value);
                if (typeof(T) == typeof(sbyte)) return (T)(object)(value is long s ? (sbyte)s : (sbyte)(ulong)value);
            }
        }

        throw new InvalidCastException($"Tuple field {index} cannot be read as {typeof(T).Name}.");
    }
}
