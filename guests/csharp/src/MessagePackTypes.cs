using System.Globalization;
using System.Numerics;

namespace Tarantool.Wasm;

/// <summary>A map retaining order, duplicate keys, nil keys and non-scalar keys.</summary>
public sealed class MessagePackMap : List<KeyValuePair<object?, object?>>
{
    public MessagePackMap() { }

    public MessagePackMap(IEnumerable<KeyValuePair<object?, object?>> items) : base(items) { }

    public void Add(object? key, object? value) => Add(new(key, value));
}

/// <summary>An extension not interpreted by this SDK. Type and payload are preserved.</summary>
public sealed record MessagePackExtension(sbyte Type, byte[] Data);

/// <summary>Exact BCD decimal: signed Digits * 10^-Scale, with up to 38 digits.</summary>
public readonly record struct TarantoolDecimal(string Digits, int Scale, bool Negative = false)
{
    public static TarantoolDecimal FromDecimal(decimal value)
    {
        var bits = decimal.GetBits(value);
        var coefficient = new BigInteger((uint)bits[0]) + (new BigInteger((uint)bits[1]) << 32)
            + (new BigInteger((uint)bits[2]) << 64);

        return new(coefficient.ToString(CultureInfo.InvariantCulture), (bits[3] >> 16) & 255, bits[3] < 0);
    }

    /// <summary>Converts exactly, throwing OverflowException when System.Decimal would round or overflow.</summary>
    public decimal ToDecimal()
    {
        var value = decimal.Parse(ToString(), NumberStyles.Float, CultureInfo.InvariantCulture);

        static (string Digits, long Scale) Normalize(TarantoolDecimal number)
        {
            var digits = number.Digits.TrimStart('0');
            if (digits.Length == 0) return ("0", 0);

            var trimmed = digits.TrimEnd('0');
            return (trimmed, (long)number.Scale - (digits.Length - trimmed.Length));
        }

        if (Normalize(this) != Normalize(FromDecimal(value)))
            throw new OverflowException("The Tarantool decimal cannot be represented exactly as System.Decimal.");

        return value;
    }

    public override string ToString() => $"{(Negative ? "-" : "")}{Digits}e{-((long)Scale)}";
}

/// <summary>Exact Tarantool datetime; no implicit conversion discards nanoseconds or timezone identity.</summary>
public readonly record struct TarantoolDateTime(long Seconds, int Nanoseconds = 0, short TzOffsetMinutes = 0, short TzIndex = 0);

public enum IntervalAdjust { Excess = 0, None = 1, Last = 2 }

public readonly record struct TarantoolInterval(long Years = 0, long Months = 0, long Weeks = 0,
    long Days = 0, long Hours = 0, long Minutes = 0, long Seconds = 0, long Nanoseconds = 0,
    IntervalAdjust Adjust = IntervalAdjust.Excess);

/// <summary>A frame of a MessagePack error value, distinct from an exception raised by a host operation.</summary>
public sealed record TarantoolErrorFrame(string Type, string Message, uint Code = 0, string File = "",
    uint Line = 0, uint Errno = 0, IReadOnlyDictionary<string, object?>? Fields = null,
    IReadOnlyDictionary<ulong, object?>? Extra = null);

public sealed record TarantoolErrorValue(IReadOnlyList<TarantoolErrorFrame> Stack,
    IReadOnlyDictionary<ulong, object?>? Extra = null);
