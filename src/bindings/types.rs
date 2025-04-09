use crate::bindings::docs::adder::types::{self, BoxError, BoxTuple, Index, IteratorType};
use crate::tarantool_wasm::SyncWasmStates;
use crate::bindings::IteratorImpl;

use wasmtime::component::Resource;
use wasmtime_wasi::async_trait;
use rmpv::decode::read_value;

pub fn validate_msgpack_array(data: &[u8]) -> Result<(), String> {
    if data.is_empty() {
        return Err("Tuple data is empty".to_string());
    }
    match read_value(&mut &data[..]) {
        Ok(val) if val.is_array() => Ok(()),
        Ok(other) => Err(format!("Expected MsgPack array, got {:?}", other)),
        Err(e) => Err(format!("Invalid MsgPack data: {}", e)),
    }
}

unsafe impl Send for IteratorImpl {}
unsafe impl Sync for IteratorImpl {}

pub fn convert_iterator_type(t: IteratorType) -> Result<tarantool::index::IteratorType, BoxError> {
    use tarantool::index::IteratorType as Native;

    Ok(match t {
        IteratorType::IterEq => Native::Eq,
        IteratorType::IterReq => Native::Req,
        IteratorType::IterAll => Native::All,
        IteratorType::IterLt => Native::LT,
        IteratorType::IterLe => Native::LE,
        IteratorType::IterGe => Native::GE,
        IteratorType::IterGt => Native::GT,
        IteratorType::IterBitsAllSet => Native::BitsAllSet,
        IteratorType::IterBitsAnySet => Native::BitsAnySet,
        IteratorType::IterBitsAllNotSet => Native::BitsAllNotSet,
        IteratorType::IterOverlaps => Native::Overlaps,
        IteratorType::IterNeighbor => Native::Neighbor,
        // need for patch https://github.com/picodata/tarantool-module/blob/e4c78b9b0e59ef45ab6e8a7023aa192b28e330c0/tarantool/src/index.rs#L57
        // https://github.com/tarantool/tarantool/pull/10028
        IteratorType::IterNp | IteratorType::IterPp => {
            return Err(BoxError::new("Unsupported iterator type", "IteratorType", 1))
        }
    })
}


#[async_trait]
impl types::Host for SyncWasmStates {
}

#[async_trait]
impl types::HostIterator for SyncWasmStates {
    async fn new_iterator(
        &mut self,
        index: Index,
        iterator_type: IteratorType,
        key: Vec<u8>,
    ) -> Result<Resource<IteratorImpl>, BoxError> {
        let native_iter_type = convert_iterator_type(iterator_type)?;
        let tuple = unsafe { tarantool::tuple::Tuple::from_slice(&key) };

        let ind = unsafe {
            tarantool::index::Index::from_ids_unchecked(
                index.space_id as tarantool::space::SpaceId,
                index.id as tarantool::index::IndexId,
            )
        };

        let iter = ind
            .select(native_iter_type, &tuple)
            .map_err(|e| BoxError::new(&e.to_string(), "IteratorInit", 1))?;

        Ok(self.0.lock().unwrap().table.push(IteratorImpl { inner: iter }).unwrap())
    }

    async fn next(
        &mut self,
        self_: Resource<types::Iterator>,
    ) -> Result<BoxTuple, BoxError> {
        let mut guard = self.0.lock().unwrap();
        let iter = guard.table.get_mut(&self_)
            .map_err(|e| BoxError::new(
                format!("Resource error: {}", e),
                "internal",
                2,
            ))?;

        match iter.inner.next() {
            Some(t) => Ok(t.as_ptr() as BoxTuple),
            None => Err(BoxError::new("No more items", "StopIteration", 0)),
        }
    }

    async fn drop(
        &mut self, 
        self_: Resource<types::Iterator>
    ) -> Result<(), wasmtime::Error> {
        self.0.lock().unwrap().table.delete(self_)?;
        Ok(())
    }
}
