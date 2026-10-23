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

enum Backend {
    Keyring(Service),
    File(PathBuf),
}
pub struct Store {
    backend: Backend,
    attributes: HashMap<String, String>,
    fallback: Option<PathBuf>,
    label: &'static str,
    used_file: std::cell::Cell<bool>,
}
impl Store {
    pub async fn open() -> Result<Self, ()> {
        Self::open_scope(
            attributes(),
            crate::paths::config_dir()
                .ok()
                .map(|p| p.join("telegram-token")),
            "MemeDock Telegram Bot",
        )
        .await
    }
    pub async fn sync(library: &str, origin: &str, directory: Option<&Path>) -> Result<Self, ()> {
        let (attributes, name) = sync_scope(library, origin)?;
        Self::open_scope(attributes, directory.map(|p| p.join(name)), "MemeDock Sync").await
    }
    async fn open_scope(
        attributes: HashMap<String, String>,
        fallback: Option<PathBuf>,
        label: &'static str,
    ) -> Result<Self, ()> {
        let backend = match Service::encrypted().await {
            Ok(service) => Backend::Keyring(service),
            Err(error) if unavailable(&error) => Backend::File(fallback.clone().ok_or(())?),
            Err(_) => return Err(()),
        };
        Ok(Self {
            backend,
            attributes,
            fallback,
            label,
            used_file: std::cell::Cell::new(false),
        })
    }
    pub fn is_file(&self) -> bool {
        matches!(self.backend, Backend::File(_)) || self.used_file.get()
    }
    pub async fn load(&self) -> Result<Option<String>, ()> {
        match &self.backend {
            Backend::Keyring(service) => {
                let Some(collection) = service.with_alias("default").await.map_err(|_| ())? else {
                    return self.load_fallback().await;
                };
                let items = collection
                    .search_items(&self.attributes)
                    .await
                    .map_err(|_| ())?;
                let Some(item) = items.first() else {
                    return self.load_fallback().await;
                };
                item.unlock(None).await.map_err(|_| ())?;
                let secret = item.secret().await.map_err(|_| ())?;
                let value = std::str::from_utf8(secret.as_bytes()).map_err(|_| ())?;
                validate(value).map_err(|_| ())?;
                Ok(Some(value.to_owned()))
            }
            Backend::File(path) => {
                let path = path.clone();
                gtk4::gio::spawn_blocking(move || read_file(&path))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())
            }
        }
    }
    async fn load_fallback(&self) -> Result<Option<String>, ()> {
        let Some(path) = self.fallback.clone() else {
            return Ok(None);
        };
        let value = gtk4::gio::spawn_blocking(move || read_file(&path))
            .await
            .map_err(|_| ())?
            .map_err(|_| ())?;
        self.used_file.set(value.is_some());
        Ok(value)
    }
    pub async fn save(&self, value: String) -> Result<(), ()> {
        validate(&value).map_err(|_| ())?;
        match &self.backend {
            Backend::Keyring(service) => {
                let collection = collection(service).await?;
                collection
                    .create_item(
                        self.label,
                        &self.attributes,
                        Secret::text(&value),
                        true,
                        None,
                    )
                    .await
                    .map_err(|_| ())?;
                // Remove a previous current-operation fallback, so clearing the token is unambiguous.
                self.used_file.set(false);
                let Some(path) = self.fallback.clone() else {
                    return Ok(());
                };
                gtk4::gio::spawn_blocking(move || remove_file(&path))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())
            }
            Backend::File(path) => {
                let path = path.clone();
                gtk4::gio::spawn_blocking(move || write_file(&path, &value))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())
            }
        }
    }
    pub async fn clear(&self) -> Result<(), ()> {
        if let Backend::Keyring(service) = &self.backend
            && let Some(collection) = service.with_alias("default").await.map_err(|_| ())?
        {
            for item in collection
                .search_items(&self.attributes)
                .await
                .map_err(|_| ())?
            {
                item.delete(None).await.map_err(|_| ())?;
            }
        }
        let Some(path) = self.fallback.clone() else {
            return Ok(());
        };
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
fn attributes() -> HashMap<String, String> {
    HashMap::from([
        ("application", "com.grtsinry43.memedock"),
        ("purpose", "telegram-bot"),
    ])
    .into_iter()
    .map(|(key, value)| (key.into(), value.into()))
    .collect()
}
fn sync_scope(library: &str, origin: &str) -> Result<(HashMap<String, String>, String), ()> {
    let key = gtk4::glib::compute_checksum_for_data(
        gtk4::glib::ChecksumType::Sha256,
        format!("{library}\n{origin}").as_bytes(),
    )
    .ok_or(())?;
    let attributes = HashMap::from([
        ("application".into(), "com.grtsinry43.memedock".into()),
        ("purpose".into(), "sync-device".into()),
        ("library".into(), library.into()),
        ("origin".into(), origin.into()),
    ]);
    Ok((attributes, format!("sync-{key}")))
}
fn unavailable(error: &oo7::dbus::Error) -> bool {
    let error = match error {
        oo7::dbus::Error::ZBus(error)
        | oo7::dbus::Error::Service(oo7::dbus::ServiceError::ZBus(error)) => error,
        _ => return false,
    };
    if let oo7::zbus::Error::FDO(error) = error {
        return matches!(
            error.as_ref(),
            oo7::zbus::fdo::Error::ServiceUnknown(_) | oo7::zbus::fdo::Error::NameHasNoOwner(_)
        );
    }
    matches!(error, oo7::zbus::Error::MethodError(name, _, _)
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
    fn unavailable_service_is_recognized_in_both_oo7_error_envelopes() {
        use oo7::{
            dbus::{Error, ServiceError},
            zbus,
        };
        let missing =
            || zbus::Error::FDO(Box::new(zbus::fdo::Error::ServiceUnknown("missing".into())));
        assert!(unavailable(&Error::ZBus(missing())));
        assert!(unavailable(&Error::Service(ServiceError::ZBus(missing()))));
        let denied = zbus::Error::FDO(Box::new(zbus::fdo::Error::AccessDenied("locked".into())));
        assert!(!unavailable(&Error::Service(ServiceError::ZBus(denied))));
        assert!(!unavailable(&Error::Dismissed));
    }
    #[test]
    fn sync_scopes_are_isolated_and_filenames_cannot_escape_the_private_directory() -> io::Result<()>
    {
        let root = tempfile::tempdir()?;
        let (first_attributes, first) = sync_scope("library-a", "https://example.test")
            .map_err(|_| io::Error::other("scope"))?;
        let (_, other_library) = sync_scope("library-b", "https://example.test")
            .map_err(|_| io::Error::other("scope"))?;
        let (_, other_origin) = sync_scope("library-a", "https://other.test/../../token")
            .map_err(|_| io::Error::other("scope"))?;
        assert_ne!(first, other_library);
        assert_ne!(first, other_origin);
        assert_ne!(first_attributes, attributes());
        assert_eq!(Path::new(&first).components().count(), 1);
        assert_eq!(Path::new(&other_origin).components().count(), 1);
        let directory = root.path().join("credentials");
        write_file(&directory.join(&first), "first")?;
        write_file(&directory.join(&other_library), "other")?;
        remove_file(&directory.join(&first))?;
        assert_eq!(
            read_file(&directory.join(&other_library))?.as_deref(),
            Some("other")
        );
        assert_eq!(fs::metadata(&directory)?.mode() & 0o777, 0o700);
        Ok(())
    }
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
