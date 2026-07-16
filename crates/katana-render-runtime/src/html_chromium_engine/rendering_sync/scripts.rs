use serde_json::Value;

pub(super) const EXPECTED_URL_PLACEHOLDER: &str = "__KRR_EXPECTED_URL__";

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
  .then(() => {
    const SCRIPT_READY_TIMEOUT_MS = 2000;
    const scriptReady = script => {
      if (!script.src) {
        return undefined;
      }
      if (script.readyState === 'complete' || script.readyState === 'loaded') {
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
        script.addEventListener('load', finish, { once: true });
        script.addEventListener('error', finish, { once: true });
        script.addEventListener('readystatechange', () => {
          if (script.readyState === 'complete' || script.readyState === 'loaded') {
            finish();
          }
        });
        setTimeout(finish, SCRIPT_READY_TIMEOUT_MS);
      });
    };
    return Promise.all(Array.from(document.scripts || []).map(scriptReady));
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

pub(super) fn loaded_rendering_ready_script(expected_url: &str) -> String {
    let url = Value::String(expected_url.to_string()).to_string();
    LOADED_RENDERING_READY_SCRIPT.replace(EXPECTED_URL_PLACEHOLDER, &url)
}
