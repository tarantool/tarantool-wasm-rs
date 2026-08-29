use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};

use lazy_static::lazy_static;
use wasmtime::{Error, Trap};

#[derive(Clone, Copy, Debug, Default)]
pub struct TimingSnapshot {
    pub count: u64,
    pub total_ns: u64,
    pub max_ns: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CacheSnapshot {
    pub hits: u64,
    pub misses: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FuelSnapshot {
    pub executions: u64,
    pub consumed: u64,
}

#[derive(Debug)]
struct TimingMetric {
    count: AtomicU64,
    total_ns: AtomicU64,
    max_ns: AtomicU64,
}

impl TimingMetric {
    fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            total_ns: AtomicU64::new(0),
            max_ns: AtomicU64::new(0),
        }
    }

    fn observe(&self, duration: Duration) {
        let nanos = u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX);
        self.count.fetch_add(1, Ordering::Relaxed);
        self.total_ns.fetch_add(nanos, Ordering::Relaxed);
        self.max_ns.fetch_max(nanos, Ordering::Relaxed);
    }

    fn snapshot(&self) -> TimingSnapshot {
        TimingSnapshot {
            count: self.count.load(Ordering::Relaxed),
            total_ns: self.total_ns.load(Ordering::Relaxed),
            max_ns: self.max_ns.load(Ordering::Relaxed),
        }
    }
}

struct Metrics {
    compilation: TimingMetric,
    worker_start: TimingMetric,
    runtime_setup: TimingMetric,
    store_setup: TimingMetric,
    instantiation: TimingMetric,
    argument_lowering: TimingMetric,
    guest_execution: TimingMetric,
    result_lifting: TimingMetric,
    worker_wait: TimingMetric,
    tx_wait: TimingMetric,
    lua_callback: TimingMetric,
    component_cache_hits: AtomicU64,
    component_cache_misses: AtomicU64,
    export_cache_hits: AtomicU64,
    export_cache_misses: AtomicU64,
    fuel_executions: AtomicU64,
    fuel_consumed: AtomicU64,
    current_memory_bytes: AtomicU64,
    peak_memory_bytes: AtomicU64,
    traps: Mutex<BTreeMap<&'static str, u64>>,
    conversion_errors: Mutex<BTreeMap<&'static str, u64>>,
}

impl Metrics {
    fn new() -> Self {
        Self {
            compilation: TimingMetric::new(),
            worker_start: TimingMetric::new(),
            runtime_setup: TimingMetric::new(),
            store_setup: TimingMetric::new(),
            instantiation: TimingMetric::new(),
            argument_lowering: TimingMetric::new(),
            guest_execution: TimingMetric::new(),
            result_lifting: TimingMetric::new(),
            worker_wait: TimingMetric::new(),
            tx_wait: TimingMetric::new(),
            lua_callback: TimingMetric::new(),
            component_cache_hits: AtomicU64::new(0),
            component_cache_misses: AtomicU64::new(0),
            export_cache_hits: AtomicU64::new(0),
            export_cache_misses: AtomicU64::new(0),
            fuel_executions: AtomicU64::new(0),
            fuel_consumed: AtomicU64::new(0),
            current_memory_bytes: AtomicU64::new(0),
            peak_memory_bytes: AtomicU64::new(0),
            traps: Mutex::new(BTreeMap::new()),
            conversion_errors: Mutex::new(BTreeMap::new()),
        }
    }
}

lazy_static! {
    static ref METRICS: Metrics = Metrics::new();
}

pub(crate) fn observe_compilation(duration: Duration) {
    METRICS.compilation.observe(duration);
}

pub(crate) fn observe_worker_start(duration: Duration) {
    METRICS.worker_start.observe(duration);
}

pub(crate) fn observe_runtime_setup(duration: Duration) {
    METRICS.runtime_setup.observe(duration);
}

pub(crate) fn observe_store_setup(duration: Duration) {
    METRICS.store_setup.observe(duration);
}

pub(crate) fn observe_instantiation(duration: Duration) {
    METRICS.instantiation.observe(duration);
}

pub(crate) fn observe_argument_lowering(duration: Duration) {
    METRICS.argument_lowering.observe(duration);
}

pub(crate) fn observe_guest_execution(duration: Duration) {
    METRICS.guest_execution.observe(duration);
}

pub(crate) fn observe_result_lifting(duration: Duration) {
    METRICS.result_lifting.observe(duration);
}

pub(crate) fn observe_worker_wait(duration: Duration) {
    METRICS.worker_wait.observe(duration);
}

pub(crate) fn observe_tx_wait(duration: Duration) {
    METRICS.tx_wait.observe(duration);
}

pub(crate) fn observe_lua_callback(duration: Duration) {
    METRICS.lua_callback.observe(duration);
}

pub(crate) fn observe_component_cache_hit() {
    METRICS.component_cache_hits.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn observe_component_cache_miss() {
    METRICS
        .component_cache_misses
        .fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn observe_export_cache_hit() {
    METRICS.export_cache_hits.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn observe_export_cache_miss() {
    METRICS.export_cache_misses.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn observe_fuel(before: Option<u64>, after: Option<u64>) {
    if let (Some(before), Some(after)) = (before, after) {
        METRICS.fuel_executions.fetch_add(1, Ordering::Relaxed);
        METRICS
            .fuel_consumed
            .fetch_add(before.saturating_sub(after), Ordering::Relaxed);
    }
}

pub(crate) fn observe_conversion_error(kind: &'static str) {
    let mut errors = METRICS
        .conversion_errors
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    *errors.entry(kind).or_default() += 1;
}

pub(crate) fn observe_trap(error: &Error) {
    let kind = error
        .downcast_ref::<Trap>()
        .map(classify_trap)
        .unwrap_or("host");
    let mut traps = METRICS
        .traps
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    *traps.entry(kind).or_default() += 1;
}

fn classify_trap(trap: &Trap) -> &'static str {
    match trap {
        Trap::StackOverflow => "stack_overflow",
        Trap::MemoryOutOfBounds | Trap::StringOutOfBounds | Trap::ListOutOfBounds => {
            "memory_out_of_bounds"
        }
        Trap::HeapMisaligned => "heap_misaligned",
        Trap::TableOutOfBounds | Trap::IndirectCallToNull | Trap::NullReference => {
            "invalid_reference"
        }
        Trap::BadSignature => "bad_signature",
        Trap::IntegerOverflow | Trap::IntegerDivisionByZero | Trap::BadConversionToInteger => {
            "numeric"
        }
        Trap::UnreachableCodeReached => "unreachable",
        Trap::Interrupt => "interrupt",
        Trap::OutOfFuel => "out_of_fuel",
        Trap::CannotEnterComponent | Trap::CannotLeaveComponent => "component_reentrancy",
        Trap::AsyncDeadlock => "async_deadlock",
        _ => "other",
    }
}

pub(crate) fn add_memory_bytes(bytes: usize) {
    let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
    let current = METRICS
        .current_memory_bytes
        .fetch_add(bytes, Ordering::Relaxed)
        .saturating_add(bytes);
    METRICS
        .peak_memory_bytes
        .fetch_max(current, Ordering::Relaxed);
}

pub(crate) fn remove_memory_bytes(bytes: usize) {
    let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
    let mut current = METRICS.current_memory_bytes.load(Ordering::Relaxed);
    loop {
        let updated = current.saturating_sub(bytes);
        match METRICS.current_memory_bytes.compare_exchange_weak(
            current,
            updated,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(actual) => current = actual,
        }
    }
}

#[derive(Debug)]
pub(crate) struct MetricsSnapshot {
    pub compilation: TimingSnapshot,
    pub worker_start: TimingSnapshot,
    pub runtime_setup: TimingSnapshot,
    pub store_setup: TimingSnapshot,
    pub instantiation: TimingSnapshot,
    pub argument_lowering: TimingSnapshot,
    pub guest_execution: TimingSnapshot,
    pub result_lifting: TimingSnapshot,
    pub worker_wait: TimingSnapshot,
    pub tx_wait: TimingSnapshot,
    pub lua_callback: TimingSnapshot,
    pub component_cache: CacheSnapshot,
    pub export_cache: CacheSnapshot,
    pub fuel: FuelSnapshot,
    pub current_memory_bytes: u64,
    pub peak_memory_bytes: u64,
    pub traps: BTreeMap<&'static str, u64>,
    pub conversion_errors: BTreeMap<&'static str, u64>,
}

pub(crate) fn snapshot() -> MetricsSnapshot {
    MetricsSnapshot {
        compilation: METRICS.compilation.snapshot(),
        worker_start: METRICS.worker_start.snapshot(),
        runtime_setup: METRICS.runtime_setup.snapshot(),
        store_setup: METRICS.store_setup.snapshot(),
        instantiation: METRICS.instantiation.snapshot(),
        argument_lowering: METRICS.argument_lowering.snapshot(),
        guest_execution: METRICS.guest_execution.snapshot(),
        result_lifting: METRICS.result_lifting.snapshot(),
        worker_wait: METRICS.worker_wait.snapshot(),
        tx_wait: METRICS.tx_wait.snapshot(),
        lua_callback: METRICS.lua_callback.snapshot(),
        component_cache: CacheSnapshot {
            hits: METRICS.component_cache_hits.load(Ordering::Relaxed),
            misses: METRICS.component_cache_misses.load(Ordering::Relaxed),
        },
        export_cache: CacheSnapshot {
            hits: METRICS.export_cache_hits.load(Ordering::Relaxed),
            misses: METRICS.export_cache_misses.load(Ordering::Relaxed),
        },
        fuel: FuelSnapshot {
            executions: METRICS.fuel_executions.load(Ordering::Relaxed),
            consumed: METRICS.fuel_consumed.load(Ordering::Relaxed),
        },
        current_memory_bytes: METRICS.current_memory_bytes.load(Ordering::Relaxed),
        peak_memory_bytes: METRICS.peak_memory_bytes.load(Ordering::Relaxed),
        traps: METRICS
            .traps
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone(),
        conversion_errors: METRICS
            .conversion_errors
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone(),
    }
}
