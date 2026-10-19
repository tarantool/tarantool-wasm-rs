using TarantoolSdkWorld.wit.Imports.tarantool.tarantool.v0_2_0;
using Raw = TarantoolSdkWorld.wit.Imports.tarantool.tarantool.v0_2_0.ITypesImports;

namespace Tarantool.Wasm;

public enum IteratorType
{
    Equal, ReverseEqual, All, LessThan, LessThanOrEqual, GreaterThanOrEqual, GreaterThan,
    BitsAllSet, BitsAnySet, BitsAllNotSet, Overlaps, Neighbor, NextPrefix, PreviousPrefix
}

public sealed class Index
{
    private readonly Space space;
    private readonly string? name;
    private readonly uint id;

    internal Index(Space space, string name) { ArgumentException.ThrowIfNullOrWhiteSpace(name); this.space = space; this.name = name; }

    internal Index(Space space, uint id) { this.space = space; this.id = id; }

    internal Raw.Index Resolve()
    {
        var resolved = space.Resolve();

        return name is null ? new(id, resolved.id) : HostResult.Get(IDatabaseImports.IndexByName(resolved, name))
            ?? throw new KeyNotFoundException($"Tarantool index '{name}' does not exist.");
    }

    public uint Id => Resolve().id;

    public ulong Length => HostResult.Get(IIndexImports.TupleCount(Resolve()));

    private static byte[] Key(object?[] key) { ArgumentNullException.ThrowIfNull(key); return MessagePack.Pack(key); }

    public TarantoolTuple? Get(params object?[] key) => TupleHandle.Take(HostResult.Get(IIndexImports.Get(Resolve(), Key(key))));

    public TarantoolTuple? Delete(params object?[] key) => TupleHandle.Take(HostResult.Get(IDatabaseImports.Delete(Resolve(), Key(key))));

    public TarantoolTuple? Update(object?[] key, params UpdateOperation[] operations) =>
        TupleHandle.Take(HostResult.Get(IDatabaseImports.Update(Resolve(), Key(key), UpdateOperation.Pack(operations))));

    public ulong Count(object?[]? key = null, IteratorType mode = IteratorType.All) =>
        HostResult.Get(IIndexImports.Count(Resolve(), Mode(mode), Key(key ?? [])));

    private static Raw.IteratorType Mode(IteratorType mode) => mode >= IteratorType.Equal && mode <= IteratorType.PreviousPrefix
        ? (Raw.IteratorType)(int)mode : throw new ArgumentOutOfRangeException(nameof(mode));

    /// <summary>Lazy synchronous scan. foreach/using disposes the host iterator on exhaustion, break or exception.</summary>
    public IEnumerable<TarantoolTuple> Scan(object?[]? key = null, IteratorType mode = IteratorType.All, int? limit = null, int offset = 0)
    {
        if (limit < 0) throw new ArgumentOutOfRangeException(nameof(limit));
        if (offset < 0) throw new ArgumentOutOfRangeException(nameof(offset));

        var rawMode = Mode(mode);
        if (limit == 0) yield break;

        using var iterator = HostResult.Get(IIndexImports.Iterator.New(Resolve(), rawMode, Key(key ?? [])));
        var skipped = 0; var returned = 0;

        while (limit is null || returned < limit)
        {
            var tuple = HostResult.Get(iterator.Next());
            if (tuple is null) yield break;

            var row = TupleHandle.Take(tuple.Value);
            if (skipped < offset) { ++skipped; continue; }

            ++returned;
            yield return row;
        }
    }

    public IReadOnlyList<TarantoolTuple> Select(object?[]? key = null, IteratorType mode = IteratorType.All, int? limit = null, int offset = 0)
        => Scan(key, mode, limit, offset).ToArray();
}
