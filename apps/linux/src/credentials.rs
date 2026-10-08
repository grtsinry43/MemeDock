use oo7::{
    Secret,
    dbus::{Collection, Service},
};
use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub enum Store {
    Keyring(Service),
    File(PathBuf),
}
impl Store {
    pub async fn open() -> Result<Self, ()> {
        match Service::encrypted().await {
            Ok(service) => Ok(Self::Keyring(service)),
            Err(error) if unavailable(&error) => crate::paths::config_dir()
                .map(|p| Self::File(p.join("telegram-token")))
                .map_err(|_| ()),
            Err(_) => Err(()),
        }
    }
    pub fn is_file(&self) -> bool {
        matches!(self, Self::File(_))
    }
    pub async fn load(&self) -> Result<Option<String>, ()> {
        match self {
            Self::Keyring(service) => {
                let Some(collection) = service.with_alias("default").await.map_err(|_| ())? else {
                    return Ok(None);
                };
                let items = collection
                    .search_items(&attributes())
                    .await
                    .map_err(|_| ())?;
                let Some(item) = items.first() else {
                    return Ok(None);
                };
                item.unlock(None).await.map_err(|_| ())?;
                let secret = item.secret().await.map_err(|_| ())?;
                let value = std::str::from_utf8(secret.as_bytes()).map_err(|_| ())?;
                validate(value).map_err(|_| ())?;
                Ok(Some(value.to_owned()))
            }
            Self::File(path) => {
                let path = path.clone();
                gtk4::gio::spawn_blocking(move || read_file(&path))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())
            }
        }
    }
    pub async fn save(&self, value: String) -> Result<(), ()> {
        validate(&value).map_err(|_| ())?;
        match self {
            Self::Keyring(service) => {
                let collection = collection(service).await?;
                collection
                    .create_item(
                        "MemeDock Telegram Bot",
                        &attributes(),
                        Secret::text(&value),
                        true,
                        None,
                    )
                    .await
                    .map_err(|_| ())?;
                // Remove a previous current-operation fallback, so clearing the token is unambiguous.
                let path = crate::paths::config_dir()
                    .map_err(|_| ())?
                    .join("telegram-token");
                gtk4::gio::spawn_blocking(move || remove_file(&path))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())
            }
            Self::File(path) => {
                let path = path.clone();
                gtk4::gio::spawn_blocking(move || write_file(&path, &value))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())
            }
        }
    }
    pub async fn clear(&self) -> Result<(), ()> {
        if let Self::Keyring(service) = self
            && let Some(collection) = service.with_alias("default").await.map_err(|_| ())?
        {
            for item in collection
                .search_items(&attributes())
                .await
                .map_err(|_| ())?
            {
                item.delete(None).await.map_err(|_| ())?;
            }
        }
        let path = crate::paths::config_dir()
            .map_err(|_| ())?
            .join("telegram-token");
        gtk4::gio::spawn_blocking(move || remove_file(&path))
            .await
            .map_err(|_| ())?
            .map_err(|_| ())
    }
}
async fn collection(service: &Service) -> Result<Collection, ()> {
    let collection = service.default_collection().await.map_err(|_| ())?;
    collection.unlock(None).await.map_err(|_| ())?;
    Ok(collection)
}
fn attributes() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("application", "com.grtsinry43.memedock"),
        ("purpose", "telegram-bot"),
    ])
}
fn unavailable(error: &oo7::dbus::Error) -> bool {
    if let oo7::dbus::Error::ZBus(oo7::zbus::Error::FDO(error)) = error {
        return matches!(
            error.as_ref(),
            oo7::zbus::fdo::Error::ServiceUnknown(_) | oo7::zbus::fdo::Error::NameHasNoOwner(_)
        );
    }
    matches!(error, oo7::dbus::Error::ZBus(oo7::zbus::Error::MethodError(name, _, _))
        if matches!(name.as_str(), "org.freedesktop.DBus.Error.ServiceUnknown" | "org.freedesktop.DBus.Error.NameHasNoOwner"))
}
fn validate(value: &str) -> io::Result<()> {
    if value.is_empty() || value.len() > 256 || value.contains(['\n', '\r', '\0']) {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid credential",
        ))
    } else {
        Ok(())
    }
}
fn read_file(path: &Path) -> io::Result<Option<String>> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "credential permissions too wide",
        ));
    }
    let mut value = String::new();
    file.take(257).read_to_string(&mut value)?;
    validate(&value)?;
    Ok(Some(value))
}
fn write_file(path: &Path, value: &str) -> io::Result<()> {
    validate(value)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("credential directory missing"))?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)?;
    let meta = fs::symlink_metadata(parent)?;
    if !meta.is_dir() || meta.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "credential directory permissions too wide",
        ));
    }
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    temp.write_all(value.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    fs::File::open(parent)?.sync_all()
}
fn remove_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => {
            if let Some(parent) = path.parent() {
                fs::File::open(parent)?.sync_all()?;
            }
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_credentials_are_private_and_reject_symlinks_and_wide_permissions() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("private").join("telegram-token");
        write_file(&path, "token")?;
        assert_eq!(read_file(&path)?, Some("token".into()));
        assert_eq!(fs::metadata(&path)?.mode() & 0o777, 0o600);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
        assert!(read_file(&path).is_err());
        remove_file(&path)?;
        std::os::unix::fs::symlink("missing", &path)?;
        assert!(read_file(&path).is_err());
        remove_file(&path)?;
        assert_eq!(read_file(&path)?, None);
        Ok(())
    }
}
