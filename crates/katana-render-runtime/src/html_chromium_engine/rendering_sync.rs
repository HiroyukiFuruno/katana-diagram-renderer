use self::scripts::{RENDERING_READY_SCRIPT, loaded_rendering_ready_script};
use super::{
    page::{ChromiumPage, string_error},
    trace,
};
use serde_json::Value;
use std::{
    thread,
    time::{Duration, Instant},
};

mod scripts;

const LOADED_RENDERING_SYNC_TIMEOUT: Duration = Duration::from_secs(10);
const LOADED_RENDERING_SYNC_INTERVAL: Duration = Duration::from_millis(20);

impl ChromiumPage {
    pub(super) fn synchronize_rendering(&self) -> Result<(), String> {
        trace::stage("page:rendering-sync:evaluate");
        evaluate_rendering_sync(self, RENDERING_READY_SCRIPT).map(|()| {
            trace::stage("page:rendering-sync:ready");
        })
    }

    pub(super) fn synchronize_loaded_rendering(&self, expected_url: &str) -> Result<(), String> {
        trace::stage("page:rendering-load-sync:evaluate");
        evaluate_loaded_rendering_sync(self, expected_url).map(|()| {
            trace::stage("page:rendering-load-sync:ready");
        })
    }
}

fn evaluate_loaded_rendering_sync(page: &ChromiumPage, expected_url: &str) -> Result<(), String> {
    let script = loaded_rendering_ready_script(expected_url);
    let mut evaluate = || {
        if page.navigation.has_confirmed() {
            return Ok(true);
        }
        page.tab
            .evaluate(&script, true)
            .map(|result| is_ready_value(result.value))
            .map_err(string_error)
    };
    retry_loaded_rendering_sync(
        &mut evaluate,
        LOADED_RENDERING_SYNC_TIMEOUT,
        LOADED_RENDERING_SYNC_INTERVAL,
    )
}

fn evaluate_rendering_sync(page: &ChromiumPage, script: &str) -> Result<(), String> {
    page.tab
        .evaluate(script, true)
        .map(|_| ())
        .map_err(string_error)
}

fn is_ready_value(value: Option<Value>) -> bool {
    match value {
        Some(Value::Bool(true)) => true,
        Some(Value::Bool(false)) | Some(_) | None => false,
    }
}

fn retry_loaded_rendering_sync(
    evaluate: &mut dyn FnMut() -> Result<bool, String>,
    timeout: Duration,
    retry_interval: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        let evaluation = evaluate();
        let error = match evaluation {
            Ok(true) => return Ok(()),
            Ok(false) => "browser document did not reach expected URL".to_string(),
            Err(error) => error,
        };
        if Instant::now() >= deadline {
            return Err(error);
        }
        thread::sleep(retry_interval);
    }
}

#[cfg(test)]
mod tests;
