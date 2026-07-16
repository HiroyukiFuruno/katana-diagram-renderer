use super::{
    page::{ChromiumPage, string_error},
    trace,
};
use serde_json::Value;
use std::{
    thread,
    time::{Duration, Instant},
};

const EXPECTED_URL_PLACEHOLDER: &str = "__KRR_EXPECTED_URL__";
const LOADED_RENDERING_SYNC_TIMEOUT: Duration = Duration::from_secs(10);
const LOADED_RENDERING_SYNC_INTERVAL: Duration = Duration::from_millis(20);
pub(super) const LOADED_RENDERING_READY_SCRIPT: &str = r#"
(() => {
  const expectedUrl = __KRR_EXPECTED_URL__;
  if (location.href !== expectedUrl) {
    return false;
  }
  return new Promise(resolve => setTimeout(resolve, 0))
  .then(() => {
    const DOCUMENT_READY_TIMEOUT_MS = 2000;
    if (document.readyState === 'complete') {
      return undefined;
    }
    return new Promise(resolve => {
      let resolved = false;
      const finish = () => {
        if (!resolved) {
          resolved = true;
          resolve();
        }
      };
      window.addEventListener('load', finish, { once: true });
      setTimeout(finish, DOCUMENT_READY_TIMEOUT_MS);
    });
  })
  .then(() => {
    const RESOURCE_READY_TIMEOUT_MS = 2000;
    const waitForResourceEvent = subscribe => new Promise(resolve => {
      let resolved = false;
      const finish = () => {
        if (!resolved) {
          resolved = true;
          resolve();
        }
      };
      subscribe(finish);
      setTimeout(finish, RESOURCE_READY_TIMEOUT_MS);
    });
    return waitForResourceEvent;
  })
  .then(waitForResourceEvent => {
    const stylesheetReady = link => {
      if (link.sheet) {
        try {
          link.sheet.cssRules;
          return undefined;
        } catch (_) {
          return undefined;
        }
      }
      return waitForResourceEvent(finish => {
        link.addEventListener('load', finish, { once: true });
        link.addEventListener('error', finish, { once: true });
      });
    };
    return Promise.all(Array.from(document.querySelectorAll('link[rel~="stylesheet"]')).map(stylesheetReady));
  })
  .then(() => Promise.all(Array.from(document.images || []).map(image => {
    if (image.complete) {
      return image.naturalWidth > 0 && image.decode ? image.decode().catch(() => undefined) : undefined;
    }
    return new Promise(resolve => {
      let resolved = false;
      const finish = () => {
        if (!resolved) {
          resolved = true;
          resolve();
        }
      };
      image.addEventListener('load', finish, { once: true });
      image.addEventListener('error', finish, { once: true });
      setTimeout(finish, RESOURCE_READY_TIMEOUT_MS);
    });
  })))
  .then(() => new Promise(resolve => {
    let resolved = false;
    const finish = () => {
      if (!resolved) {
        resolved = true;
        resolve();
      }
    };
    requestAnimationFrame(() => requestAnimationFrame(finish));
    setTimeout(finish, 100);
  }))
  .then(() => {
    document.documentElement.getBoundingClientRect();
    return location.href === expectedUrl;
  });
})()
"#;

pub(super) const RENDERING_READY_SCRIPT: &str = r#"
new Promise(resolve => {
  let resolved = false;
  const finish = () => {
    if (!resolved) {
      resolved = true;
      resolve();
    }
  };
  requestAnimationFrame(() => requestAnimationFrame(finish));
  setTimeout(finish, 100);
})
  .then(() => {
    document.documentElement.getBoundingClientRect();
    return true;
  })
"#;

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

fn loaded_rendering_ready_script(expected_url: &str) -> String {
    let url = Value::String(expected_url.to_string()).to_string();
    LOADED_RENDERING_READY_SCRIPT.replace(EXPECTED_URL_PLACEHOLDER, &url)
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
mod tests {
    use super::*;

    #[test]
    fn rendering_sync_waits_for_two_animation_frames_with_timeout_fallback() {
        assert_eq!(
            RENDERING_READY_SCRIPT
                .matches("requestAnimationFrame")
                .count(),
            2
        );
        assert!(RENDERING_READY_SCRIPT.contains("setTimeout(finish, 100)"));
        assert!(RENDERING_READY_SCRIPT.contains("getBoundingClientRect"));
    }

    #[test]
    fn rendering_sync_capture_script_uses_only_paint_barrier() {
        assert!(!RENDERING_READY_SCRIPT.contains("DOCUMENT_READY_TIMEOUT_MS"));
        assert!(!RENDERING_READY_SCRIPT.contains("RESOURCE_READY_TIMEOUT_MS"));
        assert!(!RENDERING_READY_SCRIPT.contains("window.addEventListener('load'"));
    }

    #[test]
    fn rendering_sync_waits_for_document_scripts_before_resource_paint() {
        assert!(LOADED_RENDERING_READY_SCRIPT.contains("location.href !== expectedUrl"));
        assert!(LOADED_RENDERING_READY_SCRIPT.contains("document.readyState === 'complete'"));
        assert!(LOADED_RENDERING_READY_SCRIPT.contains("window.addEventListener('load'"));
        assert!(LOADED_RENDERING_READY_SCRIPT.contains("DOCUMENT_READY_TIMEOUT_MS = 2000"));
    }

    #[test]
    fn rendering_sync_waits_for_stylesheet_and_image_resources() {
        assert!(LOADED_RENDERING_READY_SCRIPT.contains("waitForResourceEvent"));
        assert!(LOADED_RENDERING_READY_SCRIPT.contains("RESOURCE_READY_TIMEOUT_MS = 2000"));
        assert!(
            LOADED_RENDERING_READY_SCRIPT.contains("setTimeout(finish, RESOURCE_READY_TIMEOUT_MS)")
        );
        assert!(LOADED_RENDERING_READY_SCRIPT.contains("link[rel~=\"stylesheet\"]"));
        assert!(LOADED_RENDERING_READY_SCRIPT.contains("cssRules"));
        assert!(LOADED_RENDERING_READY_SCRIPT.contains("document.images"));
        assert!(LOADED_RENDERING_READY_SCRIPT.contains("image.decode"));
    }

    #[test]
    fn loaded_rendering_script_injects_expected_url_as_json() {
        let script = loaded_rendering_ready_script("file:///tmp/document.html?name=\"quoted\"");

        assert!(!script.contains(EXPECTED_URL_PLACEHOLDER));
        assert!(script.contains(r#"file:///tmp/document.html?name=\"quoted\""#));
    }

    #[test]
    fn loaded_rendering_ready_value_accepts_only_boolean_true() {
        assert!(is_ready_value(Some(Value::Bool(true))));
        assert!(!is_ready_value(Some(Value::Bool(false))));
        assert!(!is_ready_value(Some(Value::String("true".to_string()))));
        assert!(!is_ready_value(None));
    }

    #[test]
    fn loaded_rendering_retry_accepts_ready_after_pending_document() {
        let mut attempts = 0;
        let mut evaluate = || {
            attempts += 1;
            Ok(attempts == 2)
        };

        assert_eq!(
            retry_loaded_rendering_sync(&mut evaluate, Duration::from_secs(1), Duration::ZERO,),
            Ok(())
        );

        assert_eq!(attempts, 2);
    }

    #[test]
    fn loaded_rendering_retry_retries_transient_evaluation_errors() {
        let mut attempts = 0;
        let mut evaluate = || {
            attempts += 1;
            if attempts == 1 {
                Err("execution context was destroyed".to_string())
            } else {
                Ok(true)
            }
        };

        assert_eq!(
            retry_loaded_rendering_sync(&mut evaluate, Duration::from_secs(1), Duration::ZERO,),
            Ok(())
        );

        assert_eq!(attempts, 2);
    }

    #[test]
    fn loaded_rendering_retry_reports_pending_document_timeout() {
        let mut evaluate = || Ok(false);

        assert_eq!(
            retry_loaded_rendering_sync(&mut evaluate, Duration::ZERO, Duration::ZERO),
            Err("browser document did not reach expected URL".to_string())
        );
    }

    #[test]
    fn loaded_rendering_retry_reports_last_evaluation_error_on_timeout() {
        let mut evaluate = || Err("execution context was destroyed".to_string());

        assert_eq!(
            retry_loaded_rendering_sync(&mut evaluate, Duration::ZERO, Duration::ZERO,),
            Err("execution context was destroyed".to_string())
        );
    }
}
