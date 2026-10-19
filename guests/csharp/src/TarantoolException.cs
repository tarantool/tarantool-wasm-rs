using TarantoolSdkWorld;
using Raw = TarantoolSdkWorld.wit.Imports.tarantool.tarantool.v0_2_0.ITypesImports;

namespace Tarantool.Wasm;

/// <summary>An operation failure preserving the host's complete error record and open-ended numeric code.</summary>
public sealed class TarantoolException : Exception
{
    public uint Code { get; }

    public string ErrorType { get; }

    public string? SourceFile { get; }

    public uint? SourceLine { get; }

    internal TarantoolException(Raw.BoxError error) : base(error.message)
    {
        Code = error.code; ErrorType = error.errorType; SourceFile = error.file; SourceLine = error.line;
    }
}

internal static class HostResult
{
    internal static T Get<T>(Result<T, Raw.BoxError> result) => result.IsOk
        ? result.AsOk : throw new TarantoolException(result.AsErr);
}
