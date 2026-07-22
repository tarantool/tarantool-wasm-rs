use wasmtime::{component::Linker, Cache, Config, Engine};
use wasmtime_wasi_tls::p2::LinkOptions;

use crate::bindings::{
    docs::adder::{lua_callback, zlib},
    tarantool::tarantool::{
        box_tuple, error, index, key_def, metrics, msgpack, say, sequence, session, ttbox,
        tuple_format, txn, types,
    },
};

use super::SyncWasmStates;

pub(super) fn runtime_config(with_fuel: bool, cache: Cache) -> Config {
    let mut config = Config::new();
    config.cache(Some(cache));
    if with_fuel {
        config.consume_fuel(true);
    }

    /*
     * We use epoch-based interruptions to implement graceful shutdown.
     * Epoch is a global tick counter managed by the engine. We can setup
     * a limiting epoch for each module, after which it will halt. With
     * that technique we can set the limit at 1 tick for all modules and
     * then halt all modules at once by incrementing the epoch.
     */
    config.epoch_interruption(true);
    config
}

pub(super) fn build_runtime_linker(engine: &Engine) -> Linker<SyncWasmStates> {
    let mut linker = Linker::<SyncWasmStates>::new(engine);
    let _ = wasmtime_wasi::p2::add_to_linker_async(&mut linker);
    let _ = wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker);

    let mut opts = LinkOptions::default();
    opts.tls(true);

    let _ = wasmtime_wasi_tls::p2::add_to_linker(&mut linker, &opts);

    let _ = ttbox::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = say::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = error::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = session::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = msgpack::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = txn::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = types::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = index::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = sequence::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = key_def::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ =
        tuple_format::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = box_tuple::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = metrics::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ =
        lua_callback::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = zlib::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    linker
}
