use super::*;

pub(super) fn shutdown_runtime() -> Result<()> {
    debug_assert_tx_thread();
    if !begin_shutdown() {
        return Ok(());
    }

    runtime_state.engine.increment_epoch();
    runtime_state.fuel_engine.increment_epoch();
    let sessions = loop {
        if let Ok(mut sessions) = runtime_state.sessions.try_write() {
            break sessions.drain().collect::<Vec<_>>();
        }
        drain_stopping_queue();
        fiber::sleep(Duration::from_millis(1));
    };
    let interpreters = loop {
        if let Ok(mut interpreters) = runtime_state.shared_interpreters.try_write() {
            break interpreters.drain().collect::<Vec<_>>();
        }
        drain_stopping_queue();
        fiber::sleep(Duration::from_millis(1));
    };
    let runs = loop {
        if let Ok(mut runs) = runtime_state.runs.try_write() {
            break runs.drain().collect::<Vec<_>>();
        }
        drain_stopping_queue();
        fiber::sleep(Duration::from_millis(1));
    };
    let pipes = loop {
        if let Ok(pipes) = runtime_state.pipes.try_read() {
            break pipes.values().cloned().collect::<Vec<_>>();
        }
        drain_stopping_queue();
        fiber::sleep(Duration::from_millis(1));
    };

    for (_, session) in &sessions {
        let (done_tx, _done_rx) = std_mpsc::channel();
        let _ = session.cmd_tx.send(SessionCommand::Close(done_tx));
    }
    for (_, interpreter) in &interpreters {
        let (done_tx, _done_rx) = std_mpsc::channel();
        let _ = interpreter.cmd_tx.send(InterpreterCommand::Close(done_tx));
    }
    for (_, run) in &runs {
        run.cancel_tx.request();
    }
    for pipe in &pipes {
        pipe.cancel();
    }

    while lifecycle::active_workers() != 0 {
        drain_stopping_queue();
        fiber::sleep(Duration::from_millis(1));
    }
    drain_stopping_queue();

    let mut errors = Vec::new();
    for (name, session) in sessions {
        join_worker_at_shutdown(
            &format!("session `{name}`"),
            &session.join_handle,
            &mut errors,
        );
    }
    for (name, interpreter) in interpreters {
        join_worker_at_shutdown(
            &format!("interpreter `{name}`"),
            &interpreter.join_handle,
            &mut errors,
        );
    }
    for (id, run) in runs {
        join_worker_at_shutdown(
            &format!("background run #{id}"),
            &run.join_handle,
            &mut errors,
        );
    }
    for pipe in pipes {
        if let Err(err) = pipe.wait_join_for_cleanup() {
            errors.push(format!("pipe process failed: {err}"));
        }
    }
    runtime_state.pipes.write().unwrap().clear();

    runtime_state.modules.write().unwrap().clear();
    runtime_state.components.write().unwrap().clear();
    lifecycle::finish_stopping();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(errors.join("; ")))
    }
}

fn join_worker_at_shutdown(
    name: &str,
    handle: &Mutex<Option<JoinHandle<Result<()>>>>,
    errors: &mut Vec<String>,
) {
    let Some(handle) = handle.lock().unwrap().take() else {
        return;
    };
    match handle.join() {
        Ok(Ok(())) => {}
        Ok(Err(err)) => errors.push(format!("{name} failed: {err}")),
        Err(_) => errors.push(format!("{name} panicked")),
    }
}
