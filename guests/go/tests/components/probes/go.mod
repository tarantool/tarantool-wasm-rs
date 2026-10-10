module example/go-probes

go 1.27.0

require github.com/tarantool/tarantool-wasm-rs/guests/go v0.1.0

require (
	github.com/coreos/go-semver v0.3.1 // indirect
	github.com/docker/libtrust v0.0.0-20160708172513-aabc10ec26b7 // indirect
	github.com/klauspost/compress v1.18.0 // indirect
	github.com/opencontainers/go-digest v1.0.0 // indirect
	github.com/regclient/regclient v0.8.3 // indirect
	github.com/sirupsen/logrus v1.9.3 // indirect
	github.com/tetratelabs/wazero v1.9.0 // indirect
	github.com/ulikunitz/xz v0.5.12 // indirect
	github.com/urfave/cli/v3 v3.3.3 // indirect
	go.bytecodealliance.org v0.7.0 // indirect
	go.bytecodealliance.org/cm v0.3.0 // indirect
	golang.org/x/mod v0.24.0 // indirect
	golang.org/x/sys v0.33.0 // indirect
)

replace github.com/tarantool/tarantool-wasm-rs/guests/go => ../../..

tool go.bytecodealliance.org/cmd/wit-bindgen-go
