pub mod types {
    #[repr(C)]
    pub struct BoxErrorT {
        _private: [u8; 0], // Opaque type
    }

    #[repr(C)]
    pub struct BoxTupleT {
        _private: [u8; 0], // Opaque type
    }
    
    pub const BOX_ID_NIL: u32 = 2147483647;    
}

pub mod tnt {
    use std::os::raw::c_void;

    extern "C" {
        // Schedule the given callback to be executed in TX thread with the provided argument. In
        // order for the messages to be actually sent to TX thread the user must call
        // tnt_tx_flush() in the same thread as the pushes.
        //
        // In TX thread the callbacks are guaranteed to start execution in the same order as the
        // push, but the order of completion is undefined if they are yielding, since they get
        // executed potentially in different fibers.
        //
        // If called during Tarantool shutdown, the behaviour is undefined. The external threads
        // must be terminated before that.
        //
        // If called in TX thread, the behaviour is undefined.
        pub fn tnt_tx_push(cb: extern "C" fn(*mut c_void), arg: *mut c_void);
        // Send all the pending callbacks of this thread to TX thread. Note, that it doesn't
        // guarantee that they are already executed when this function returns. They are only sent
        // to TX thread, not called yet. It is the caller's responsibility to ensure that the
        // messages are not being sent faster than TX thread is handling them. Otherwise the queue
        // in TX thread grows faster than shrinks and could lead to unpredictable latency and even
        // OOM.
        //
        // Note, that push is very cheap while flush is relatively expensive both for this thread
        // and for TX thread. Avoid calling it on each push if possible.
        pub fn tnt_tx_flush();

        pub fn box_txn() -> bool;
        pub fn box_txn_begin() -> i32;
        pub fn box_txn_commit() -> i32;
        pub fn box_txn_rollback() -> i32;
    }
}

pub mod error {
    use crate::ffi::types::BoxErrorT;

    extern "C" {
        pub fn box_error_last() -> *const BoxErrorT;
        pub fn box_error_type(error: *const BoxErrorT) -> *const i8;
        pub fn box_error_code(error: *const BoxErrorT) -> u32;
        pub fn box_error_message(error: *const BoxErrorT) -> *const i8;
        pub fn box_error_clear();
        pub fn box_error_set(file: *const i8, line: u32, code: u32, format: *const i8, ...);
    }
}

pub mod r#box {
    use crate::ffi::types::BoxTupleT;
    
    extern "C" {
        pub fn box_schema_version() -> u32;
        pub fn box_space_id_by_name(name: *const i8, len: u32) -> u32;
        pub fn box_index_id_by_name(space_id: u32, name: *const u8, len: u32) -> u32;
        pub fn box_insert(space_id: u32, tuple: *const u8, tuple_end: *const u8, result: *mut *mut BoxTupleT) -> i32;
        pub fn box_update(space_id: u32, index_id: u32,
            key: *const u8, key_end: *const u8,
            ops: *const u8, ops_end: *const u8, index_base: i32,
            result: *mut *mut BoxTupleT) -> i32;

        pub fn box_replace(space_id: u32, tuple: *const u8, tuple_end: *const u8, result: *mut *mut BoxTupleT) -> i32;

        pub fn box_upsert(space_id: u32, index_id: u32,
            tuple: *const u8, tuple_end: *const u8,
            ops: *const u8, ops_end: *const u8,
            index_base: i32,
            result: *mut *mut BoxTupleT) -> i32;

        pub fn box_delete(
            space_id: u32,
            index_id: u32,
            key: *const u8,
            key_end: *const u8,
            result: *mut *mut BoxTupleT,
        ) -> i32;
        
        pub fn box_truncate(space_id: u32) -> i32;
    }
}

pub mod session {
    extern "C" {
        pub fn box_session_id() -> u64;
        pub fn box_iproto_send(
            sid: u64,
            header: *const u8,
            header_end: *const u8,
            body: *const u8,
            body_end: *const u8,
        ) -> i32;
    }
}
