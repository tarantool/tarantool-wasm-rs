using Tarantool.Wasm;

namespace SecondaryWorld;

public partial class SecondaryWorldExportsImpl : ISecondaryWorldExports
{
    public static string Create(uint id, string name)
    {
        var users = Box.Space("csharp_users");
        users.Replace(id, name, 0);

        return users.Get(id)!.Get<string>(1);
    }
}
