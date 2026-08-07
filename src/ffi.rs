//! Low level bindings to Tarantool's C API.
//!
//! Only the pieces required by the WASM runtime are defined here. The
//! structures are mostly opaque wrappers around their C counterparts and all
//! functions are unsafe by definition.

pub mod types {
    //! Opaque representations of various Tarantool C structures.
    use libc;

    const BOX_KEY_PART_DEF_T_SIZE: usize = 64;

    #[repr(C)]
    pub struct BoxTupleFormatT {
        _private: [u8; 0], // Оpaque type
    }

    #[repr(C)]
    pub struct BoxTupleT {
        _private: [u8; 0], // Opaque type
    }

    #[repr(C)]
    pub struct BoxTupleIteratorT {
        _private: [u8; 0], // Opaque type
    }

    #[repr(C)]
    pub struct BoxKeyDefT {
        _private: [u8; 0], // Opaque type
    }

    #[repr(C)]
    pub union BoxKeyPartDefT {
        pub fields: std::mem::ManuallyDrop<BoxKeyPartDefFields>,
        pub padding: [u8; BOX_KEY_PART_DEF_T_SIZE],
    }

    #[repr(C)]
    pub struct BoxKeyPartDefFields {
        pub fieldno: u32,
        pub flags: u32,
        pub field_type: *const libc::c_char,
        pub collation: *const libc::c_char,
        pub path: *const libc::c_char,
    }

    impl Default for BoxKeyPartDefT {
        fn default() -> Self {
            unsafe { std::mem::zeroed() }
        }
    }
}

pub mod tnt {
    //! Functions for communicating with Tarantool's transaction thread and
    //! transaction utilities.
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

        pub fn box_txn_id() -> i64;
        pub fn box_txn_isolation() -> i32;
        pub fn box_txn_set_isolation(level: u32) -> i32;
        pub fn box_txn_make_sync();
    }
}

pub mod r#box {
    //! Bindings to selected `box_*` functions from Tarantool.
    extern "C" {
        pub fn box_region_used() -> usize;
        pub fn box_region_truncate(size: usize);
        pub fn box_schema_version() -> u32;
        pub fn box_sequence_current(seq_id: u32, result: *mut i64) -> i32;
        pub fn box_sequence_next(seq_id: u32, result: *mut i64) -> i32;
        pub fn box_sequence_reset(seq_id: u32) -> i32;
        pub fn box_sequence_set(seq_id: u32, value: i64) -> i32;
    }
}

pub mod session {
    //! Session and network related helpers.
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

pub mod tuple {
    //! Tuple manipulation routines.
    use crate::ffi::types::{
        BoxKeyDefT, BoxKeyPartDefT, BoxTupleFormatT, BoxTupleIteratorT, BoxTupleT,
    };

    extern "C" {
        pub fn box_key_def_new_v2(parts: *mut BoxKeyPartDefT, part_count: u32) -> *mut BoxKeyDefT;

        pub fn box_key_def_delete(key_def: *mut BoxKeyDefT);

        pub fn box_key_def_merge(
            first: *const BoxKeyDefT,
            second: *const BoxKeyDefT,
        ) -> *mut BoxKeyDefT;

        pub fn box_key_def_dup(key_def: *const BoxKeyDefT) -> *mut BoxKeyDefT;

        pub fn box_key_def_dump_parts(
            key_def: *const BoxKeyDefT,
            part_count_ptr: *mut u32,
        ) -> *mut BoxKeyPartDefT;

        pub fn box_key_def_validate_key(
            key_def: *const BoxKeyDefT,
            key: *const libc::c_char,
            key_size: *mut u32,
        ) -> i32;

        pub fn box_key_def_validate_full_key(
            key_def: *const BoxKeyDefT,
            key: *const libc::c_char,
            key_size: *mut u32,
        ) -> i32;

        pub fn box_tuple_format_new(
            keys: *const *const BoxKeyDefT,
            key_count: u16,
        ) -> *mut BoxTupleFormatT;

        pub fn box_tuple_format_ref(format: *mut BoxTupleFormatT);

        pub fn box_tuple_format_unref(format: *mut BoxTupleFormatT);

        pub fn box_tuple_update(
            tuple: *mut BoxTupleT,
            expr: *const u8,
            expr_end: *const u8,
        ) -> *mut BoxTupleT;

        pub fn box_tuple_upsert(
            tuple: *mut BoxTupleT,
            expr: *const u8,
            expr_end: *const u8,
        ) -> *mut BoxTupleT;

        pub fn box_tuple_to_buf(
            tuple: *mut BoxTupleT,
            buf: *mut ::std::os::raw::c_char,
            size: usize,
        ) -> isize;

        pub fn box_tuple_ref(tuple: *mut BoxTupleT) -> i32;

        pub fn box_tuple_unref(tuple: *mut BoxTupleT);

        pub fn box_tuple_field_count(tuple: *mut BoxTupleT) -> u32;

        pub fn box_tuple_bsize(tuple: *mut BoxTupleT) -> usize;

        pub fn box_tuple_data(tuple: *mut BoxTupleT, bsize: *mut usize) -> *const u8;

        pub fn box_tuple_format(tuple: *mut BoxTupleT) -> *mut BoxTupleFormatT;

        pub fn box_tuple_field(tuple: *mut BoxTupleT, fieldno: u32) -> *const u8;

        pub fn box_tuple_field_by_path(
            tuple: *mut BoxTupleT,
            path: *const libc::c_char,
            path_len: u32,
            index_base: i32,
        ) -> *const u8;

        pub fn box_tuple_validate(tuple: *mut BoxTupleT, format: *mut BoxTupleFormatT) -> i32;

        pub fn box_tuple_compare(
            tuple_a: *mut BoxTupleT,
            tuple_b: *mut BoxTupleT,
            key_def: *mut BoxKeyDefT,
        ) -> i32;

        pub fn box_tuple_compare_with_key(
            tuple_a: *mut BoxTupleT,
            key_b: *const libc::c_char,
            key_def: *mut BoxKeyDefT,
        ) -> i32;

        pub fn box_key_def_validate_tuple(key_def: *mut BoxKeyDefT, tuple: *mut BoxTupleT) -> i32;

        pub fn box_key_def_extract_key(
            key_def: *mut BoxKeyDefT,
            tuple: *mut BoxTupleT,
            multikey_idx: i32,
            key_size_ptr: *mut u32,
        ) -> *mut libc::c_char;

        pub fn box_tuple_iterator(tuple: *mut BoxTupleT) -> *mut BoxTupleIteratorT;

        pub fn box_tuple_iterator_free(iterator: *mut BoxTupleIteratorT);

        pub fn box_tuple_position(iterator: *mut BoxTupleIteratorT) -> u32;

        pub fn box_tuple_rewind(iterator: *mut BoxTupleIteratorT);

        pub fn box_tuple_seek(
            iterator: *mut BoxTupleIteratorT,
            fieldno: u32,
        ) -> *const libc::c_char;

        pub fn box_tuple_next(iterator: *mut BoxTupleIteratorT) -> *const libc::c_char;
    }
}

pub mod tarantool_extra {
    use libc::c_double;

    extern "C" {
        /// C: double tarantool_uptime(void);
        pub fn tarantool_uptime() -> c_double;
    }
}
