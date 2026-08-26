use std::ffi::OsStr;
use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// `DEBUG=true` のときだけ HTML 描画の段階別所要時間を出力する診断器。
///
/// HTML 本文、URL、入力値は機密情報になり得るため記録しない。
#[derive(Debug)]
pub(super) struct HtmlDebugTrace {
    enabled: bool,
    session_id: u64,
}

impl HtmlDebugTrace {
    pub(super) fn from_env() -> Self {
        Self::new(debug_enabled(std::env::var_os("DEBUG").as_deref()))
    }

    fn new(enabled: bool) -> Self {
        let session_id = if enabled {
            NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed)
        } else {
            0
        };
        Self {
            enabled,
            session_id,
        }
    }

    #[cfg(test)]
    pub(super) fn disabled() -> Self {
        Self::new(false)
    }

    #[cfg(test)]
    pub(super) fn enabled_for_test() -> Self {
        Self::new(true)
    }

    pub(super) fn start(&self) -> Option<Instant> {
        self.enabled.then(Instant::now)
    }

    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn finish(
        &self,
        generation: u64,
        phase: &'static str,
        started: Option<Instant>,
        metrics: &[(&'static str, usize)],
    ) {
        let Some(started) = started else {
            return;
        };
        let mut line = format!(
            "[krr-html-trace] session={} frame={} phase={} elapsed_us={}",
            self.session_id,
            generation,
            phase,
            started.elapsed().as_micros()
        );
        for (name, value) in metrics {
            let _ = write!(line, " {name}={value}");
        }
        eprintln!("{line}");
    }
}

fn debug_enabled(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case("true"))
}

#[cfg(test)]
mod tests {
    use super::{HtmlDebugTrace, debug_enabled};
    use std::ffi::OsStr;

    #[test]
    fn debug_trace_requires_explicit_true_value() {
        assert!(debug_enabled(Some(OsStr::new("true"))));
        assert!(debug_enabled(Some(OsStr::new("TRUE"))));
        assert!(!debug_enabled(Some(OsStr::new("1"))));
        assert!(!debug_enabled(Some(OsStr::new("false"))));
        assert!(!debug_enabled(None));
    }

    #[test]
    fn disabled_trace_skips_timing_and_output() {
        let trace = HtmlDebugTrace::disabled();

        assert!(trace.start().is_none());
        assert!(!trace.enabled());
        trace.finish(1, "ignored", None, &[("nodes", 1)]);
    }

    #[test]
    fn enabled_trace_accepts_phase_metrics() {
        let trace = HtmlDebugTrace::new(true);
        let started = trace.start();

        assert!(started.is_some());
        assert!(trace.enabled());
        trace.finish(7, "layout_svg", started, &[("svg_bytes", 128)]);
    }
}
