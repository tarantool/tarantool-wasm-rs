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
}
