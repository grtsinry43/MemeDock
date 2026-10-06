use super::{corrupt, format::digest, json, zip};
use crate::{ResourceLimits, Result, tasks::TaskControl};
use memedock_domain::{
    archive::{ArchiveData, ArchiveManifest, FORMAT_VERSION},
    identity::ContentHash,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
use zip::{CompressionMethod, ZipArchive};

pub(crate) fn inspect(
    path: &Path,
    limits: &ResourceLimits,
    control: &TaskControl,
) -> Result<(ArchiveData, ContentHash)> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > limits.max_archive_bytes
    {
        return Err(corrupt("invalid archive input"));
    }
    let mut source = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        control.check()?;
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let hash = ContentHash::from_bytes(hash.finalize().into());
    let count = preflight_directory(File::open(path)?, metadata.len(), limits, control)?;
    let mut archive = ZipArchive::new(File::open(path)?).map_err(zip)?;
    if archive.len() != count
        || archive.len() > limits.max_archive_entries
        || archive.has_overlapping_files().map_err(zip)?
    {
        return Err(corrupt("invalid archive entries"));
    }
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(zip)?;
        if entry.compression() != CompressionMethod::Stored
            || !entry.is_file()
            || entry.is_symlink()
            || entry.enclosed_name().is_none()
            || !names.insert(entry.name().to_owned())
        {
            return Err(corrupt("unsupported archive entry"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or(corrupt("archive size overflow"))?;
        if total > limits.max_archive_bytes {
            return Err(corrupt("archive exceeds byte limit"));
        }
    }
    let manifest: ArchiveManifest = serde_json::from_slice(&read_metadata(
        &mut archive,
        "manifest.json",
        limits.max_archive_metadata_bytes,
    )?)
    .map_err(json)?;
    if manifest.format_version != FORMAT_VERSION {
        return Err(crate::CoreError::new(
            crate::ErrorCode::UnsupportedFormat,
            "unsupported archive format",
        ));
    }
    let bytes = read_metadata(
        &mut archive,
        "library.json",
        limits.max_archive_metadata_bytes,
    )?;
    if bytes.len() as u64 != manifest.metadata_bytes || digest(&bytes) != manifest.metadata_hash {
        return Err(corrupt("archive metadata checksum mismatch"));
    }
    let data: ArchiveData = serde_json::from_slice(&bytes).map_err(json)?;
    data.validate()?;
    let expected: BTreeSet<_> = data
        .assets
        .iter()
        .map(|v| (v.hash(), v.byte_size().get() as u64))
        .collect();
    let declared: BTreeSet<_> = manifest
        .originals
        .iter()
        .map(|v| (v.hash, v.byte_size))
        .collect();
    if expected != declared
        || declared.len() != manifest.originals.len()
        || archive.len() != declared.len() + 2
    {
        return Err(corrupt("archive manifest mismatch"));
    }
    for (expected_hash, size) in expected {
        control.check()?;
        if size > limits.max_file_bytes {
            return Err(corrupt("archive original exceeds limit"));
        }
        let mut entry = archive
            .by_name(&format!("originals/{expected_hash}"))
            .map_err(zip)?;
        if entry.size() != size {
            return Err(corrupt("archive original length mismatch"));
        }
        let mut hash = Sha256::new();
        let mut read = 0u64;
        loop {
            control.check()?;
            let count = entry.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            read += count as u64;
            if read > size {
                return Err(corrupt("archive original exceeds declared size"));
            }
            hash.update(&buffer[..count]);
        }
        if read != size || ContentHash::from_bytes(hash.finalize().into()) != expected_hash {
            return Err(corrupt("archive original checksum mismatch"));
        }
    }
    Ok((data, hash))
}

// Bound central-directory allocation before the ZIP parser sees attacker-supplied
// entry counts or extra fields. Our current writer emits no archive comment.
fn preflight_directory(
    mut file: File,
    size: u64,
    limits: &ResourceLimits,
    control: &TaskControl,
) -> Result<usize> {
    if size < 22 {
        return Err(corrupt("truncated ZIP directory"));
    }
    file.seek(SeekFrom::End(-22))?;
    let mut end = [0u8; 22];
    file.read_exact(&mut end)?;
    let u16_at = |at: usize| u16::from_le_bytes([end[at], end[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes([end[at], end[at + 1], end[at + 2], end[at + 3]]);
    if end[..4] != [0x50, 0x4b, 0x05, 0x06]
        || u16_at(4) != 0
        || u16_at(6) != 0
        || u16_at(8) != u16_at(10)
        || u16_at(20) != 0
    {
        return Err(corrupt("unsupported ZIP directory"));
    }
    let mut count = u64::from(u16_at(10));
    let mut bytes = u64::from(u32_at(12));
    let mut directory = u64::from(u32_at(16));
    let mut directory_end = size - 22;
    if count == u16::MAX as u64 || bytes == u32::MAX as u64 || u32_at(16) == u32::MAX {
        if size < 98 {
            return Err(corrupt("truncated ZIP64 directory"));
        }
        file.seek(SeekFrom::End(-42))?;
        let mut locator = [0u8; 20];
        file.read_exact(&mut locator)?;
        if locator[..4] != [0x50, 0x4b, 0x06, 0x07]
            || locator[4..8] != [0; 4]
            || locator[16..20] != [1, 0, 0, 0]
        {
            return Err(corrupt("unsupported ZIP64 locator"));
        }
        let offset = u64::from_le_bytes(
            locator[8..16]
                .try_into()
                .map_err(|_| corrupt("ZIP64 offset"))?,
        );
        if offset != size - 98 {
            return Err(corrupt("ZIP64 offset out of bounds"));
        }
        file.seek(SeekFrom::Start(offset))?;
        let mut record = [0u8; 56];
        file.read_exact(&mut record)?;
        if record[..4] != [0x50, 0x4b, 0x06, 0x06]
            || record[4..12] != 44u64.to_le_bytes()
            || record[16..24] != [0; 8]
            || record[24..32] != record[32..40]
        {
            return Err(corrupt("unsupported ZIP64 directory"));
        }
        count = u64::from_le_bytes(
            record[32..40]
                .try_into()
                .map_err(|_| corrupt("ZIP64 count"))?,
        );
        bytes = u64::from_le_bytes(
            record[40..48]
                .try_into()
                .map_err(|_| corrupt("ZIP64 size"))?,
        );
        directory = u64::from_le_bytes(
            record[48..56]
                .try_into()
                .map_err(|_| corrupt("ZIP64 directory offset"))?,
        );
        directory_end = offset;
    }
    if count > limits.max_archive_entries as u64 || bytes > limits.max_archive_metadata_bytes {
        return Err(corrupt("ZIP directory exceeds memory budget"));
    }
    if directory.checked_add(bytes) != Some(directory_end) {
        return Err(corrupt("ZIP directory range mismatch"));
    }
    file.seek(SeekFrom::Start(directory))?;
    let mut consumed = 0u64;
    for _ in 0..count {
        control.check()?;
        if bytes - consumed < 46 {
            return Err(corrupt("truncated ZIP entry directory"));
        }
        let mut header = [0u8; 46];
        file.read_exact(&mut header)?;
        if header[..4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err(corrupt("invalid ZIP entry directory"));
        }
        let name = u64::from(u16::from_le_bytes([header[28], header[29]]));
        let extra = u64::from(u16::from_le_bytes([header[30], header[31]]));
        let comment = u64::from(u16::from_le_bytes([header[32], header[33]]));
        consumed = consumed
            .checked_add(46 + name + extra + comment)
            .ok_or(corrupt("ZIP directory overflow"))?;
        if consumed > bytes {
            return Err(corrupt("ZIP directory exceeds declared budget"));
        }
        file.seek(SeekFrom::Start(directory + consumed))?;
    }
    if consumed != bytes {
        return Err(corrupt("ZIP directory has unaccounted data"));
    }
    usize::try_from(count).map_err(|_| corrupt("ZIP entry count overflow"))
}
fn read_metadata(archive: &mut ZipArchive<File>, name: &str, maximum: u64) -> Result<Vec<u8>> {
    let entry = archive.by_name(name).map_err(zip)?;
    if entry.size() > maximum {
        return Err(corrupt("archive metadata exceeds limit"));
    }
    let mut bytes = Vec::new();
    entry.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(corrupt("archive metadata exceeds limit"));
    }
    Ok(bytes)
}
