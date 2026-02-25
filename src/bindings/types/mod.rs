mod collation;
mod error_code;
mod field_type;
mod iterator;
mod validation;

use crate::bindings::tarantool::tarantool::types;
use crate::runtime::tarantool_wasm::SyncWasmStates;

pub use iterator::convert_iterator_type;
pub use validation::{validate_msgpack_array, validate_msgpack_map};

impl types::Host for SyncWasmStates {}
