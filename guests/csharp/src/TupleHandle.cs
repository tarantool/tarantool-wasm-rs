using TarantoolSdkWorld.wit.Imports.tarantool.tarantool.v0_2_0;
using RawTuple = TarantoolSdkWorld.wit.Imports.tarantool.tarantool.v0_2_0.ITypesImports.BoxTuple;

namespace Tarantool.Wasm;

/// <summary>One owned host tuple reference. Dispose explicitly; no finalizer calls into the host.</summary>
public sealed class TupleHandle : IDisposable
{
    private readonly RawTuple tuple;
    private bool disposed;

    internal TupleHandle(RawTuple tuple) => this.tuple = tuple;

    private RawTuple Borrow()
    {
        ObjectDisposedException.ThrowIf(disposed, this);

        return tuple;
    }

    public uint FieldCount => IBoxTupleImports.FieldCount(Borrow());

    public TarantoolTuple Read() => new(HostResult.Get(IBoxTupleImports.ToBytes(Borrow())));

    public TupleHandle Copy()
    {
        var borrowed = Borrow(); IBoxTupleImports.Retain(borrowed); return new(borrowed);
    }

    /// <summary>False means no such field; true with null means a MessagePack nil field.</summary>
    public bool TryGetField(uint index, out object? value)
    {
        var bytes = HostResult.Get(IBoxTupleImports.Field(Borrow(), index));
        value = bytes is null ? null : MessagePack.Unpack(bytes);

        return bytes is not null;
    }

    public void Dispose()
    {
        if (disposed) return;

        disposed = true;
        IBoxTupleImports.Release(tuple);
    }

    internal static TarantoolTuple Take(RawTuple tuple)
    {
        using var owned = new TupleHandle(tuple);

        return owned.Read();
    }

    internal static TarantoolTuple? Take(RawTuple? tuple) => tuple is null ? null : Take(tuple.Value);
}
