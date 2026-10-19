using TarantoolSdkWorld.wit.Imports.tarantool.tarantool.v0_2_0;
using RawSpace = TarantoolSdkWorld.wit.Imports.tarantool.tarantool.v0_2_0.ITypesImports.Space;

namespace Tarantool.Wasm;

/// <summary>Borrowed catalog identity. Names are resolved on every operation, including after schema changes.</summary>
public sealed class Space
{
    private readonly string? name;
    private readonly uint id;

    internal Space(string name) { ArgumentException.ThrowIfNullOrWhiteSpace(name); this.name = name; }

    internal Space(uint id) => this.id = id;

    internal RawSpace Resolve() => name is null ? new(id) : HostResult.Get(IDatabaseImports.SpaceByName(name))
        ?? throw new KeyNotFoundException($"Tarantool space '{name}' does not exist.");

    public uint Id => Resolve().id;

    public Index Index(string name) => new(this, name);

    public Index Index(uint id) => new(this, id);

    public Index Primary => Index(0);

    public TarantoolTuple Insert(params object?[] fields)
    {
        ArgumentNullException.ThrowIfNull(fields);

        return TupleHandle.Take(HostResult.Get(IDatabaseImports.Insert(Resolve(), MessagePack.Pack(fields))));
    }

    public TarantoolTuple Replace(params object?[] fields)
    {
        ArgumentNullException.ThrowIfNull(fields);

        return TupleHandle.Take(HostResult.Get(IDatabaseImports.Replace(Resolve(), MessagePack.Pack(fields))));
    }

    public TarantoolTuple? Get(params object?[] key) => Primary.Get(key);

    public TarantoolTuple? Delete(params object?[] key) => Primary.Delete(key);

    public TarantoolTuple? Update(object?[] key, params UpdateOperation[] operations) => Primary.Update(key, operations);

    public void Upsert(object?[] fields, params UpdateOperation[] operations)
    {
        ArgumentNullException.ThrowIfNull(fields);

        HostResult.Get(IDatabaseImports.Upsert(Primary.Resolve(), MessagePack.Pack(fields), UpdateOperation.Pack(operations)));
    }

    public void Truncate() => HostResult.Get(IDatabaseImports.Truncate(Resolve()));

    public IEnumerable<TarantoolTuple> Scan(object?[]? key = null, IteratorType mode = IteratorType.All, int? limit = null, int offset = 0)
        => Primary.Scan(key, mode, limit, offset);
}
