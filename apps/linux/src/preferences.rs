use memedock_domain::export::ExportPreset;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use std::{fs, io};

#[path = "preferences/v0_1_0_to_v0_1_3.rs"]
mod v0_1_0_to_v0_1_3;

#[derive(Clone, Copy)]
pub enum OutputAction {
    Copy,
    Share,
    Drag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaDefaults {
    pub still: ExportPreset,
    pub animated: ExportPreset,
}

impl Default for MediaDefaults {
    fn default() -> Self {
        Self {
            still: ExportPreset::BestCompatibility,
            animated: ExportPreset::BestCompatibility,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputDefaults {
    pub copy: MediaDefaults,
    pub share: MediaDefaults,
    pub drag: MediaDefaults,
}

impl OutputDefaults {
    pub fn media(self, action: OutputAction) -> MediaDefaults {
        match action {
            OutputAction::Copy => self.copy,
            OutputAction::Share => self.share,
            OutputAction::Drag => self.drag,
        }
    }
    pub fn preset(self, action: OutputAction, animated: bool) -> ExportPreset {
        let media = self.media(action);
        if animated {
            media.animated
        } else {
            media.still
        }
    }
    pub fn set(&mut self, action: OutputAction, animated: bool, preset: ExportPreset) {
        let media = match action {
            OutputAction::Copy => &mut self.copy,
            OutputAction::Share => &mut self.share,
            OutputAction::Drag => &mut self.drag,
        };
        if animated {
            media.animated = preset;
        } else {
            media.still = preset;
        }
    }
    fn validate(self) -> io::Result<()> {
        for action in [OutputAction::Copy, OutputAction::Share, OutputAction::Drag] {
            for animated in [false, true] {
                if !crate::output::presets(animated).contains(&self.preset(action, animated)) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid output preset",
                    ));
                }
            }
        }
        Ok(())
    }
}

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
    pub output: OutputDefaults,
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
    let data = match read_settings(path) {
        Ok(data) => data,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Preferences::default()),
        Err(error) => return Err(error),
    };
    let (preferences, migrated) = v0_1_0_to_v0_1_3::decode(&data)?;
    if migrated {
        save(path, preferences)?;
    }
    Ok(preferences)
}

fn read_settings(path: &Path) -> io::Result<Vec<u8>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)?;
    let mut data = Vec::new();
    file.take(4097).read_to_end(&mut data)?;
    if data.len() > 4096 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "settings too large",
        ));
    }
    Ok(data)
}
pub fn save(path: &Path, value: Preferences) -> io::Result<()> {
    value.output.validate()?;
    // A failed load must not turn a later settings edit into a silent reset.
    match read_settings(path) {
        Ok(bytes) => {
            v0_1_0_to_v0_1_3::decode(&bytes)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
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
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    #[test]
    fn released_preferences_upgrade_atomically_and_keep_output_choices() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.json");
        fs::write(&path, br#"{"theme":"Dark","language":"English"}"#)?;
        let mut upgraded = load(&path)?;
        assert_eq!(upgraded.theme, Theme::Dark);
        assert_eq!(upgraded.language, Language::English);
        assert_eq!(upgraded.output, OutputDefaults::default());
        upgraded
            .output
            .set(OutputAction::Copy, true, ExportPreset::Original);
        upgraded
            .output
            .set(OutputAction::Drag, false, ExportPreset::SmallJpeg);
        save(&path, upgraded)?;
        assert_eq!(load(&path)?, upgraded);
        assert_eq!(load(&path)?, upgraded);
        let selection = crate::output::Selection {
            preferences: Rc::new(RefCell::new(upgraded)),
            temporary: Cell::new(Some(ExportPreset::WhiteBackground)),
        };
        assert_eq!(
            selection.get(OutputAction::Copy, true),
            ExportPreset::Original
        );
        assert_eq!(
            selection.get(OutputAction::Share, false),
            ExportPreset::WhiteBackground
        );
        Ok(())
    }
    #[test]
    fn current_settings_survive_restart_and_invalid_input_is_not_overwritten() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.json");
        assert_eq!(load(&path)?, Preferences::default());
        let selected = Preferences {
            theme: Theme::Dark,
            language: Language::English,
            ..Preferences::default()
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
        assert_eq!(
            save(&path, Preferences::default()).err().map(|e| e.kind()),
            Some(io::ErrorKind::InvalidData)
        );
        assert!(fs::read_to_string(&path)?.contains("legacy"));
        let mut future = serde_json::to_value(selected).map_err(io::Error::other)?;
        future["output"]["copy"]["animated"] = serde_json::json!("future_preset");
        let bytes = serde_json::to_vec(&future).map_err(io::Error::other)?;
        fs::write(&path, &bytes)?;
        assert!(load(&path).is_err());
        assert!(save(&path, selected).is_err());
        assert_eq!(fs::read(&path)?, bytes);
        Ok(())
    }
}
