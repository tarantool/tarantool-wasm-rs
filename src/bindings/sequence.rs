use crate::bindings::error::handle_tarantool_error;
use crate::bindings::tarantool::tarantool::sequence;
use crate::bindings::tarantool::tarantool::types::{BoxError, Sequence};
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;

impl sequence::Host for SyncWasmStates {
    async fn next(&mut self, seq: Sequence) -> Result<i64, BoxError> {
        let seq_copy = seq.clone();

        self.executor()
            .execute(move || {
                let mut res: i64 = 0;
                let rc = unsafe { ffi::r#box::box_sequence_next(seq_copy.id as u32, &mut res) };

                match rc {
                    0 => Ok(res),
                    _ => Err(handle_tarantool_error()),
                }
            })
            .await
    }

    async fn current(&mut self, seq: Sequence) -> Result<i64, BoxError> {
        let mut res: i64 = 0;
        let rc = unsafe { ffi::r#box::box_sequence_current(seq.id as u32, &mut res) };

        match rc {
            0 => Ok(res),
            _ => Err(handle_tarantool_error()),
        }
    }

    async fn set(&mut self, seq: Sequence, value: i64) -> Result<(), BoxError> {
        let seq_copy = seq.clone();
        let value_copy = value.clone();

        self.executor()
            .execute(move || {
                let rc = unsafe { ffi::r#box::box_sequence_set(seq_copy.id as u32, value_copy) };

                match rc {
                    0 => Ok(()),
                    _ => Err(handle_tarantool_error()),
                }
            })
            .await
    }

    async fn reset(&mut self, seq: Sequence) -> Result<(), BoxError> {
        let seq_copy = seq.clone();

        self.executor()
            .execute(move || {
                let rc = unsafe { ffi::r#box::box_sequence_reset(seq_copy.id as u32) };

                match rc {
                    0 => Ok(()),
                    _ => Err(handle_tarantool_error()),
                }
            })
            .await
    }
}
