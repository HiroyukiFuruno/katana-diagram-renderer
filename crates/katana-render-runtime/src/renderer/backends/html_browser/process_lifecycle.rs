use super::{HTML_BROWSER_REQUEST_TIMEOUT, HtmlBrowserError, HtmlBrowserProcess};
use std::process::ExitStatus;

impl HtmlBrowserProcess {
    pub fn terminate(&mut self) -> Result<(), HtmlBrowserError> {
        match self.child.try_wait().map_err(process_terminate_error)? {
            Some(_) => Ok(()),
            None => {
                self.child.kill().map_err(process_terminate_error)?;
                self.child
                    .wait()
                    .map(|_| ())
                    .map_err(process_terminate_error)
            }
        }
    }

    pub fn wait_for_exit(&mut self) -> Result<(), HtmlBrowserError> {
        self.child
            .wait()
            .map(|_| ())
            .map_err(process_terminate_error)
    }

    pub(super) fn timeout(&mut self) -> Result<super::HtmlBrowserResponse, HtmlBrowserError> {
        let _ = self.terminate();
        Err(HtmlBrowserError::ProcessTimeout {
            timeout_ms: HTML_BROWSER_REQUEST_TIMEOUT.as_millis() as u64,
        })
    }

    pub(super) fn child_crashed(&mut self) -> HtmlBrowserError {
        child_crashed_from_status(self.child.try_wait())
    }
}

impl Drop for HtmlBrowserProcess {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

pub(super) fn child_crashed_from_status(
    status: Result<Option<ExitStatus>, std::io::Error>,
) -> HtmlBrowserError {
    match status {
        Ok(Some(status)) => HtmlBrowserError::ProcessCrashed {
            status: status.to_string(),
        },
        Ok(None) => HtmlBrowserError::ProcessCrashed {
            status: "stdout closed while process was still running".into(),
        },
        Err(error) => HtmlBrowserError::ProcessRead {
            error: error.to_string(),
        },
    }
}

pub(super) fn process_terminate_error(error: std::io::Error) -> HtmlBrowserError {
    HtmlBrowserError::ProcessTerminate {
        error: error.to_string(),
    }
}
