using TarantoolSdkWorld.wit.Imports.tarantool.tarantool.v0_2_0;

namespace Tarantool.Wasm;

/// <summary>Entry point for the synchronous guest database API.</summary>
public static class Box
{
    public static uint SchemaVersion => IDatabaseImports.SchemaVersion();

    public static Space Space(string name) => new(name);

    public static Space Space(uint id) => new(id);

    public static TupleHandle NewTuple(params object?[] fields)
    {
        ArgumentNullException.ThrowIfNull(fields);

        return new(HostResult.Get(IBoxTupleImports.New(MessagePack.Pack(fields))));
    }
}
