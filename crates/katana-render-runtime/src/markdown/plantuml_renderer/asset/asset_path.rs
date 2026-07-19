use std::path::{Path, PathBuf};

const KRR_PLANTUML_CACHE_ENV: &str = "KRR_PLANTUML_CACHE_DIR";
const KDR_PLANTUML_CACHE_ENV: &str = "KDR_PLANTUML_CACHE_DIR";

pub(super) struct PlantUmlCachePathOps;

impl PlantUmlCachePathOps {
    pub(super) fn cache_root(cache_dir: Option<&Path>) -> PathBuf {
        if let Some(path) = cache_dir {
            return path.to_path_buf();
        }
        Self::env_path(KRR_PLANTUML_CACHE_ENV)
            .or_else(|| Self::env_path(KDR_PLANTUML_CACHE_ENV))
            .unwrap_or_else(Self::platform_cache_root)
    }

    #[cfg(target_os = "macos")]
    fn platform_cache_root() -> PathBuf {
        Self::platform_cache_root_from(Self::home_dir())
    }

    #[cfg(target_os = "macos")]
    fn platform_cache_root_from(home_dir: Option<PathBuf>) -> PathBuf {
        home_dir
            .map(|it| {
                it.join("Library")
                    .join("Caches")
                    .join("krr")
                    .join("plantuml")
            })
            .unwrap_or_else(Self::temp_cache_root)
    }

    #[cfg(target_os = "windows")]
    fn platform_cache_root() -> PathBuf {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| Self::home_dir().map(|it| it.join("AppData").join("Local")))
            .map(|it| it.join("krr").join("plantuml"))
            .unwrap_or_else(Self::temp_cache_root)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    fn platform_cache_root() -> PathBuf {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| Self::home_dir().map(|it| it.join(".cache")))
            .map(|it| it.join("krr").join("plantuml"))
            .unwrap_or_else(Self::temp_cache_root)
    }

    fn temp_cache_root() -> PathBuf {
        std::env::temp_dir().join("krr").join("plantuml")
    }

    fn env_path(name: &'static str) -> Option<PathBuf> {
        let value = std::env::var_os(name)?;
        (!value.is_empty()).then(|| PathBuf::from(value))
    }

    fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::PlantUmlCachePathOps;

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_cache_root_uses_temp_directory_without_home_directory() {
        let path = PlantUmlCachePathOps::platform_cache_root_from(None);

        assert!(path.ends_with("krr/plantuml"));
        assert!(path.starts_with(std::env::temp_dir()));
    }

    #[test]
    fn temp_cache_root_uses_krr_namespace() {
        let path = PlantUmlCachePathOps::temp_cache_root();

        assert!(path.ends_with("krr/plantuml"));
        assert!(path.starts_with(std::env::temp_dir()));
    }
}
