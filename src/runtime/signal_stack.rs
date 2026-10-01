use std::{io, ptr};

pub(super) fn install_signal_stack(stack: &libc::stack_t) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let stack = {
        let mut stack = *stack;
        // macOS returns ENOMEM for SS_DISABLE when ss_size < MINSIGSTKSZ,
        // although POSIX says the size is ignored when disabling the stack.
        if stack.ss_flags & libc::SS_DISABLE != 0 {
            stack.ss_size = stack.ss_size.max(libc::MINSIGSTKSZ);
        }
        stack
    };
    #[cfg(target_os = "macos")]
    let stack = &stack;

    // SAFETY: stack is a captured stack configuration or a disabled stack.
    // The caller keeps any enabled stack allocation alive while installed.
    if unsafe { libc::sigaltstack(stack, ptr::null_mut()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_stack() -> libc::stack_t {
        let mut stack = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { libc::sigaltstack(ptr::null(), &mut stack) }, 0);
        stack
    }

    struct RestoreStack(libc::stack_t);

    impl Drop for RestoreStack {
        fn drop(&mut self) {
            install_signal_stack(&self.0).expect("restore original signal stack");
        }
    }

    #[test]
    fn restores_disabled_and_enabled_stacks() {
        // Signal stacks are thread-local. Keep the test and its temporary stack
        // isolated from the test runner, and restore the original before freeing.
        std::thread::spawn(|| {
            let mut storage = vec![0u8; libc::SIGSTKSZ];
            let _restore = RestoreStack(current_stack());
            let enabled = libc::stack_t {
                ss_sp: storage.as_mut_ptr().cast(),
                ss_size: storage.len(),
                ss_flags: 0,
            };
            install_signal_stack(&enabled).unwrap();

            let disabled = libc::stack_t {
                ss_sp: ptr::null_mut(),
                ss_size: 0,
                ss_flags: libc::SS_DISABLE,
            };
            install_signal_stack(&disabled).unwrap();
            assert_ne!(current_stack().ss_flags & libc::SS_DISABLE, 0);

            install_signal_stack(&enabled).unwrap();
            let restored = current_stack();
            assert_eq!(restored.ss_sp, enabled.ss_sp);
            assert_eq!(restored.ss_size, enabled.ss_size);
            assert_eq!(restored.ss_flags, enabled.ss_flags);
        })
        .join()
        .unwrap();
    }
}
