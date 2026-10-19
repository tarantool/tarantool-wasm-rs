using Tarantool.Wasm;

namespace SdkTestWorld;

public partial class SdkTestWorldExportsImpl : ISdkTestWorldExports
{
    private static readonly Space Users = Box.Space("csharp_users");
    private static readonly Tarantool.Wasm.Index Names = Users.Index("name");

    private static void Assert(bool value) { if (!value) throw new Exception("C# SDK assertion failed"); }

    public static uint SpaceId() => Users.Id;

    public static uint IndexId() => Names.Id;

    public static byte[] Roundtrip(byte[] data) => MessagePack.Pack(MessagePack.Unpack(data));

    public static byte[] ErrorDetails()
    {
        try { Users.Insert(10, "duplicate", 0); throw new Exception("duplicate accepted"); }
        catch (TarantoolException error)
        {
            return MessagePack.Pack(new object?[] { error.Code, error.Message, error.ErrorType, error.SourceFile, error.SourceLine });
        }
    }

    public static string Check(string name)
    {
        switch (name)
        {
            case "lua-read":
                var luaRow = Users.Get(99)!;
                Assert(luaRow.Get<string>(1) == "Lua" && luaRow.Get<int>(2) == 1);
                break;

            case "crud":
                var created = Users.Insert(1, "Alice", 10);
                Assert(created.Get<int>(0) == 1 && created.Get<string>(1) == "Alice");
                Assert(Users.Get(999) is null && Users.Delete(999) is null);
                var updated = Users.Update([1], UpdateOperation.Set(1, "Алиса 🙂"), UpdateOperation.Add(2, 2));
                Assert(updated!.Get<string>(1) == "Алиса 🙂" && updated.Get<int>(2) == 12);
                Users.Upsert([1, "ignored", 0], UpdateOperation.Add(2, 5));
                Assert(Users.Get(1)!.Get<int>(2) == 17);
                Users.Upsert([2, "Bob", 20], UpdateOperation.Set(1, "unused"));
                Assert(Users.Get(2)!.Get<string>(1) == "Bob");
                Assert(Users.Replace(1, "replaced", 0).Get<string>(1) == "replaced");
                Assert(Users.Delete(1)!.Get<string>(1) == "replaced");
                Assert(Users.Get(1) is null);
                Assert(Users.Get(2)!.Get<int>(2) == 20);
                break;

            case "indexes":
                Users.Insert(1, "a", 1); Users.Insert(2, "b", 2); Users.Insert(3, "c", 3);
                Assert(Names.Get("b")!.Get<int>(0) == 2);
                Assert(Names.Count(["b"], IteratorType.Equal) == 1 && Names.Length == 3);
                Assert(Users.Primary.Select(limit: 1, offset: 1).Single().Get<int>(0) == 2);
                Assert(Users.Primary.Select(limit: 0).Count == 0);

                for (var i = 0; i < 40; ++i)
                {
                    foreach (var tuple in Users.Scan()) { Assert(tuple.Get<int>(0) == 1); break; }
                    try { foreach (var tuple in Users.Scan()) { throw new ApplicationException("stop"); } }
                    catch (ApplicationException) { }
                }

                Assert(Users.Scan().Count() == 3);
                Assert(Box.Space(Users.Id).Index(0u).Get(2)!.Get<string>(1) == "b");
                break;

            case "tuples":
                var tupleHandle = Box.NewTuple(1, null, "text", new byte[] { 0, 255 });
                var copy = tupleHandle.Copy();
                tupleHandle.Dispose(); tupleHandle.Dispose();
                try { _ = tupleHandle.FieldCount; throw new Exception("closed tuple accepted"); }
                catch (ObjectDisposedException) { }

                using (copy)
                {
                    Assert(copy.FieldCount == 4);
                    Assert(copy.TryGetField(1, out var nil) && nil is null);
                    Assert(!copy.TryGetField(9, out _));
                    Assert(copy.Read().Get<string>(2) == "text");
                }

                using (var limits = Box.NewTuple(ulong.MaxValue, long.MinValue))
                {
                    var values = limits.Read();
                    Assert(values.Get<ulong>(0) == ulong.MaxValue && values.Get<long>(1) == long.MinValue);
                    try { values.Get<long>(0); throw new Exception("overflow accepted"); }
                    catch (OverflowException) { }
                }
                break;

            case "errors":
                Users.Insert(1, "original", 0);
                try { Users.Insert(1, "duplicate", 0); throw new Exception("duplicate accepted"); }
                catch (TarantoolException error)
                {
                    Assert(error.Code == 3 && error.Message.Contains("csharp_users") && error.ErrorType.Length > 0);
                }
                Assert(Users.Get(1)!.Get<string>(1) == "original");
                try { Box.Space("csharp_missing").Get(1); throw new Exception("missing space accepted"); }
                catch (KeyNotFoundException) { }
                try { Users.Index("missing").Get(1); throw new Exception("missing index accepted"); }
                catch (KeyNotFoundException) { }
                break;

            case "extensions":
                var row = Box.Space("csharp_specials").Replace(1,
                    new TarantoolDecimal("12345678901234567890123456789012345678", 10),
                    Guid.Parse("00112233-4455-6677-8899-aabbccddeeff"),
                    new TarantoolDateTime(-1, 999999999, 180),
                    new TarantoolInterval(Years: 2, Days: -4, Adjust: IntervalAdjust.Last),
                    new TarantoolErrorValue(new[] { new TarantoolErrorFrame("ClientError", "C# error value", 42) }),
                    new byte[] { 0, 255 });
                Assert(row.Get<TarantoolDecimal>(1).Digits == "12345678901234567890123456789012345678");
                Assert(row.Get<TarantoolDateTime>(3).Nanoseconds == 999999999);
                Assert(row.Get<TarantoolErrorValue>(5).Stack[0].Code == 42);
                Assert(row.Get<Guid>(2).ToString() == "00112233-4455-6677-8899-aabbccddeeff");
                Box.Space("csharp_bigints").Replace(1, ulong.MaxValue, long.MinValue, 1UL << 53);
                break;

            case "invalid":
                try { MessagePack.Unpack(new byte[] { 0x90, 0x90 }); throw new Exception("trailing bytes accepted"); }
                catch (FormatException) { }
                try { Users.Primary.Select(limit: -1); throw new Exception("negative limit accepted"); }
                catch (ArgumentOutOfRangeException) { }
                break;

            default: throw new ArgumentException(name);
        }

        return "ok";
    }
}
