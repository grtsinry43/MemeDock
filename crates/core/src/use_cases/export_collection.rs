//! Exporting collections as one archive of their originals.
//!
//! A collection is the unit people actually hand to each other, so the export
//! is a single file rather than a folder: it cannot be left half-written in a
//! directory someone else is browsing, and a partially written archive is
//! thrown away instead of leaving stray images behind.
//!
//! The archive stores bytes as they are, so whatever the image carried in —
//! EXIF included — comes back out unchanged. Images are already compressed, so
//! deflating them would spend time for a percent or two; `Stored` also keeps
//! the export honest about what it costs.
//!
//! Entry timestamps are written explicitly. A zip entry with no time falls back
//! to 1980-01-01, which is what an extractor would then stamp on every file;
//! the sticker's own creation time is the closest honest answer.
//!
//! Verification never blocks the export. An original that is missing, damaged
//! or unreadable is left out and reported, because one bad sticker is not a
//! reason to hand someone nine hundred and ninety-nine good ones.

use crate::{
    CoreError, ErrorCode, Library, ResourceLimits, Result,
    tasks::{Priority, Task, TaskControl, scheduler::Lane},
};
use memedock_domain::{
    asset::ImageFormat,
    identity::{CollectionId, ContentHash},
};
use memedock_storage::{
    files::FsBlobStore,
    queries::{PageCursor, StickerQuery, StickerSort},
};
use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// The page size the storage layer accepts as a maximum.
const PAGE_SIZE: u32 = 200;
/// Long enough for a descriptive name, short enough for every file system the
/// archive may later be unpacked on.
const MAX_NAME_CHARS: usize = 120;
/// Enough detail to act on without letting a broken collection fill the UI.
const MAX_REPORTED_ISSUES: usize = 50;

/// What to write: one collection, or every collection as one folder each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportScope {
    Collection(CollectionId),
    /// Every live collection gets its own folder in the archive. Stickers that
    /// belong to no collection are not part of any collection, so they are not
    /// part of this either.
    All,
}

/// Why one sticker did not make it into the archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportIssueReason {
    /// The library knows the sticker but its bytes are gone.
    MissingOriginal,
    /// The bytes are present but no longer match the recorded hash.
    Corrupt,
    /// The original is larger than the library's own file limit.
    TooLarge,
    /// The original could not be read at all.
    Unreadable,
}

impl ExportIssueReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingOriginal => "missing",
            Self::Corrupt => "corrupt",
            Self::TooLarge => "too_large",
            Self::Unreadable => "unreadable",
        }
    }
}

/// One sticker that was skipped, named for a person rather than a developer.
#[derive(Clone, Debug)]
pub struct ExportIssue {
    pub sticker_id: String,
    pub name: String,
    pub reason: ExportIssueReason,
}

/// What an export did, including what it could not do.
#[derive(Clone, Debug)]
pub struct CollectionExportSummary {
    pub path: PathBuf,
    pub file_name: String,
    pub byte_size: u64,
    /// Folders written; one for a single collection, one per collection for All.
    pub collections: u32,
    /// Stickers written to the archive.
    pub exported: u32,
    /// Stickers left out; equal to the sum of the reason counts below.
    pub skipped: u32,
    pub missing_original: u32,
    pub corrupt: u32,
    pub too_large: u32,
    pub unreadable: u32,
    /// At most `MAX_REPORTED_ISSUES` entries; the counts above stay complete.
    pub issues: Vec<ExportIssue>,
}

/// A sticker resolved to everything the writer needs.
struct Entry {
    sticker_id: String,
    name: String,
    /// Set only when several collections share one archive.
    folder: Option<String>,
    hash: ContentHash,
    byte_size: u64,
    created_at: i64,
    format: ImageFormat,
}

impl Library {
    /// Writes the requested collections into `destination`.
    ///
    /// `destination` is the archive the caller wants; it is replaced only once
    /// the new archive is complete, so an interrupted run never leaves a
    /// half-written file under the name the user chose.
    ///
    /// `utc_offset_minutes` is the caller's offset from UTC. Zip entries carry
    /// no zone, so the value written is the local wall-clock time the person
    /// exporting would recognise, which every extractor then reads back as-is.
    pub fn export_collections(
        &self,
        scope: ExportScope,
        destination: PathBuf,
        utc_offset_minutes: i32,
    ) -> Result<Task<CollectionExportSummary>> {
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                control.check()?;
                control.stage("collecting_collection");
                let (targets, folders) = targets(&services, scope).await?;
                let mut entries = Vec::new();
                for (folder, collection) in targets {
                    collect(&services, collection, folder, &mut entries, &control).await?;
                }
                if entries.is_empty() {
                    return Err(CoreError::new(
                        ErrorCode::InvalidInput,
                        "collection has no stickers to export",
                    ));
                }
                control.stage("writing_archive");
                let blobs = services.blobs.clone();
                let limits = services.config.limits.clone();
                tokio::task::spawn_blocking(move || {
                    write_archive(
                        &destination,
                        entries,
                        folders,
                        &blobs,
                        &limits,
                        utc_offset_minutes,
                        &control,
                    )
                })
                .await?
            },
        )
    }
}

/// Resolves the scope into collection rows, naming the folders up front so two
/// collections with the same name cannot end up sharing a directory.
async fn targets(
    services: &crate::runtime::Services,
    scope: ExportScope,
) -> Result<(Vec<(Option<String>, CollectionId)>, u32)> {
    match scope {
        ExportScope::Collection(id) => {
            let row = services
                .db
                .collection(id)
                .await?
                .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "collection not found"))?;
            if !row.lifecycle().is_active() {
                return Err(CoreError::new(
                    ErrorCode::EntityDeleted,
                    "collection deleted",
                ));
            }
            Ok((vec![(None, id)], 1))
        }
        ExportScope::All => {
            let rows = services.db.collections(false).await?;
            let mut used = HashSet::new();
            let targets: Vec<_> = rows
                .iter()
                .filter(|row| row.lifecycle().is_active())
                .map(|row| {
                    let folder = unique_folder(row.name().as_str(), &mut used);
                    (Some(folder), row.id())
                })
                .collect();
            if targets.is_empty() {
                return Err(CoreError::new(
                    ErrorCode::InvalidInput,
                    "no collections to export",
                ));
            }
            let count = targets.len() as u32;
            Ok((targets, count))
        }
    }
}

/// Reads every page of one collection into `entries`.
///
/// This runs before the archive is opened so that a selection with nothing
/// exportable fails without creating a file at all.
async fn collect(
    services: &crate::runtime::Services,
    collection: CollectionId,
    folder: Option<String>,
    entries: &mut Vec<Entry>,
    control: &TaskControl,
) -> Result<()> {
    let query = StickerQuery {
        text: String::new(),
        collection: Some(collection),
        tags: Vec::new(),
        starred: None,
        deleted: false,
        sort: StickerSort::CollectionOrder,
    };
    let mut cursor: Option<PageCursor> = None;
    loop {
        control.check()?;
        let page = services
            .db
            .stickers(&query, PAGE_SIZE, cursor.as_ref())
            .await?;
        for sticker in &page.stickers {
            let id = sticker.id();
            let name = display_name(sticker.original_name(), sticker.title());
            let created_at = sticker.lifecycle().created_at().get();
            let Some(asset) = services.db.asset(id.content_hash()).await? else {
                // The sticker row outlived its asset. Keep it in the report
                // rather than dropping it silently.
                entries.push(Entry {
                    sticker_id: id.to_string(),
                    name,
                    folder: folder.clone(),
                    hash: id.content_hash(),
                    byte_size: 0,
                    created_at,
                    format: ImageFormat::Png,
                });
                continue;
            };
            entries.push(Entry {
                sticker_id: id.to_string(),
                name,
                folder: folder.clone(),
                hash: asset.hash(),
                byte_size: u64::try_from(asset.byte_size().get())
                    .map_err(|_| CoreError::internal("asset size overflow"))?,
                created_at,
                format: asset.format(),
            });
        }
        let Some(next) = page.next else { break };
        cursor = Some(next);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_archive(
    destination: &Path,
    entries: Vec<Entry>,
    collections: u32,
    blobs: &FsBlobStore,
    limits: &ResourceLimits,
    utc_offset_minutes: i32,
    control: &TaskControl,
) -> Result<CollectionExportSummary> {
    let parent = destination.parent().ok_or_else(|| {
        CoreError::new(ErrorCode::InvalidInput, "archive has no parent directory")
    })?;
    if !parent.is_dir() {
        return Err(CoreError::new(
            ErrorCode::InvalidInput,
            "archive directory does not exist",
        ));
    }
    let temporary = temporary_path(destination);
    let result = write_entries(
        &temporary,
        &entries,
        collections,
        blobs,
        limits,
        utc_offset_minutes,
        control,
    );
    let mut summary = match result {
        Ok(summary) => summary,
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
    };
    // Rename last: until here the chosen name still holds whatever it held.
    if std::fs::rename(&temporary, destination).is_err() {
        // Windows refuses to replace an existing file; the rename above is the
        // atomic path, this is the fallback that keeps the operation possible.
        std::fs::remove_file(destination)?;
        std::fs::rename(&temporary, destination)?;
    }
    summary.path = destination.to_owned();
    summary.file_name = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export.zip".to_owned());
    Ok(summary)
}

#[allow(clippy::too_many_arguments)]
fn write_entries(
    path: &Path,
    entries: &[Entry],
    collections: u32,
    blobs: &FsBlobStore,
    limits: &ResourceLimits,
    utc_offset_minutes: i32,
    control: &TaskControl,
) -> Result<CollectionExportSummary> {
    let file = File::options().write(true).create_new(true).open(path)?;
    let mut writer = ZipWriter::new(file);
    let mut used: HashSet<String> = HashSet::new();
    let mut summary = CollectionExportSummary {
        path: path.to_owned(),
        file_name: String::new(),
        byte_size: 0,
        collections,
        exported: 0,
        skipped: 0,
        missing_original: 0,
        corrupt: 0,
        too_large: 0,
        unreadable: 0,
        issues: Vec::new(),
    };
    let mut total = 0u64;
    for entry in entries {
        control.check()?;
        if entry.byte_size == 0 {
            note(&mut summary, entry, ExportIssueReason::MissingOriginal);
            continue;
        }
        if entry.byte_size > limits.max_file_bytes {
            note(&mut summary, entry, ExportIssueReason::TooLarge);
            continue;
        }
        // Verified before the entry is started: an entry cannot be withdrawn
        // once its bytes are in the archive, so a bad original must be caught
        // here rather than half-written.
        if let Err(error) = blobs.verify(entry.hash, entry.byte_size, || control.is_cancelled()) {
            let reason = match crate::CoreError::from(error).code() {
                ErrorCode::NotFound => ExportIssueReason::MissingOriginal,
                ErrorCode::CorruptData => ExportIssueReason::Corrupt,
                ErrorCode::ResourceLimit => ExportIssueReason::TooLarge,
                ErrorCode::Cancelled => {
                    return Err(CoreError::new(ErrorCode::Cancelled, "export cancelled"));
                }
                _ => ExportIssueReason::Unreadable,
            };
            note(&mut summary, entry, reason);
            continue;
        }
        let Some(name) = unique_name(entry, &mut used) else {
            note(&mut summary, entry, ExportIssueReason::Unreadable);
            continue;
        };
        let mut source = match blobs.open_original(entry.hash) {
            Ok(file) => file,
            Err(error) => {
                let reason = match crate::CoreError::from(error).code() {
                    ErrorCode::NotFound => ExportIssueReason::MissingOriginal,
                    _ => ExportIssueReason::Unreadable,
                };
                note(&mut summary, entry, reason);
                continue;
            }
        };
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .large_file(true)
            .last_modified_time(zip_time(entry.created_at, utc_offset_minutes));
        writer.start_file(&name, options).map_err(zip_error)?;
        let mut copied = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            control.check()?;
            let count = source.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            copied += count as u64;
            if copied > entry.byte_size {
                return Err(CoreError::new(
                    ErrorCode::CorruptData,
                    "original changed during export",
                ));
            }
            writer.write_all(&buffer[..count])?;
        }
        if copied != entry.byte_size {
            // Short reads are a broken library, not a broken sticker: the same
            // cause would hit every remaining entry, so stop and say so.
            return Err(CoreError::new(
                ErrorCode::CorruptData,
                "original changed during export",
            ));
        }
        total = total.saturating_add(copied);
        if total > limits.max_archive_bytes {
            return Err(CoreError::new(
                ErrorCode::ResourceLimit,
                "collection archive exceeds the size limit",
            ));
        }
        summary.exported = summary.exported.saturating_add(1);
    }
    let file = writer.finish().map_err(zip_error)?;
    file.sync_all()?;
    summary.byte_size = file.metadata()?.len();
    memedock_storage::files::sync_directory(
        path.parent()
            .ok_or_else(|| CoreError::new(ErrorCode::Internal, "archive parent missing"))?,
    )?;
    Ok(summary)
}

fn note(summary: &mut CollectionExportSummary, entry: &Entry, reason: ExportIssueReason) {
    summary.skipped = summary.skipped.saturating_add(1);
    match reason {
        ExportIssueReason::MissingOriginal => summary.missing_original += 1,
        ExportIssueReason::Corrupt => summary.corrupt += 1,
        ExportIssueReason::TooLarge => summary.too_large += 1,
        ExportIssueReason::Unreadable => summary.unreadable += 1,
    }
    if summary.issues.len() < MAX_REPORTED_ISSUES {
        summary.issues.push(ExportIssue {
            sticker_id: entry.sticker_id.clone(),
            name: entry.name.clone(),
            reason,
        });
    }
}

fn zip_error(error: zip::result::ZipError) -> CoreError {
    CoreError::caused(ErrorCode::Io, "archive write failed", error)
}

/// Converts a millisecond timestamp into the local wall clock zip stores.
///
/// Zip has no zone field, so the value must already be local. Anything the DOS
/// format cannot represent (before 1980, after 2107) falls back to the format's
/// own default rather than aborting an export over a date.
fn zip_time(timestamp_ms: i64, utc_offset_minutes: i32) -> zip::DateTime {
    let seconds = timestamp_ms.div_euclid(1000);
    let Some(local) = seconds.checked_add(i64::from(utc_offset_minutes) * 60) else {
        return zip::DateTime::default();
    };
    let days = local.div_euclid(86_400);
    let seconds_of_day = local.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let Ok(year) = u16::try_from(year) else {
        return zip::DateTime::default();
    };
    zip::DateTime::from_date_and_time(
        year,
        month,
        day,
        (seconds_of_day / 3600) as u8,
        ((seconds_of_day % 3600) / 60) as u8,
        (seconds_of_day % 60) as u8,
    )
    .unwrap_or_default()
}

/// Howard Hinnant's `civil_from_days`, which needs no calendar crate.
fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = u64::try_from(z - era * 146_097).unwrap_or(0);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = i64::try_from(yoe).unwrap_or(0) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 {
        (mp + 3) as u8
    } else {
        (mp - 9) as u8
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// The base name a person would recognise, without its extension.
fn display_name(original_name: &str, title: &str) -> String {
    for candidate in [original_name, title] {
        if let Some(stem) = Path::new(candidate).file_stem() {
            let stem = sanitize(&stem.to_string_lossy());
            if !stem.is_empty() {
                return stem;
            }
        }
    }
    // A sticker imported from the clipboard has neither name nor title; it is
    // still worth exporting, so the writer falls back to a numbered name.
    String::new()
}

/// Makes a name safe to write on every platform the archive may land on.
fn sanitize(value: &str) -> String {
    let mut cleaned = String::with_capacity(value.len());
    for ch in value.chars() {
        let forbidden =
            matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || ch.is_control();
        cleaned.push(if forbidden { '_' } else { ch });
    }
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']).trim();
    let capped: String = trimmed.chars().take(MAX_NAME_CHARS).collect();
    let capped = capped.trim_end_matches(['.', ' ']).trim();
    if capped.is_empty() {
        return String::new();
    }
    // Reserved device names are rejected by Windows even when they have an
    // extension, so prefix rather than rename: the file stays recognisable.
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED
        .iter()
        .any(|name| capped.eq_ignore_ascii_case(name))
    {
        return format!("_{capped}");
    }
    capped.to_owned()
}

/// The entry name for one sticker, deduplicated across the whole archive.
///
/// Uniqueness is tracked per full path, so the same file name may appear once
/// in every collection folder while two in the same folder cannot collide.
fn unique_name(entry: &Entry, used: &mut HashSet<String>) -> Option<String> {
    let extension = entry.format.extension();
    let base = entry.name.as_str();
    let mut candidate = base.to_owned();
    let mut attempt = 1u32;
    loop {
        let file = if candidate.is_empty() {
            format!("sticker-{attempt}.{extension}")
        } else {
            format!("{candidate}.{extension}")
        };
        let path = match &entry.folder {
            Some(folder) => format!("{folder}/{file}"),
            None => file,
        };
        // Case-insensitive because macOS and Windows default to case-folding
        // file systems: "Meme.png" and "meme.png" cannot both be written.
        if used.insert(path.to_lowercase()) {
            return Some(path);
        }
        attempt = attempt.saturating_add(1);
        if attempt > 10_000 {
            return None;
        }
        candidate = format!("{base} ({attempt})");
    }
}

/// A folder name for one collection inside a combined archive.
fn unique_folder(name: &str, used: &mut HashSet<String>) -> String {
    let base = sanitize(name);
    let base = if base.is_empty() {
        "collection".to_owned()
    } else {
        base
    };
    let mut candidate = base.clone();
    let mut attempt = 1u32;
    loop {
        if used.insert(candidate.to_lowercase()) {
            return candidate;
        }
        attempt = attempt.saturating_add(1);
        candidate = format!("{base} ({attempt})");
    }
}

fn temporary_path(destination: &Path) -> PathBuf {
    let name = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export.zip".to_owned());
    destination.with_file_name(format!(".{name}.{}.part", uuid::Uuid::now_v7()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImportOptions, Library, LibraryConfig};
    use memedock_domain::{identity::StickerId, tag::Name};

    fn library(directory: &tempfile::TempDir) -> LibraryConfig {
        LibraryConfig::new(
            directory.path().join("data"),
            directory.path().join("cache"),
            directory.path().join("share"),
        )
    }

    async fn import(
        library: &Library,
        collection: Option<CollectionId>,
        name: &str,
        shade: u8,
    ) -> std::result::Result<StickerId, Box<dyn std::error::Error>> {
        let input = library.create_import_input()?.wait().await?;
        let mut image = image::RgbaImage::new(4, 4);
        for pixel in image.pixels_mut() {
            *pixel = image::Rgba([shade, 0, 0, 255]);
        }
        image.save_with_format(input.path(), image::ImageFormat::Png)?;
        let outcome = library
            .import_staged(
                input,
                ImportOptions {
                    original_name: name.to_owned(),
                    title: None,
                    collection,
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

    #[tokio::test]
    async fn damaged_and_missing_originals_are_reported_not_fatal()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let config = library(&directory);
        let library = Library::open(config.clone()).await?;
        let collection = library
            .create_collection(Name::new("Memes".into())?, None)?
            .wait()
            .await?;
        let good = import(&library, Some(collection.id()), "good.png", 1).await?;
        let corrupt = import(&library, Some(collection.id()), "corrupt.png", 2).await?;
        let missing = import(&library, Some(collection.id()), "missing.png", 3).await?;
        let corrupt_path = original_path(&config, corrupt);
        let length = std::fs::metadata(&corrupt_path)?.len() as usize;
        std::fs::write(&corrupt_path, vec![0u8; length])?;
        std::fs::remove_file(original_path(&config, missing))?;
        let destination = directory.path().join("memes.zip");
        let summary = library
            .export_collections(
                ExportScope::Collection(collection.id()),
                destination.clone(),
                0,
            )?
            .wait()
            .await?;
        assert_eq!(summary.exported, 1);
        assert_eq!(summary.skipped, 2);
        assert_eq!(summary.corrupt, 1);
        assert_eq!(summary.missing_original, 1);
        assert_eq!(summary.collections, 1);
        assert_eq!(summary.issues.len(), 2);
        assert!(
            !summary
                .issues
                .iter()
                .any(|issue| issue.sticker_id == good.to_string())
        );
        assert_eq!(
            summary
                .issues
                .iter()
                .find(|issue| issue.sticker_id == corrupt.to_string())
                .map(|issue| issue.reason),
            Some(ExportIssueReason::Corrupt)
        );
        assert_eq!(
            summary
                .issues
                .iter()
                .find(|issue| issue.sticker_id == missing.to_string())
                .map(|issue| issue.reason),
            Some(ExportIssueReason::MissingOriginal)
        );
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&destination)?)?;
        assert_eq!(archive.len(), 1);
        let entry = archive.by_index(0)?;
        assert_eq!(entry.name(), "good.png");
        drop(entry);
        assert!(std::fs::read_dir(directory.path())?.all(|entry| {
            !entry
                .map(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
                .unwrap_or(false)
        }));
        library.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn every_collection_gets_its_own_folder()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let library = Library::open(library(&directory)).await?;
        let cats = library
            .create_collection(Name::new("Cats".into())?, None)?
            .wait()
            .await?;
        let dogs = library
            .create_collection(Name::new("Dogs".into())?, None)?
            .wait()
            .await?;
        // Same file name in both, which is the case a flat archive would lose.
        import(&library, Some(cats.id()), "same.png", 11).await?;
        import(&library, Some(dogs.id()), "same.png", 12).await?;
        let destination = directory.path().join("all.zip");
        let summary = library
            .export_collections(ExportScope::All, destination.clone(), 0)?
            .wait()
            .await?;
        assert_eq!(summary.exported, 2);
        assert_eq!(summary.collections, 2);
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&destination)?)?;
        let mut names: Vec<String> = (0..archive.len())
            .map(|index| archive.by_index(index).map(|entry| entry.name().to_owned()))
            .collect::<std::result::Result<_, _>>()?;
        names.sort();
        assert_eq!(names, vec!["Cats/same.png", "Dogs/same.png"]);
        // Entries carry the import time rather than the 1980 zip epoch, which
        // is what an extractor would otherwise stamp on every file.
        let modified = archive
            .by_index(0)?
            .last_modified()
            .ok_or("entry has no timestamp")?;
        assert!(modified.year() >= 2024, "got {}", modified.year());
        library.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn empty_collection_creates_no_file()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let library = Library::open(library(&directory)).await?;
        let collection = library
            .create_collection(Name::new("Empty".into())?, None)?
            .wait()
            .await?;
        let destination = directory.path().join("empty.zip");
        let error = library
            .export_collections(
                ExportScope::Collection(collection.id()),
                destination.clone(),
                0,
            )?
            .wait()
            .await
            .err()
            .ok_or("expected an error")?;
        assert_eq!(error.code(), ErrorCode::InvalidInput);
        assert!(!destination.exists());
        library.close().await?;
        Ok(())
    }

    #[test]
    fn names_are_safe_unique_and_case_insensitive()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        assert_eq!(sanitize("a/b:c*d?.png"), "a_b_c_d_.png");
        assert_eq!(sanitize("  ...  "), "");
        assert_eq!(sanitize("con"), "_con");
        let mut used = HashSet::new();
        let entry = |name: &str, folder: Option<&str>| Entry {
            sticker_id: String::new(),
            name: name.to_owned(),
            folder: folder.map(str::to_owned),
            hash: ContentHash::from_bytes([0u8; 32]),
            byte_size: 1,
            created_at: 0,
            format: ImageFormat::Png,
        };
        assert_eq!(
            unique_name(&entry("Meme", None), &mut used),
            Some("Meme.png".to_owned())
        );
        assert_eq!(
            unique_name(&entry("meme", None), &mut used),
            Some("meme (2).png".to_owned())
        );
        assert_eq!(
            unique_name(&entry("Meme", Some("Cats")), &mut used),
            Some("Cats/Meme.png".to_owned())
        );
        assert_eq!(
            unique_name(&entry("", None), &mut used),
            Some("sticker-1.png".to_owned())
        );
        let mut folders = HashSet::new();
        assert_eq!(unique_folder("Cats", &mut folders), "Cats");
        assert_eq!(unique_folder("cats", &mut folders), "cats (2)");
        Ok(())
    }

    #[test]
    fn timestamps_are_local_and_never_the_zip_epoch() {
        // 2026-11-02T08:00:00Z, seen from UTC+8, is 16:00 the same day.
        let utc = 1_793_606_400_000;
        let local = zip_time(utc, 8 * 60);
        assert_eq!(local.year(), 2026);
        assert_eq!(local.month(), 11);
        assert_eq!(local.day(), 2);
        assert_eq!(local.hour(), 16);
    }
}
