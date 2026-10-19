using Tarantool.Wasm;

var assertions = 0;

void Assert(bool condition, string message)
{
    ++assertions;
    if (!condition) throw new Exception(message);
}

void Reject(Action action, string message)
{
    var rejected = false;
    try { action(); }
    catch (Exception error) when (error is FormatException or ArgumentException or OverflowException) { rejected = true; }
    Assert(rejected, message);
}

void Golden(object? value, string expected)
{
    var bytes = MessagePack.Pack(value);
    Assert(Convert.ToHexString(bytes).ToLowerInvariant() == expected, "golden " + expected);
    Assert(MessagePack.Pack(MessagePack.Unpack(bytes)).SequenceEqual(bytes), "golden roundtrip " + expected);
}

// These bytes are shared with Go SDK fixtures and Lua's canonical MessagePack encoding.
Golden(null, "c0"); Golden(false, "c2"); Golden(true, "c3");
Golden(Array.Empty<byte>(), "c400"); Golden(Array.Empty<object?>(), "90");
Golden(new Dictionary<string, object?>(), "80");
Golden(ulong.MaxValue, "cfffffffffffffffff"); Golden(long.MinValue, "d38000000000000000");
Golden(long.MaxValue, "cf7fffffffffffffff");
Golden(new TarantoolDecimal("12345", 2, true), "d6010212345d");
Golden(Guid.Parse("00112233-4455-6677-8899-aabbccddeeff"), "d80200112233445566778899aabbccddeeff");
Golden(new TarantoolDateTime(-1, 999999999, -180, 42), "d804ffffffffffffffffffc99a3b4cff2a00");
Golden(new TarantoolInterval(Years: -2, Nanoseconds: 999, Adjust: IntervalAdjust.Last), "c709060300fe07cd03e70802");

foreach (var value in new ulong[] { 0, 127, 128, 255, 256, 65535, 65536, uint.MaxValue, 1UL << 53, ulong.MaxValue })
    Assert(MessagePack.Unpack(MessagePack.Pack(value)) is ulong actual && actual == value, "unsigned boundary");

foreach (var value in new long[] { -1, -32, -33, -128, -129, -32768, -32769, int.MinValue, long.MinValue })
    Assert(MessagePack.Unpack(MessagePack.Pack(value)) is long actual && actual == value, "signed boundary");

foreach (var value in new[] { "", "a\0b", "Привет 🙂", new string('Ж', 32768) })
    Assert((string?)MessagePack.Unpack(MessagePack.Pack(value)) == value, "UTF-8 string");

foreach (var value in new decimal[] { 0m, -1.2300m, decimal.MaxValue, decimal.MinValue })
    Assert(((TarantoolDecimal)MessagePack.Unpack(MessagePack.Pack(value))!).ToDecimal() == value, "System.Decimal exact");

Reject(() => new TarantoolDecimal("12345678901234567890123456789012345678", 37).ToDecimal(), "decimal narrowing must not round");
Reject(() => new TarantoolDecimal("1", 100).ToDecimal(), "decimal narrowing must not underflow");
Assert(new TarantoolDecimal("123000", 5).ToDecimal() == 1.23m, "decimal normalization");
Assert(new TarantoolDecimal("0", 100, true).ToDecimal() == 0m, "decimal zero scale");

var errorValue = new TarantoolErrorValue(new[] {
    new TarantoolErrorFrame("ClientError", "boom", uint.MaxValue, "guest.cs", 12, 5,
        new Dictionary<string, object?> { ["binary"] = new byte[] { 0, 255 }, ["nested"] = new TarantoolDecimal("123", 2) },
        new Dictionary<ulong, object?> { [8] = "future frame attribute" })
}, new Dictionary<ulong, object?> { [99] = "future error attribute" });

var values = new object[] {
    new TarantoolDecimal("12345678901234567890123456789012345678", -20, true),
    new TarantoolDecimal("0", 38, true), new TarantoolDateTime(long.MinValue),
    new TarantoolInterval(Days: -4, Months: 8, Adjust: IntervalAdjust.None),
    new MessagePackExtension(-7, new byte[] { 1, 2, 3 }), errorValue,
    new MessagePackMap { { null, "nil key" }, { new object?[] { 1 }, "array key" }, { "same", 1 }, { "same", 2 } }
};

foreach (var value in values)
{
    var bytes = MessagePack.Pack(value);
    var decoded = MessagePack.Unpack(bytes);
    Assert(MessagePack.Pack(decoded).SequenceEqual(bytes), "exact extension/map roundtrip");
}

var decodedError = (TarantoolErrorValue)MessagePack.Unpack(MessagePack.Pack(errorValue))!;

// Signed positive map keys are legal MessagePack, even though this SDK writes compact unsigned keys.
var signedErrorPayload = Convert.FromHexString("81d0009183d000a145d001a0d003a14d");
var signedError = (TarantoolErrorValue)MessagePack.Unpack(MessagePack.Pack(new MessagePackExtension(3, signedErrorPayload)))!;
Assert(signedError.Stack[0].Type == "E" && signedError.Stack[0].Message == "M", "signed positive error keys");
Assert(decodedError.Stack[0].Code == uint.MaxValue, "open error codes");
Assert((string?)decodedError.Extra![99] == "future error attribute", "future error attribute");
Assert((string?)decodedError.Stack[0].Extra![8] == "future frame attribute", "future frame attribute");
Assert(MessagePack.Unpack(MessagePack.Pack(new byte[] { 0, 255 })) is byte[], "binary differs from array");
Assert(MessagePack.Unpack(MessagePack.Pack(new object?[] { 0, 255 })) is object?[], "array differs from binary");

var cyclic = new object?[1]; cyclic[0] = cyclic;
Reject(() => MessagePack.Pack(cyclic), "cycle rejected");

var cycleMap = new Dictionary<string, object?>(); cycleMap["self"] = cycleMap;
Reject(() => MessagePack.Pack(cycleMap), "map cycle rejected");

object? nested = null; for (var i = 0; i < 140; ++i) nested = new object?[] { nested };
Reject(() => MessagePack.Pack(nested), "encode depth limit");

var deepBytes = Enumerable.Repeat((byte)0x91, 140).Append((byte)0xc0).ToArray();
Reject(() => MessagePack.Unpack(deepBytes), "decode depth limit");

foreach (var invalid in new object[] { new object(), "\ud800", new TarantoolDecimal("NaN", 0),
    new TarantoolDecimal(new string('1', 39), 0), new TarantoolDateTime(0, -1),
    new TarantoolDateTime(0, 1_000_000_000), new TarantoolInterval(Adjust: (IntervalAdjust)3),
    new TarantoolErrorValue(Array.Empty<TarantoolErrorFrame>()) })
    Reject(() => MessagePack.Pack(invalid), "invalid value rejected");

foreach (var hex in new[] { "", "c1", "dbffffffff", "9090", "c702020102", "c703060100c0", "d401ff", "a1ff", "c702060000", "c7040602000000", "c70503820090d00090" })
    Reject(() => MessagePack.Unpack(Convert.FromHexString(hex)), "invalid wire " + hex);

// Construction must be usable without a WASI host. No eager imports or host calls.
Box.Space("users").Index("primary"); Box.Space(512u).Index(0u);
Assert(true, "proxy construction is lazy");

Console.WriteLine($"C# SDK unit checks: {assertions} passed");
