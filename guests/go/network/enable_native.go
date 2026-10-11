//go:build !tinygo.wasm || !wasip2

package network

// Enable is only supported in TinyGo WASI Preview 2 components.
func Enable(Options) error { return ErrUnsupported }
