use super::script::HtmlTryCatchScope;
use super::types::HtmlRuntimeError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

const EXECUTION_TIMEOUT: Duration = Duration::from_millis(100);

pub(super) struct ExecutionBudget {
    completion: mpsc::Sender<()>,
    timed_out: Arc<AtomicBool>,
    timer: JoinHandle<()>,
}

impl ExecutionBudget {
    pub(super) fn start(scope: &HtmlTryCatchScope<'_, '_, '_, '_>) -> Self {
        let isolate = scope.thread_safe_handle();
        let (completion, wait_for_completion) = mpsc::channel();
        let timed_out = Arc::new(AtomicBool::new(false));
        let timeout_marker = Arc::clone(&timed_out);
        let timer = std::thread::spawn(move || {
            if wait_for_completion.recv_timeout(EXECUTION_TIMEOUT).is_err() {
                timeout_marker.store(true, Ordering::SeqCst);
                isolate.terminate_execution();
            }
        });
        Self {
            completion,
            timed_out,
            timer,
        }
    }

    pub(super) fn finish(self) -> Result<(), HtmlRuntimeError> {
        let _ = self.completion.send(());
        self.timer
            .join()
            .map_err(|_| HtmlRuntimeError::ExecutionTimeout)?;
        if self.timed_out.load(Ordering::SeqCst) {
            return Err(HtmlRuntimeError::ExecutionTimeout);
        }
        Ok(())
    }
}
