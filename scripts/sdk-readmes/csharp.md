# C# SDK

Package: `Tarantool.Wasm` NuGet package. Matching WIT: `../../wit/csharp/`.

Dependencies for the experimental component toolchain:

- .NET SDK 10.0.402 (the package targets `net10.0`).
- componentize-dotnet 0.8.0-preview00011.
- NativeAOT-LLVM 10.0.0-rc.1.26306.1.
- wit-bindgen 0.58.0, WASI SDK 29.0 and wasm-tools 1.261.0.
- Linux amd64; use the C# Docker toolchain on macOS arm64.
- `tarantool:tarantool@0.2.0`; the NuGet package supplies its component-type WIT.

Installation and build instructions:
[tarawasm](https://github.com/mandesero/tarawasm).
