//! The initial release through 0.1.2 stored only appearance and language.
//! Decode before replacing the file; the caller publishes via atomic rename.
use super::{Language, Preferences, Theme};
use serde::Deserialize;
use std::io;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Legacy {
    theme: Theme,
    language: Language,
}

pub(super) fn decode(bytes: &[u8]) -> io::Result<(Preferences, bool)> {
    if let Ok(current) = serde_json::from_slice::<Preferences>(bytes) {
        current.output.validate()?;
        return Ok((current, false));
    }
    let old: Legacy = serde_json::from_slice(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid settings"))?;
    Ok((
        Preferences {
            theme: old.theme,
            language: old.language,
            ..Preferences::default()
        },
        true,
    ))
}
