use gtk4::{gio, prelude::*};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

pub fn copy(
    source: &gio::File,
    destination: &gio::File,
    limit: u64,
    stop: &AtomicBool,
    cancellable: &gio::Cancellable,
) -> Result<(), gio::glib::Error> {
    if stop.load(Ordering::Relaxed) || cancellable.is_cancelled() {
        return Err(gio::glib::Error::new(
            gio::IOErrorEnum::Cancelled,
            "cancelled",
        ));
    }
    let input = source.read(Some(cancellable))?;
    let output = destination.replace(
        None,
        false,
        gio::FileCreateFlags::PRIVATE,
        Some(cancellable),
    )?;
    let result = (|| {
        let mut buffer = [0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            if stop.load(Ordering::Relaxed) {
                return Err(gio::glib::Error::new(
                    gio::IOErrorEnum::Cancelled,
                    "cancelled",
                ));
            }
            let count = input.read(&mut buffer, Some(cancellable))?;
            if count == 0 {
                break;
            }
            total = total.checked_add(count as u64).ok_or_else(|| {
                gio::glib::Error::new(gio::IOErrorEnum::Failed, "archive too large")
            })?;
            if total > limit {
                return Err(gio::glib::Error::new(
                    gio::IOErrorEnum::Failed,
                    "archive too large",
                ));
            }
            let (written, error) = output.write_all(&buffer[..count], Some(cancellable))?;
            if let Some(error) = error {
                return Err(error);
            }
            if written != count {
                return Err(gio::glib::Error::new(
                    gio::IOErrorEnum::Failed,
                    "short archive write",
                ));
            }
        }
        output.flush(Some(cancellable))
    })();
    if let Err(error) = result {
        cancellable.cancel();
        let _ = output.close(Some(cancellable));
        // GIO replacement discards its temporary output when closed with cancellation.
        return Err(gio::glib::Error::new(
            gio::IOErrorEnum::Failed,
            &error.to_string(),
        ));
    }
    output.close(gio::Cancellable::NONE)?;
    Ok(())
}
pub fn local(path: &Path) -> gio::File {
    gio::File::for_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_copy_does_not_replace_existing_destination()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let from = dir.path().join("source");
        let to = dir.path().join("destination");
        std::fs::write(&from, b"new data")?;
        std::fs::write(&to, b"original")?;
        assert!(
            copy(
                &local(&from),
                &local(&to),
                1024,
                &AtomicBool::new(true),
                &gio::Cancellable::new()
            )
            .is_err()
        );
        assert!(
            copy(
                &local(&from),
                &local(&to),
                1,
                &AtomicBool::new(false),
                &gio::Cancellable::new()
            )
            .is_err()
        );
        assert_eq!(std::fs::read(to)?, b"original");
        Ok(())
    }
}
