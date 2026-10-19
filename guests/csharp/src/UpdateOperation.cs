namespace Tarantool.Wasm;

/// <summary>Zero-based tuple update operations. Negative field numbers keep Tarantool semantics.</summary>
public sealed class UpdateOperation
{
    private readonly string op;
    private readonly int field;
    private readonly object?[] args;

    private UpdateOperation(string op, int field, params object?[] args) { this.op = op; this.field = field; this.args = args; }

    public static UpdateOperation Set(int field, object? value) => new("=", field, value);

    public static UpdateOperation Add(int field, object value) => new("+", field, value);

    public static UpdateOperation Subtract(int field, object value) => new("-", field, value);

    public static UpdateOperation Insert(int field, object? value) => new("!", field, value);

    public static UpdateOperation Delete(int field, uint count = 1) => new("#", field, count);

    internal object?[] Values() => new object?[] { op, field }.Concat(args).ToArray();

    internal static byte[] Pack(UpdateOperation[] operations)
    {
        ArgumentNullException.ThrowIfNull(operations);

        return MessagePack.Pack(operations.Select(operation => operation?.Values()
            ?? throw new ArgumentException("Update operations cannot contain null.", nameof(operations))).ToArray());
    }
}
