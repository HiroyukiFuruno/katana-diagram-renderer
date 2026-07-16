pub(super) const RENDERING_READY_SCRIPT: &str = r#"
new Promise(resolve => setTimeout(resolve, 0))
  .then(() => Promise.all(Array.from(document.querySelectorAll('link[rel~="stylesheet"]')).map(link => {
    if (link.sheet) {
      try {
        link.sheet.cssRules;
        return undefined;
      } catch (_) {
      }
    }
    return new Promise(resolve => {
      let resolved = false;
      const finish = () => {
        if (!resolved) {
          resolved = true;
          resolve();
        }
      };
      link.addEventListener('load', finish, { once: true });
      link.addEventListener('error', finish, { once: true });
      setTimeout(finish, 100);
    });
  })))
  .then(() => Promise.all(Array.from(document.images || []).map(image => {
    if (image.complete && image.naturalWidth > 0) {
      return image.decode ? image.decode().catch(() => undefined) : undefined;
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
      setTimeout(finish, 100);
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
        assert!(RENDERING_READY_SCRIPT.contains("link[rel~=\"stylesheet\"]"));
        assert!(RENDERING_READY_SCRIPT.contains("cssRules"));
        assert!(RENDERING_READY_SCRIPT.contains("document.images"));
        assert!(RENDERING_READY_SCRIPT.contains("image.decode"));
    }
}
