//! Exporting collections as one archive of their originals.
//!
//! Both ways into this feature — one collection from its card menu, every
//! collection from the page header — build the same request and go through
//! `present`, so the destination picker, the local time zone and the report
//! exist once. The archive itself is written by the core: it is the only side
//! that can read originals and tell which of them are still intact. What comes
//! back is a report rather than a plain success, because an export that had to
//! skip something still produced a usable file.

use crate::i18n::{self, Key};
use gtk4::{glib, prelude::*};
use memedock_core::{CollectionExportSummary, ErrorCode, ExportScope, Library};
use memedock_domain::identity::CollectionId;
use std::{cell::Cell, path::PathBuf, rc::Rc};

pub type Notify = Rc<dyn Fn(&str)>;

thread_local! {
    /// One export at a time: two archives would compete for the same originals,
    /// and the second report would overwrite the first one on screen.
    static RUNNING: Cell<bool> = const { Cell::new(false) };
}

/// Everything the shared path needs to describe one export.
struct Request {
    scope: ExportScope,
    title: Key,
    file_name: String,
    /// What "there was nothing to write" means for this entry point.
    empty: Key,
}

impl Request {
    fn collection(id: CollectionId, name: &str) -> Self {
        Self {
            scope: ExportScope::Collection(id),
            title: Key::ExportCollection,
            file_name: format!("{name}.zip"),
            empty: Key::CollectionEmpty,
        }
    }

    fn all() -> Self {
        Self {
            scope: ExportScope::All,
            title: Key::ExportAllCollections,
            file_name: i18n::text(Key::ExportAllFileName).to_owned(),
            empty: Key::ExportNothing,
        }
    }
}

/// Exports one collection, named after it.
pub fn present_collection(
    parent: &impl IsA<gtk4::Widget>,
    library: Library,
    id: CollectionId,
    name: &str,
    notify: Notify,
) {
    present(parent, library, Request::collection(id, name), notify);
}

/// Exports every live collection, one folder each in a single archive.
pub fn present_all(parent: &impl IsA<gtk4::Widget>, library: Library, notify: Notify) {
    present(parent, library, Request::all(), notify);
}

fn present(parent: &impl IsA<gtk4::Widget>, library: Library, request: Request, notify: Notify) {
    // Claim the slot before the picker opens: a second export could
    // otherwise be chosen while the first one is still being written.
    if RUNNING.replace(true) {
        notify(i18n::text(Key::ExportRunning));
        return;
    }
    let Some(window) = parent.root().and_downcast::<gtk4::Window>() else {
        RUNNING.set(false);
        notify(i18n::text(Key::FailureUnknown));
        return;
    };
    let dialog = gtk4::FileDialog::new();
    i18n::bind(&dialog, "title", request.title);
    dialog.set_initial_name(Some(&request.file_name));
    glib::spawn_future_local(async move {
        let chosen = match dialog.save_future(Some(&window)).await {
            Ok(file) => file,
            Err(error)
                if error.matches(gtk4::DialogError::Dismissed)
                    || error.matches(gtk4::DialogError::Cancelled) =>
            {
                RUNNING.set(false);
                return;
            }
            Err(_) => {
                RUNNING.set(false);
                notify(i18n::text(Key::FailureIo));
                return;
            }
        };
        let Some(destination) = chosen.path() else {
            RUNNING.set(false);
            notify(i18n::text(Key::FailureIo));
            return;
        };
        start(library, request.scope, request.empty, destination, notify);
    });
}

fn start(library: Library, scope: ExportScope, empty: Key, destination: PathBuf, notify: Notify) {
    let offset = match glib::DateTime::now_local() {
        Ok(now) => offset_minutes(&now),
        // Writing UTC times under the user's chosen name would be wrong for
        // every extractor on this machine, so stop instead of guessing.
        Err(_) => {
            RUNNING.set(false);
            notify(i18n::text(Key::FailureUnknown));
            return;
        }
    };
    glib::spawn_future_local(async move {
        let result = export(&library, scope, destination, offset).await;
        RUNNING.set(false);
        match result {
            Ok(summary) => notify(&report(&summary)),
            Err(error) => match failure(error.code()) {
                Failure::Silent => {}
                Failure::NothingToWrite => notify(i18n::text(empty)),
                Failure::Reported(code) => notify(i18n::core(code)),
            },
        }
    });
}

/// How one export failure should reach the person who asked for it.
#[derive(Debug, PartialEq, Eq)]
enum Failure {
    /// The caller's own decision; the partial archive was already discarded.
    Silent,
    /// Asking found nothing to write. The core cannot know which of the two
    /// entry points was used, so the caller supplies the wording.
    NothingToWrite,
    Reported(ErrorCode),
}

fn failure(code: ErrorCode) -> Failure {
    match code {
        ErrorCode::Cancelled => Failure::Silent,
        ErrorCode::InvalidInput => Failure::NothingToWrite,
        code => Failure::Reported(code),
    }
}

/// Writes the archive through the core, which owns reading and verifying the
/// originals. `utc_offset_minutes` is the caller's offset from UTC.
async fn export(
    library: &Library,
    scope: ExportScope,
    destination: PathBuf,
    utc_offset_minutes: i32,
) -> memedock_core::Result<CollectionExportSummary> {
    let task = library.export_collections(scope, destination, utc_offset_minutes)?;
    task.wait().await
}

/// Names only the reasons that actually occurred, so the line stays readable
/// when one sticker out of a thousand was missing.
pub(crate) fn report(summary: &CollectionExportSummary) -> String {
    if summary.skipped == 0 {
        return i18n::export_done(summary.exported);
    }
    let mut reasons = Vec::new();
    if summary.missing_original > 0 {
        reasons.push(i18n::export_issue_missing(summary.missing_original));
    }
    if summary.corrupt > 0 {
        reasons.push(i18n::export_issue_corrupt(summary.corrupt));
    }
    if summary.too_large > 0 {
        reasons.push(i18n::export_issue_too_large(summary.too_large));
    }
    if summary.unreadable > 0 {
        reasons.push(i18n::export_issue_unreadable(summary.unreadable));
    }
    i18n::export_partial(
        summary.exported,
        summary.skipped,
        &reasons.join(i18n::export_issue_separator()),
    )
}

/// Zip stores wall-clock time and has no zone field, so the core needs the
/// caller's offset. Sub-minute historical offsets cannot be represented; they
/// truncate toward zero exactly like the macOS client.
fn offset_minutes(now: &glib::DateTime) -> i32 {
    i32::try_from(now.utc_offset().as_microseconds() / 60_000_000).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use memedock_core::{ImportOptions, LibraryConfig};
    use memedock_domain::{identity::StickerId, tag::Name};
    use std::{
        error::Error,
        fs::File,
        path::{Path, PathBuf},
    };

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    fn config(root: &Path) -> LibraryConfig {
        LibraryConfig::new(root.join("data"), root.join("cache"), root.join("export"))
    }

    async fn import(
        library: &Library,
        collection: CollectionId,
        name: &str,
        shade: u8,
    ) -> TestResult<StickerId> {
        let input = library.create_import_input()?.wait().await?;
        image::RgbaImage::from_pixel(3, 2, image::Rgba([shade, 43, 91, 255]))
            .save_with_format(input.path(), image::ImageFormat::Png)?;
        let outcome = library
            .import_staged(
                input,
                ImportOptions {
                    original_name: format!("{name}.png"),
                    title: None,
                    collection: Some(collection),
                },
            )?
            .wait()
            .await?;
        Ok(outcome.sticker.id())
    }

    fn original_path(config: &LibraryConfig, id: StickerId) -> PathBuf {
        let hash = id.content_hash().to_string();
        config
            .data_dir
            .join("blobs")
            .join(&hash[..2])
            .join(&hash[2..4])
            .join(&hash)
    }

    fn entry_names(path: &Path) -> TestResult<Vec<String>> {
        let mut archive = zip::ZipArchive::new(File::open(path)?)?;
        (0..archive.len())
            .map(|index| {
                archive
                    .by_index(index)
                    .map(|entry| entry.name().to_owned())
                    .map_err(Into::into)
            })
            .collect()
    }

    fn entry_hour(path: &Path) -> TestResult<i32> {
        let mut archive = zip::ZipArchive::new(File::open(path)?)?;
        let entry = archive.by_index(0)?;
        let modified = entry.last_modified().ok_or("entry has no timestamp")?;
        Ok(i32::from(modified.hour()))
    }

    #[test]
    fn offsets_are_whole_minutes_in_both_directions() -> TestResult {
        let east = glib::TimeZone::from_offset(8 * 3600);
        let at = glib::DateTime::new(&east, 2026, 11, 2, 8, 0, 0.0)?;
        assert_eq!(offset_minutes(&at), 480);
        let west = glib::TimeZone::from_offset(-(3 * 3600 + 30 * 60));
        let at = glib::DateTime::new(&west, 2026, 11, 2, 8, 0, 0.0)?;
        assert_eq!(offset_minutes(&at), -210);
        Ok(())
    }

    #[test]
    fn both_entries_describe_the_same_kind_of_request() {
        let id = CollectionId::new();
        let one = Request::collection(id, "Cats");
        assert_eq!(one.scope, ExportScope::Collection(id));
        assert_eq!(one.title, Key::ExportCollection);
        assert_eq!(one.file_name, "Cats.zip");
        assert_eq!(one.empty, Key::CollectionEmpty);
        let all = Request::all();
        assert_eq!(all.scope, ExportScope::All);
        assert_eq!(all.title, Key::ExportAllCollections);
        assert_eq!(all.file_name, i18n::text(Key::ExportAllFileName));
        assert_eq!(all.empty, Key::ExportNothing);
    }

    #[test]
    fn cancelling_stays_silent_and_an_empty_scope_speaks_for_its_entry() {
        assert_eq!(failure(ErrorCode::Cancelled), Failure::Silent);
        assert_eq!(failure(ErrorCode::InvalidInput), Failure::NothingToWrite);
        assert_eq!(
            failure(ErrorCode::NotFound),
            Failure::Reported(ErrorCode::NotFound)
        );
    }

    #[test]
    fn the_report_names_only_the_reasons_that_occurred() {
        let mut summary = CollectionExportSummary {
            path: PathBuf::new(),
            file_name: String::new(),
            byte_size: 0,
            collections: 2,
            exported: 3,
            skipped: 2,
            missing_original: 1,
            corrupt: 1,
            too_large: 0,
            unreadable: 0,
            issues: Vec::new(),
        };
        let message = report(&summary);
        assert!(message.contains(&i18n::export_issue_missing(1)));
        assert!(message.contains(&i18n::export_issue_corrupt(1)));
        assert!(!message.contains(&i18n::export_issue_too_large(1)));
        assert!(!message.contains(&i18n::export_issue_unreadable(1)));
        assert_eq!(
            message,
            i18n::export_partial(
                3,
                2,
                &[i18n::export_issue_missing(1), i18n::export_issue_corrupt(1),]
                    .join(i18n::export_issue_separator())
            )
        );
        summary.skipped = 0;
        summary.missing_original = 0;
        summary.corrupt = 0;
        assert_eq!(report(&summary), i18n::export_done(3));
    }

    #[test]
    fn exporting_uses_the_real_core_and_reports_what_it_skipped() -> TestResult {
        let root = tempfile::tempdir()?;
        let context = glib::MainContext::default();
        let library = context.block_on(Library::open(config(root.path())))?;
        let result = context.block_on(async {
            let cats = library
                .create_collection(Name::new("Cats".into())?, None)?
                .wait()
                .await?;
            let dogs = library
                .create_collection(Name::new("Dogs".into())?, None)?
                .wait()
                .await?;
            let empty = library
                .create_collection(Name::new("Empty".into())?, None)?
                .wait()
                .await?;
            let gone = library
                .create_collection(Name::new("Gone".into())?, None)?
                .wait()
                .await?;
            import(&library, cats.id(), "one", 11).await?;
            import(&library, dogs.id(), "two", 12).await?;

            // One collection, named after it, into the chosen file.
            let local = root.path().join("cats.zip");
            let summary = export(
                &library,
                ExportScope::Collection(cats.id()),
                local.clone(),
                8 * 60,
            )
            .await?;
            assert_eq!(summary.exported, 1);
            assert_eq!(summary.collections, 1);
            assert_eq!(summary.skipped, 0);
            assert_eq!(entry_names(&local)?, ["one.png"]);

            // The same sticker exported with two offsets must show wall-clock
            // times eight hours apart: the value reached the core.
            let utc = root.path().join("cats-utc.zip");
            export(&library, ExportScope::Collection(cats.id()), utc.clone(), 0).await?;
            let shift =
                (entry_hour(&root.path().join("cats.zip"))? - entry_hour(&utc)?).rem_euclid(24);
            assert_eq!(shift, 8);

            // An empty collection writes nothing at all.
            let nothing = root.path().join("empty.zip");
            let error = export(
                &library,
                ExportScope::Collection(empty.id()),
                nothing.clone(),
                0,
            )
            .await
            .err()
            .ok_or("expected an error")?;
            assert_eq!(error.code(), ErrorCode::InvalidInput);
            assert!(!nothing.exists());

            // A collection deleted while the picker was open is reported, not
            // silently written.
            library
                .delete_collection(gone.id(), gone.lifecycle().generation())?
                .wait()
                .await?;
            let error = export(
                &library,
                ExportScope::Collection(gone.id()),
                root.path().join("gone.zip"),
                0,
            )
            .await
            .err()
            .ok_or("expected an error")?;
            assert!(matches!(
                error.code(),
                ErrorCode::NotFound | ErrorCode::EntityDeleted
            ));

            // Every collection gets its own folder in one archive.
            let all = root.path().join("all.zip");
            let summary = export(&library, ExportScope::All, all.clone(), 0).await?;
            assert_eq!(summary.exported, 2);
            let mut names = entry_names(&all)?;
            names.sort();
            assert_eq!(names, ["Cats/one.png", "Dogs/two.png"]);

            // One damaged original is skipped and named, not fatal.
            import(&library, cats.id(), "keep", 13).await?;
            let broken = import(&library, cats.id(), "torn", 15).await?;
            std::fs::remove_file(original_path(&config(root.path()), broken))?;
            let partial = root.path().join("partial.zip");
            let summary = export(
                &library,
                ExportScope::Collection(cats.id()),
                partial.clone(),
                0,
            )
            .await?;
            assert_eq!(summary.exported, 2);
            assert_eq!(summary.skipped, 1);
            assert_eq!(summary.missing_original, 1);
            assert_eq!(summary.issues.len(), 1);
            assert_eq!(summary.issues[0].sticker_id, broken.to_string());
            let mut names = entry_names(&partial)?;
            names.sort();
            assert_eq!(names, ["keep.png", "one.png"]);
            let message = report(&summary);
            assert_eq!(
                message,
                i18n::export_partial(2, 1, &i18n::export_issue_missing(1))
            );
            Ok::<_, Box<dyn Error>>(())
        });
        context.block_on(library.close())?;
        result
    }
}
