use crate::bindings::docs::adder::txn;
use crate::bindings::docs::adder::types::{BoxError, TxnIsolationLevel};
use crate::tarantool_wasm::SyncWasmStates;
use crate::ffi as ffi;
use crate::bindings::error::handle_tarantool_error;

use wasmtime_wasi::async_trait;
use tarantool::lua_state;
use tarantool::tlua::LuaError;

impl std::str::FromStr for TxnIsolationLevel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "best-effort" => Ok(TxnIsolationLevel::TxnIsolationBestEffort),
            "read-committed" => Ok(TxnIsolationLevel::TxnIsolationReadCommitted),
            "read-confirmed" => Ok(TxnIsolationLevel::TxnIsolationReadConfirmed),
            "linearizable" => Ok(TxnIsolationLevel::TxnIsolationLinearizable),
            _ => Err(()),
        }
    }
}

#[async_trait]
impl txn::Host for SyncWasmStates {
    async fn txn_id(&mut self) -> i64 {
        let lua = lua_state();
        let res: Result<Option<i64>, _> = lua.eval("return box.txn_id()");
        match res {
            Ok(Some(txn_id)) => txn_id,
            Ok(None) | Err(_) => -1,
        }
    }

    async fn txn_isolation(&mut self) -> TxnIsolationLevel {
        let lua = lua_state();
    
        let isolation: Result<String, LuaError> = lua.eval("return require('config'):get('database.txn_isolation')");
    
        match isolation {
            Ok(level) => level.parse::<TxnIsolationLevel>().unwrap_or(TxnIsolationLevel::TxnIsolationBestEffort),
            Err(e) => {
                eprintln!("Failed to get txn_isolation: {:?}", e);
                TxnIsolationLevel::TxnIsolationBestEffort
            }
        }
    }

    async fn txn(&mut self) -> bool {
        return true;
    }

    async fn begin(&mut self) -> Result<(), BoxError> {
        let rt = unsafe { ffi::tnt::box_txn_begin() };
        if rt != 0 {
            let err = unsafe { handle_tarantool_error() };
            Err(err)
        } else {
            Ok(())
        }
    }

    async fn commit(&mut self) -> Result<(), BoxError> {
        let rt = unsafe { ffi::tnt::box_txn_commit() };
        if rt != 0 {
            let err = unsafe { handle_tarantool_error() };
            Err(err)
        } else {
            Ok(())
        }
    }

    async fn rollback(&mut self) -> Result<(), BoxError> {
        let rt = unsafe { ffi::tnt::box_txn_rollback() };
        if rt != 0 {
            let err = unsafe { handle_tarantool_error() };
            Err(err)
        } else {
            Ok(())
        }
    }

    // async fn txn_set_isolation(&mut self, isolation: TxnIsolationLevel) -> Result<(), BoxError> {

    // }


    // async fn txn_make_sync(&mut self) {

    // }
    
}