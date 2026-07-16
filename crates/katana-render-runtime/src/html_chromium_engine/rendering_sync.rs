pub(super) const RENDERING_READY_SCRIPT: &str = r#"
new Promise(resolve => setTimeout(resolve, 0))
  .then(() => {
    const DOCUMENT_READY_TIMEOUT_MS = 2000;
    if (document.readyState !== 'loading') {
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
      document.addEventListener('DOMContentLoaded', finish, { once: true });
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
    return true;
  })
"#;

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
    fn rendering_sync_waits_for_document_scripts_before_resource_paint() {
        assert!(RENDERING_READY_SCRIPT.contains("document.readyState !== 'loading'"));
        assert!(RENDERING_READY_SCRIPT.contains("DOMContentLoaded"));
        assert!(RENDERING_READY_SCRIPT.contains("DOCUMENT_READY_TIMEOUT_MS = 2000"));
    }

    #[test]
    fn rendering_sync_waits_for_stylesheet_and_image_resources() {
        assert!(RENDERING_READY_SCRIPT.contains("waitForResourceEvent"));
        assert!(RENDERING_READY_SCRIPT.contains("RESOURCE_READY_TIMEOUT_MS = 2000"));
        assert!(RENDERING_READY_SCRIPT.contains("setTimeout(finish, RESOURCE_READY_TIMEOUT_MS)"));
        assert!(RENDERING_READY_SCRIPT.contains("link[rel~=\"stylesheet\"]"));
        assert!(RENDERING_READY_SCRIPT.contains("cssRules"));
        assert!(RENDERING_READY_SCRIPT.contains("document.images"));
        assert!(RENDERING_READY_SCRIPT.contains("image.decode"));
    }
}
