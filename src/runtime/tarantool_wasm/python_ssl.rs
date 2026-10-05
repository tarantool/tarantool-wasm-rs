//! Resource-table integration for the optional Python SSL compatibility host.
use super::{store::WasmStates, SyncWasmStates};
use std::{
    ops::{Deref, DerefMut},
    sync::MutexGuard,
};
use wasmtime::component::ResourceTable;

pub(crate) struct TableGuard<'a>(MutexGuard<'a, WasmStates>);
impl Deref for TableGuard<'_> {
    type Target = ResourceTable;
    fn deref(&self) -> &ResourceTable {
        &self.0.table
    }
}
impl DerefMut for TableGuard<'_> {
    fn deref_mut(&mut self) -> &mut ResourceTable {
        &mut self.0.table
    }
}
impl twr_python_ssl_host::SslView for SyncWasmStates {
    type Table<'a> = TableGuard<'a>;
    fn ssl_table(&mut self) -> Self::Table<'_> {
        TableGuard(self.state.lock().unwrap_or_else(|p| p.into_inner()))
    }
}
