use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use std::{fs, io};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    System,
    Chinese,
    English,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub theme: Theme,
    pub language: Language,
}

impl Preferences {
    pub fn apply(self) {
        libadwaita::StyleManager::default().set_color_scheme(match self.theme {
            Theme::System => libadwaita::ColorScheme::Default,
            Theme::Light => libadwaita::ColorScheme::ForceLight,
            Theme::Dark => libadwaita::ColorScheme::ForceDark,
        });
        crate::i18n::set_language_mode(match self.language {
            Language::System => crate::i18n::LanguageMode::System,
            Language::Chinese => crate::i18n::LanguageMode::Chinese,
            Language::English => crate::i18n::LanguageMode::English,
        });
    }
}
pub fn load(path: &Path) -> io::Result<Preferences> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Preferences::default()),
        Err(error) => return Err(error),
    };
    let mut data = Vec::new();
    file.take(4097).read_to_end(&mut data)?;
    if data.len() > 4096 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "settings too large",
        ));
    }
    serde_json::from_slice(&data)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid settings"))
}
pub fn save(path: &Path, value: Preferences) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("settings directory missing"))?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)?;
    let bytes = serde_json::to_vec(&value).map_err(io::Error::other)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|e| e.error)?;
    fs::File::open(parent)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_settings_survive_restart_and_invalid_input_is_not_overwritten() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.json");
        assert_eq!(load(&path)?, Preferences::default());
        let selected = Preferences {
            theme: Theme::Dark,
            language: Language::English,
        };
        save(&path, selected)?;
        assert_eq!(load(&path)?, selected);
        fs::write(
            &path,
            br#"{"theme":"Dark","language":"English","legacy":true}"#,
        )?;
        assert_eq!(
            load(&path).err().map(|e| e.kind()),
            Some(io::ErrorKind::InvalidData)
        );
        assert!(fs::read_to_string(path)?.contains("legacy"));
        Ok(())
    }
}
