use super::{BackupFile, corrupt, json, zip};
use crate::{ResourceLimits, Result, tasks::TaskControl};
use memedock_domain::{
    archive::{ArchiveData, ArchiveManifest, ArchiveOriginal, FORMAT_VERSION},
    identity::ContentHash,
    version::TimestampMs,
};
use memedock_storage::files::FsBlobStore;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

pub(crate) fn digest(bytes: &[u8]) -> ContentHash {
    ContentHash::from_bytes(Sha256::digest(bytes).into())
}

pub(crate) fn write(
    path: &Path,
    data: &ArchiveData,
    blobs: &FsBlobStore,
    limits: &ResourceLimits,
    at: TimestampMs,
    control: &TaskControl,
) -> Result<BackupFile> {
    data.validate()?;
    let metadata = encode_bounded(data, limits.max_archive_metadata_bytes)?;
    if metadata.len() as u64 > limits.max_archive_metadata_bytes
        || data.assets.len() + 2 > limits.max_archive_entries
    {
        return Err(corrupt("archive metadata exceeds limits"));
    }
    let manifest = ArchiveManifest {
        format_version: FORMAT_VERSION,
        created_at: at,
        metadata_hash: digest(&metadata),
        metadata_bytes: metadata.len() as u64,
        originals: data
            .assets
            .iter()
            .map(|v| ArchiveOriginal {
                hash: v.hash(),
                byte_size: v.byte_size().get() as u64,
            })
            .collect(),
    };
    let manifest = encode_bounded(&manifest, limits.max_archive_metadata_bytes)?;
    let total = data
        .summary()?
        .original_bytes
        .checked_add(metadata.len() as u64)
        .and_then(|v| v.checked_add(manifest.len() as u64))
        .ok_or(corrupt("archive size overflow"))?;
    if total > limits.max_archive_bytes {
        return Err(corrupt("archive exceeds byte limit"));
    }
    let file = File::options().write(true).create_new(true).open(path)?;
    let result = (|| {
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .large_file(true);
        writer.start_file("manifest.json", options).map_err(zip)?;
        writer.write_all(&manifest)?;
        writer.start_file("library.json", options).map_err(zip)?;
        writer.write_all(&metadata)?;
        for asset in &data.assets {
            control.check()?;
            control.stage("copying_backup_originals");
            let size = asset.byte_size().get() as u64;
            if size > limits.max_file_bytes {
                return Err(corrupt("archive original exceeds file limit"));
            }
            blobs.verify(asset.hash(), size, || control.is_cancelled())?;
            writer
                .start_file(format!("originals/{}", asset.hash()), options)
                .map_err(zip)?;
            let mut source = blobs.open_original(asset.hash())?;
            let mut hash = Sha256::new();
            let mut copied = 0u64;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                control.check()?;
                let count = source.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                copied += count as u64;
                if copied > size {
                    return Err(corrupt("original changed during backup"));
                }
                writer.write_all(&buffer[..count])?;
                hash.update(&buffer[..count]);
            }
            if copied != size || ContentHash::from_bytes(hash.finalize().into()) != asset.hash() {
                return Err(corrupt("original changed during backup"));
            }
        }
        control.check()?;
        let file = writer.finish().map_err(zip)?;
        file.sync_all()?;
        let byte_size = file.metadata()?.len();
        if byte_size > limits.max_archive_bytes {
            return Err(corrupt("archive exceeds byte limit"));
        }
        File::open(path.parent().ok_or(corrupt("backup parent missing"))?)?.sync_all()?;
        Ok(BackupFile {
            path: path.to_owned(),
            byte_size,
            file_name: format!("MemeDock-{}.memedock.zip", at.get()),
        })
    })();
    if result.is_err() {
        std::fs::remove_file(path)?;
    }
    result
}

fn encode_bounded(value: &impl serde::Serialize, maximum: u64) -> Result<Vec<u8>> {
    struct Bounded {
        bytes: Vec<u8>,
        maximum: u64,
        exceeded: bool,
    }
    impl Write for Bounded {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            if self.bytes.len() as u64 + buffer.len() as u64 > self.maximum {
                self.exceeded = true;
                return Err(std::io::Error::other("archive metadata exceeds budget"));
            }
            self.bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Bounded {
        bytes: Vec::new(),
        maximum,
        exceeded: false,
    };
    if let Err(error) = serde_json::to_writer(&mut writer, value) {
        return Err(if writer.exceeded {
            crate::CoreError::new(
                crate::ErrorCode::ResourceLimit,
                "archive metadata exceeds budget",
            )
        } else {
            json(error)
        });
    }
    Ok(writer.bytes)
}
