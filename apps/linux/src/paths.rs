use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryPaths {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub export_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    HomeMissing,
}

impl PathError {
    pub fn message(self) -> &'static str {
        match self {
            Self::HomeMissing => crate::i18n::text(crate::i18n::Key::NoHome),
        }
    }
}

pub fn from_env() -> Result<LibraryPaths, PathError> {
    let data_home = std::env::var("XDG_DATA_HOME").ok();
    let cache_home = std::env::var("XDG_CACHE_HOME").ok();
    let home = std::env::var("HOME").ok();
    library_paths(data_home.as_deref(), cache_home.as_deref(), home.as_deref())
}

pub fn library_paths(
    data_home: Option<&str>,
    cache_home: Option<&str>,
    home: Option<&str>,
) -> Result<LibraryPaths, PathError> {
    let home = home.filter(|value| !value.is_empty());
    let data_base = absolute_base(data_home, home, ".local/share")?;
    let cache_base = absolute_base(cache_home, home, ".local/cache")?;
    Ok(LibraryPaths {
        data_dir: data_base.join("sticker-library"),
        cache_dir: cache_base.join("sticker-library"),
        export_dir: data_base.join("sticker-exports"),
    })
}

fn absolute_base(
    configured: Option<&str>,
    home: Option<&str>,
    fallback: &str,
) -> Result<PathBuf, PathError> {
    if let Some(path) = absolute_xdg(configured) {
        return Ok(path);
    }
    let home = home.ok_or(PathError::HomeMissing)?;
    let home = Path::new(home);
    if !home.is_absolute() {
        return Err(PathError::HomeMissing);
    }
    Ok(home.join(fallback))
}

fn absolute_xdg(value: Option<&str>) -> Option<PathBuf> {
    let value = value.filter(|value| !value.is_empty())?;
    let path = PathBuf::from(value);
    if path.is_absolute() { Some(path) } else { None }
}

#[cfg(test)]
mod tests {
    use super::{LibraryPaths, PathError, library_paths};
    use std::path::PathBuf;

    fn opened(
        data_home: Option<&str>,
        cache_home: Option<&str>,
        home: Option<&str>,
    ) -> LibraryPaths {
        match library_paths(data_home, cache_home, home) {
            Ok(paths) => paths,
            Err(error) => panic!("{}", error.message()),
        }
    }

    #[test]
    fn absolute_xdg_directories_are_siblings() {
        let paths = opened(Some("/data"), Some("/cache"), None);
        assert_eq!(paths.data_dir, PathBuf::from("/data/sticker-library"));
        assert_eq!(paths.cache_dir, PathBuf::from("/cache/sticker-library"));
        assert_eq!(paths.export_dir, PathBuf::from("/data/sticker-exports"));
        assert!(!paths.export_dir.starts_with(&paths.data_dir));
        assert!(!paths.data_dir.starts_with(&paths.export_dir));
    }

    #[test]
    fn missing_xdg_uses_home_defaults() {
        let paths = opened(None, None, Some("/home/user"));
        assert_eq!(
            paths.data_dir,
            PathBuf::from("/home/user/.local/share/sticker-library")
        );
        assert_eq!(
            paths.cache_dir,
            PathBuf::from("/home/user/.local/cache/sticker-library")
        );
        assert_eq!(
            paths.export_dir,
            PathBuf::from("/home/user/.local/share/sticker-exports")
        );
    }

    #[test]
    fn empty_and_relative_xdg_fall_back_to_home() {
        let paths = opened(Some(""), Some("relative/cache"), Some("/home/user"));
        assert_eq!(
            paths.data_dir,
            PathBuf::from("/home/user/.local/share/sticker-library")
        );
        assert_eq!(
            paths.cache_dir,
            PathBuf::from("/home/user/.local/cache/sticker-library")
        );
    }

    #[test]
    fn a_relative_cache_still_needs_an_absolute_home() {
        assert_eq!(
            library_paths(Some("/data"), Some(""), None),
            Err(PathError::HomeMissing)
        );
        assert_eq!(
            library_paths(None, Some("cache"), Some("relative-home")),
            Err(PathError::HomeMissing)
        );
    }
}
