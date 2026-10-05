use wasmtime::{
    component::Linker, Cache, Config, Engine, InstanceAllocationStrategy, PoolingAllocationConfig,
};
use wasmtime_wasi_tls::p2::LinkOptions;

use crate::bindings::{
    docs::adder::{lua_callback, zlib},
    tarantool::tarantool::{
        box_tuple, database, error, index, key_def, log, metrics, msgpack, sequence, session,
        transaction, tuple_format, types,
    },
};

use super::SyncWasmStates;

const POOLING_ALLOCATOR_TOTAL_SLOTS: u32 = if cfg!(target_pointer_width = "64") {
    1_000
} else {
    100
};
const POOLING_ALLOCATOR_MAX_INSTANCE_SIZE: usize = 1 << 20;
const POOLING_ALLOCATOR_MAX_TABLES_PER_MODULE: u32 = 1;
const POOLING_ALLOCATOR_TABLE_ELEMENTS: usize = 20_000;
const POOLING_ALLOCATOR_MAX_MEMORIES_PER_MODULE: u32 = 1;

#[cfg(target_pointer_width = "64")]
const POOLING_ALLOCATOR_MAX_MEMORY_SIZE: usize = 1 << 32;
#[cfg(target_pointer_width = "32")]
const POOLING_ALLOCATOR_MAX_MEMORY_SIZE: usize = 10 << 20;

fn pooling_allocation_strategy() -> InstanceAllocationStrategy {
    let mut pooling = PoolingAllocationConfig::new();
    pooling
        .total_component_instances(POOLING_ALLOCATOR_TOTAL_SLOTS)
        .total_core_instances(POOLING_ALLOCATOR_TOTAL_SLOTS)
        .total_memories(POOLING_ALLOCATOR_TOTAL_SLOTS)
        .total_tables(POOLING_ALLOCATOR_TOTAL_SLOTS)
        .total_stacks(POOLING_ALLOCATOR_TOTAL_SLOTS)
        .total_gc_heaps(POOLING_ALLOCATOR_TOTAL_SLOTS)
        .max_component_instance_size(POOLING_ALLOCATOR_MAX_INSTANCE_SIZE)
        .max_core_instance_size(POOLING_ALLOCATOR_MAX_INSTANCE_SIZE)
        .max_tables_per_module(POOLING_ALLOCATOR_MAX_TABLES_PER_MODULE)
        .table_elements(POOLING_ALLOCATOR_TABLE_ELEMENTS)
        .max_memories_per_module(POOLING_ALLOCATOR_MAX_MEMORIES_PER_MODULE)
        .max_memory_size(POOLING_ALLOCATOR_MAX_MEMORY_SIZE);
    InstanceAllocationStrategy::Pooling(pooling)
}

pub(super) fn runtime_config(with_fuel: bool, cache: Cache) -> Config {
    let mut config = Config::new();
    config.cache(Some(cache));
    config.allocation_strategy(pooling_allocation_strategy());
    if with_fuel {
        config.consume_fuel(true);
    }

    /*
     * Epoch checks support pipe-local cancellation and global shutdown.
     * Every store has a callback which rearms on another pipe's tick, and
     * traps when shutdown starts or its own cancellation is requested.
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

    let _ = database::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = log::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = error::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = session::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ = msgpack::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
    let _ =
        transaction::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
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
    #[cfg(feature = "python-ssl-compat")]
    twr_python_ssl_host::add_to_linker(&mut linker)
        .expect("SSL BIO linker registration must succeed");
    linker
}
