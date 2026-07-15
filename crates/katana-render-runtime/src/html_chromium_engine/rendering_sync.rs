pub(super) const RENDERING_READY_SCRIPT: &str = r#"
new Promise(resolve => setTimeout(resolve, 0))
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
}
