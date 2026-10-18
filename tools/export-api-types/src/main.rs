use memedock_api_contracts::*;
use std::{io::Write, path::PathBuf};
use ts_rs::{Config, TS};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: memedock-export-api-types <output.ts>")?;
    let cfg = Config::default();
    let declarations = [
        ApiMeta::decl(&cfg),
        ApiEnvelope::<String>::decl(&cfg),
        ApiErrorKind::decl(&cfg),
        LoginRequest::decl(&cfg),
        ChangePasswordRequest::decl(&cfg),
        AdminInfo::decl(&cfg),
        CreateDeviceRequest::decl(&cfg),
        DeviceInfo::decl(&cfg),
        DeviceList::decl(&cfg),
        DeviceCredential::decl(&cfg),
        SyncInfo::decl(&cfg),
        LibraryAsset::decl(&cfg),
        LibraryQuery::decl(&cfg),
        FacetQuery::decl(&cfg),
        ActivityQuery::decl(&cfg),
        LibrarySticker::decl(&cfg),
        LibraryPage::decl(&cfg),
        LibraryFacet::decl(&cfg),
        LibraryFacets::decl(&cfg),
        LibraryDetail::decl(&cfg),
        ActivityResult::decl(&cfg),
        DeviceSyncProgress::decl(&cfg),
        SyncOverview::decl(&cfg),
        ActivityEntry::decl(&cfg),
        ActivityPage::decl(&cfg),
    ];
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(destination)?;
    writeln!(file, "// Generated from Rust HTTP contracts. DO NOT EDIT.")?;
    for declaration in declarations {
        writeln!(file, "export {declaration}")?;
    }
    Ok(())
}
