use super::*;

pub(super) type CallChain = Vec<ThreadId>;

lazy_static! {
    static ref ACTIVE_CALL_CHAINS: Mutex<HashMap<fiber::FiberId, CallChain>> =
        Mutex::new(HashMap::new());
}

pub(super) fn current_call_chain() -> CallChain {
    debug_assert_tx_thread();
    let fiber_id = fiber::id();
    let chain = ACTIVE_CALL_CHAINS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get(&fiber_id)
        .cloned()
        .unwrap_or_default();
    chain
}

pub(super) fn ensure_target_not_active(target: ThreadId, description: &str) -> Result<CallChain> {
    let chain = current_call_chain();
    if chain.contains(&target) {
        let mut cycle = chain.iter().map(|id| format!("{id:?}")).collect::<Vec<_>>();
        cycle.push(format!("{target:?}"));
        return Err(anyhow!(
            "reentrant WASM call rejected for {description}: {}",
            cycle.join(" -> ")
        ));
    }
    Ok(chain)
}

pub(crate) struct ActiveCallChainGuard {
    fiber_id: fiber::FiberId,
    previous: Option<CallChain>,
}

impl Drop for ActiveCallChainGuard {
    fn drop(&mut self) {
        let mut active = ACTIVE_CALL_CHAINS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(previous) = self.previous.take() {
            active.insert(self.fiber_id, previous);
        } else {
            active.remove(&self.fiber_id);
        }
    }
}

pub(crate) fn install_call_chain(chain: CallChain) -> ActiveCallChainGuard {
    debug_assert_tx_thread();
    let fiber_id = fiber::id();
    let previous = ACTIVE_CALL_CHAINS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .insert(fiber_id, chain);
    ActiveCallChainGuard { fiber_id, previous }
}
