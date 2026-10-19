# C# guest SDK for Tarantool-Wasm-Rs

`Tarantool.Wasm` is a small synchronous guest library for
`tarantool:tarantool@0.2.0`: spaces, indexes, CRUD, scans, owned tuples, host
errors and MessagePack with Tarantool extensions. It executes inside a Wasm
component hosted by Tarantool-Wasm-Rs. Schema creation remains the Lua host's
responsibility. This is not an IPROTO network client.

## Build the package

C# support is experimental and requires a Linux amd64 build environment.
Use a [tarawasm](https://github.com/mandesero/tarawasm) `csharp` Docker image
with the revision and toolchain versions in [sdk-manifest.json](sdk-manifest.json).
In that tarawasm checkout:

```sh
docker build --platform linux/amd64 --target csharp -t tarawasm:csharp-sdk .
```

Then, from this repository root:

```sh
export TWR_CSHARP_IMAGE=tarawasm:csharp-sdk
bash guests/csharp/build.sh
```

The local NuGet package is `guests/csharp/dist/Tarantool.Wasm.0.1.0.nupkg`.

## Consume from your own component

Initialize a C# project with tarawasm and your own WIT package, world and exports.
For example, `wit/app.wit`:

```wit
package example:users@0.1.0;
world app {
  export create-user: func(id: u32, name: string) -> string;
}
```

```sh
tarawasm init --lang csharp --wit wit --world app .
```

Add this reference to the generated `.csproj`:

```xml
<PackageReference Include="Tarantool.Wasm" Version="[0.1.0]" />
```

Add the package directory as a source in the project's `nuget.config`, keeping
the generated NuGet.org and dotnet-experimental feeds. For Docker, mount this
directory and use its container-visible absolute path. Then explicitly refresh
the lock once, inside the selected Linux C# build environment:

```sh
dotnet restore --use-lock-file --force-evaluate -p:RestoreLockedMode=false --configfile nuget.config
```

Ordinary `tarawasm bind` and `build` use the resulting lock without changing
dependencies. Replace the generated implementation with:

```csharp
using Tarantool.Wasm;

namespace AppWorld;

public partial class AppWorldExportsImpl : IAppWorldExports
{
    public static string CreateUser(uint id, string name)
    {
        var users = Box.Space("users");
        users.Insert(id, name);
        return users.Get(id)!.Get<string>(1);
    }
}
```

Build with `tarawasm bind` and `tarawasm build`. The package adds the required
Tarantool imports; your WIT only needs to declare the application's exports.

## Database values and resource ownership

Address spaces and indexes by name or numeric ID: `Box.Space("users")`,
`Box.Space(512u)`, `.Index("name")`, `.Index(0u)`. Names resolve on each operation,
so references survive schema replacement. Missing names throw
`KeyNotFoundException`.

`Insert` and `Replace` accept tuple fields with `params object?[]`. `Get` and
`Delete` accept key parts the same way, including composite keys. A missing row
returns null. CRUD results are detached `TarantoolTuple` values that need no
disposal. `Get<T>` performs checked integer conversions without losing precision.

```csharp
var users = Box.Space("users");
users.Update([42], UpdateOperation.Set(1, "Alice"));
users.Upsert([42, "Alice"], UpdateOperation.Set(1, "updated"));
foreach (var row in users.Index("name").Scan(["Alice"], IteratorType.Equal, limit: 10))
{
    Console.WriteLine(row.Get<string>(1));
    break;
}
```

Tuple fields and update positions are zero-based; negative update positions keep
Tarantool semantics. `Scan` supports offset and an optional limit; a zero limit
produces no rows. `Select` materializes a scan. `foreach`, including break or an
exception, releases the iterator. Manually obtained enumerators must be disposed
with `using`. The SDK is synchronous and is not thread-safe.

For explicit host tuple ownership:

```csharp
using var tuple = Box.NewTuple(42, null, "value");
bool present = tuple.TryGetField(1, out object? value); // true; value is null
using var copy = tuple.Copy(); // acquires another reference
```

Dispose owned tuples explicitly; repeated disposal is safe. Access after disposal
throws `ObjectDisposedException`.

Database errors raise `TarantoolException` with `Code`, `Message`, `ErrorType`,
`SourceFile` and `SourceLine` from the host.

## MessagePack and Tarantool extension types

`MessagePack.Pack`/`Unpack` support nil, booleans, integers, float/double,
UTF-8 strings, byte arrays, lists and dictionaries, and can run without a host.

| Value | C# representation |
| --- | --- |
| UUID, extension 2 | `Guid` in network byte order |
| Decimal, extension 1 | `TarantoolDecimal(Digits, Scale, Negative)`; exact 38-digit BCD |
| Datetime, extension 4 | `TarantoolDateTime(Seconds, Nanoseconds, TzOffsetMinutes, TzIndex)` |
| Interval, extension 6 | `TarantoolInterval`; signed fields and explicit `IntervalAdjust` |
| Error, extension 3 | `TarantoolErrorValue` with stack frames, fields and future numeric attributes |
| Unknown extension | `MessagePackExtension(Type, Data)` |

System.Decimal inputs are converted exactly; `TarantoolDecimal.ToDecimal()` is
an exact explicit conversion and throws if System.Decimal would round or overflow.
Datetime preserves nanoseconds and timezone index without an implicit lossy
DateTime conversion. Error *values* differ from exceptions raised by database
operations. Unknown extension payloads are copied unchanged.

Unsigned integers decode as `ulong`, signed encodings as `long`; full
64-bit limits are preserved. Positive signed inputs use compact unsigned wire
encodings. `byte[]` is binary, not an integer array. String-key maps with unique
keys decode as `Dictionary<string, object?>`; other maps use `MessagePackMap`,
retaining order, duplicate keys, nil keys and compound keys.

Invalid input, unsupported objects and cycles are rejected. Nesting is limited
to 128. Tarantool may reject extension values outside its supported ranges.

Transactions, sessions, callbacks, logging and network adapters are outside
this SDK.

## Run tests

With `TWR_CSHARP_IMAGE` set and the package built, run from the repository root:

```sh
bash guests/csharp/tests/build-components.sh
just build
bash guests/csharp/tests/test-runtime.sh
```

`build.sh` also runs the standalone codec tests. The [C# SDK workflow](../../.github/workflows/guests-csharp.yml)
runs package, component and Tarantool integration checks. Pull requests require
the `sdk-ci` or `full-ci` label; it can also be started manually.
