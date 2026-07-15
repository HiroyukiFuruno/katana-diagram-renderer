const TRACE_ENV: &str = "KRR_HTML_CHROMIUM_TRACE";

pub(super) fn stage(name: &str) {
    if trace_enabled() {
        eprintln!("krr-html-chromium-engine stage={name}");
    }
}

fn trace_enabled() -> bool {
    std::env::var_os(TRACE_ENV).is_some_and(|value| value == "1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static TRACE_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn trace_enabled_accepts_only_explicit_one() {
        let _guard = trace_env_guard();
        unsafe { std::env::set_var(TRACE_ENV, "1") };

        assert!(trace_enabled());
        stage("test");
    }

    #[test]
    fn trace_enabled_ignores_missing_or_other_values() {
        let _guard = trace_env_guard();
        unsafe { std::env::remove_var(TRACE_ENV) };
        assert!(!trace_enabled());

        unsafe { std::env::set_var(TRACE_ENV, "true") };
        assert!(!trace_enabled());
    }

    #[test]
    fn trace_env_guard_recovers_from_poisoned_lock() {
        let _ = std::panic::catch_unwind(|| {
            let _guard = trace_env_guard();
            std::panic::resume_unwind(Box::new("poison trace env lock"));
        });

        drop(trace_env_guard());
    }

    fn trace_env_guard() -> MutexGuard<'static, ()> {
        TRACE_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}
