use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static CACHE_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) struct MermaidSvgCache;

impl MermaidSvgCache {
    pub(super) fn ensure_parent(cache_file: &Path) -> Result<&Path, String> {
        let Some(parent) = cache_file.parent() else {
            return Err("Mermaid cache path has no parent directory".to_string());
        };
        std::fs::create_dir_all(parent)
            .map_err(|error| Self::io_error("create parent", parent, error))?;
        Ok(parent)
    }

    pub(super) fn read(cache_file: &Path) -> Result<Option<String>, String> {
        match std::fs::read_to_string(cache_file) {
            Ok(svg) if svg.contains("<svg") && svg.contains("</svg>") => Ok(Some(svg)),
            Ok(_) => Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Self::io_error("read", cache_file, error)),
        }
    }

    pub(super) fn write(cache_file: &Path, bytes: &[u8]) -> Result<(), String> {
        let parent = Self::ensure_parent(cache_file)?;
        let sequence = CACHE_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let filename = cache_file
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("mermaid-cache.svg");
        let temporary = parent.join(format!(
            ".{filename}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        Self::write_temporary(&temporary, bytes)?;
        Self::publish(&temporary, cache_file)
    }

    pub(super) fn write_temporary(temporary: &Path, bytes: &[u8]) -> Result<(), String> {
        std::fs::write(temporary, bytes)
            .map_err(|error| Self::io_error("write temporary", temporary, error))
    }

    fn publish(temporary: &Path, cache_file: &Path) -> Result<(), String> {
        match std::fs::rename(temporary, cache_file) {
            Ok(()) => Ok(()),
            #[cfg(windows)]
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if Self::read(cache_file)?.is_some() {
                    return std::fs::remove_file(temporary)
                        .map_err(|cleanup| Self::io_error("remove temporary", temporary, cleanup));
                }
                std::fs::remove_file(cache_file)
                    .map_err(|remove| Self::io_error("remove invalid", cache_file, remove))?;
                std::fs::rename(temporary, cache_file)
                    .map_err(|rename| Self::io_error("publish", cache_file, rename))
            }
            Err(error) => {
                let _ = std::fs::remove_file(temporary);
                Err(Self::io_error("publish", cache_file, error))
            }
        }
    }

    fn io_error(operation: &str, path: &Path, error: std::io::Error) -> String {
        format!(
            "Mermaid cache {operation} failed at {}: {error}",
            path.display()
        )
    }
}
