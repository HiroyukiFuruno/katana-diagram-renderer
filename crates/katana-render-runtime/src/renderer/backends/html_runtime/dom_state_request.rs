use super::{HtmlDomBridgeState, argument, node_id};
use crate::renderer::backends::html_runtime::types::DomValue;

struct HostIoActivity<'a> {
    active: &'a std::sync::atomic::AtomicBool,
}

impl<'a> HostIoActivity<'a> {
    fn begin(active: &'a std::sync::atomic::AtomicBool) -> Self {
        active.store(true, std::sync::atomic::Ordering::SeqCst);
        Self { active }
    }
}

impl Drop for HostIoActivity<'_> {
    fn drop(&mut self) {
        self.active
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

impl HtmlDomBridgeState {
    pub(super) fn request_text(&self, arguments: &[String]) -> Result<DomValue, String> {
        let method = argument(arguments, 0)?;
        let reference = argument(arguments, 1)?;
        let result = if method.eq_ignore_ascii_case("GET") {
            let _activity = HostIoActivity::begin(&self.host_io_active);
            self.resource_loader
                .as_ref()
                .ok_or_else(|| "dynamic requests require an interactive session".to_string())
                .and_then(|loader| loader.load_same_origin_text(reference))
        } else {
            Err(format!("dynamic request method is not allowed: {method}"))
        };
        let response = match result {
            Ok(response_text) => serde_json::json!({
                "ok": true,
                "status": 200,
                "statusText": "OK",
                "responseText": response_text,
            }),
            Err(error) => serde_json::json!({
                "ok": false,
                "status": 0,
                "statusText": error,
                "responseText": "",
            }),
        };
        Ok(DomValue::String(response.to_string()))
    }

    pub(crate) fn event_target_ids(&self, event_type: &str) -> std::collections::HashSet<u64> {
        self.event_targets
            .borrow()
            .get(event_type)
            .cloned()
            .unwrap_or_else(std::collections::HashSet::new)
    }

    pub(super) fn set_event_target(&self, arguments: &[String]) -> Result<DomValue, String> {
        let node_id = node_id(argument(arguments, 0)?)?;
        self.document.borrow().node(node_id)?;
        let event_type = argument(arguments, 1)?.to_string();
        let enabled = argument(arguments, 2)? == "true";
        let mut targets = self.event_targets.borrow_mut();
        let nodes = targets.entry(event_type.clone()).or_default();
        if enabled {
            nodes.insert(node_id);
        } else {
            nodes.remove(&node_id);
            if nodes.is_empty() {
                targets.remove(&event_type);
            }
        }
        Ok(DomValue::Undefined)
    }
}
