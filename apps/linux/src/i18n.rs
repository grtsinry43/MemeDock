use memedock_core::ErrorCode;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Zh,
    En,
}

/// Same three modes as the Android language setting. System follows the process locale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanguageMode {
    System,
    Chinese,
    English,
}

const MODE_SYSTEM: u8 = 0;
const MODE_CHINESE: u8 = 1;
const MODE_ENGLISH: u8 = 2;

static MODE: AtomicU8 = AtomicU8::new(MODE_SYSTEM);

pub fn set_language_mode(mode: LanguageMode) {
    let value = match mode {
        LanguageMode::System => MODE_SYSTEM,
        LanguageMode::Chinese => MODE_CHINESE,
        LanguageMode::English => MODE_ENGLISH,
    };
    MODE.store(value, Ordering::Relaxed);
    refresh_bindings();
}

use gtk4::glib;
use gtk4::prelude::*;
use std::cell::RefCell;
struct Binding {
    object: glib::WeakRef<glib::Object>,
    property: &'static str,
    key: Key,
    last: String,
}
struct Listener {
    object: glib::WeakRef<glib::Object>,
    update: Box<dyn Fn()>,
}
thread_local! {
    static BINDINGS: RefCell<Vec<Binding>> = const { RefCell::new(Vec::new()) };
    static LISTENERS: RefCell<Vec<Listener>> = const { RefCell::new(Vec::new()) };
}
pub fn bind(object: &impl IsA<glib::Object>, property: &'static str, key: Key) {
    let object = object.upcast_ref::<glib::Object>();
    object.set_property(property, text(key));
    BINDINGS.with(|bindings| {
        let mut bindings = bindings.borrow_mut();
        bindings.retain(|b| {
            b.object
                .upgrade()
                .is_some_and(|o| o != *object || b.property != property)
        });
        bindings.push(Binding {
            object: object.downgrade(),
            property,
            key,
            last: text(key).into(),
        });
    });
}
pub fn on_language(object: &impl IsA<glib::Object>, update: impl Fn() + 'static) {
    LISTENERS.with(|listeners| {
        listeners.borrow_mut().push(Listener {
            object: object.upcast_ref::<glib::Object>().downgrade(),
            update: Box::new(update),
        })
    });
}
fn refresh_bindings() {
    let mut bindings = BINDINGS.with(|v| std::mem::take(&mut *v.borrow_mut()));
    bindings.retain_mut(|binding| {
        let Some(object) = binding.object.upgrade() else {
            return false;
        };
        // A runtime value such as an album name takes ownership of this property.
        if object
            .property::<Option<String>>(binding.property)
            .as_deref()
            != Some(&binding.last)
        {
            return false;
        }
        binding.last = text(binding.key).into();
        object.set_property(binding.property, &binding.last);
        true
    });
    BINDINGS.with(|v| v.borrow_mut().extend(bindings));
    let mut listeners = LISTENERS.with(|v| std::mem::take(&mut *v.borrow_mut()));
    listeners.retain(|listener| {
        if listener.object.upgrade().is_none() {
            return false;
        }
        (listener.update)();
        true
    });
    LISTENERS.with(|v| v.borrow_mut().extend(listeners));
}
pub fn button(key: Key) -> gtk4::Button {
    let v = gtk4::Button::new();
    bind(&v, "label", key);
    v
}
pub fn label(key: Key) -> gtk4::Label {
    let v = gtk4::Label::new(None);
    bind(&v, "label", key);
    v
}

pub fn language() -> Language {
    match MODE.load(Ordering::Relaxed) {
        MODE_CHINESE => Language::Zh,
        MODE_ENGLISH => Language::En,
        _ => system_language(),
    }
}

pub fn system_language() -> Language {
    language_from_env(
        std::env::var("LANGUAGE").ok().as_deref(),
        std::env::var("LC_ALL").ok().as_deref(),
        std::env::var("LC_MESSAGES").ok().as_deref(),
        std::env::var("LANG").ok().as_deref(),
    )
}

pub fn language_from_env(
    language: Option<&str>,
    lc_all: Option<&str>,
    lc_messages: Option<&str>,
    lang: Option<&str>,
) -> Language {
    for value in [language, lc_all, lc_messages, lang].into_iter().flatten() {
        let first = value.split(':').next().unwrap_or("");
        if first.is_empty() || first == "C" || first == "POSIX" {
            continue;
        }
        return language_from_tag(first);
    }
    Language::Zh
}

pub fn language_from_tag(tag: &str) -> Language {
    let primary = tag.split(['_', '-', '.']).next().unwrap_or("");
    if primary.eq_ignore_ascii_case("en") {
        Language::En
    } else {
        Language::Zh
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Settings,
    Appearance,
    Language,
    System,
    Light,
    Dark,
    Chinese,
    English,
    Library,
    OriginalCount,
    OriginalBytes,
    CacheBytes,
    SettingsFailed,
    Backup,
    CreateBackup,
    RestoreBackup,
    BackupHint,
    BackupPreview,
    Merge,
    Replace,
    ReplaceHint,
    BackupSaved,
    BackupRestored,
    Working,
    Telegram,
    Token,
    TokenHint,
    RememberToken,
    ClearToken,
    TokenFailed,
    StickerPack,
    ViewStickers,
    ChooseStickers,
    SelectNew,
    ClearSelection,
    ImportSelected,
    ChangePack,
    AlreadyImported,
    InTrash,
    DownloadAgain,
    NoPreview,
    ImportFailed,
    ImportFinished,
    ImportStopped,
    NetworkFailed,
    TokenInvalid,
    RateLimited,
    PackMissing,
    LeaveImport,
    LeaveImportHint,
    FileTokenHint,
    RestoreHintNew,
    Rename,
    MoveUp,
    MoveDown,
    SelectItems,
    BatchOrganize,
    AssignCollection,
    ClearCollection,
    AddTags,
    RemoveTags,
    Back,
    Cancel,
    Done,
    Retry,
    Name,
    TabStickers,
    TabCollections,
    TagsTitle,
    Trash,
    TrashEmpty,
    TrashEmptyHint,
    LibraryAdd,
    LibrarySearch,
    ClearSearch,
    FavoritesOnly,
    FilterRecent,
    FilterAll,
    Organize,
    OrganizeHint,
    LibraryEmpty,
    LibraryEmptyHint,
    LibraryNoMatches,
    LibraryNoMatchesHint,
    LibraryLoadTitle,
    StarredEmpty,
    StarredEmptyHint,
    AnimatedImage,
    DetailLoadTitle,
    DetailFileTitle,
    DetailSize,
    DetailBytes,
    DetailFormat,
    DetailOriginalName,
    AddNote,
    PreviewUnavailable,
    PauseAnimation,
    PlayAnimation,
    ApngFirstFrame,
    Favorite,
    Unfavorite,
    CopyImage,
    CopyFile,
    SaveAs,
    SaveImage,
    Share,
    ShareOriginal,
    SharePreparing,
    ShareFirstFrame,
    QuickAnimatedOriginal,
    ExportChoose,
    ExportOriginal,
    ExportPng,
    ExportWhite,
    ExportSmall,
    ExportOriginalHint,
    ExportPngHint,
    ExportWhiteHint,
    ExportSmallHint,
    ImageCopied,
    Saved,
    StickerDeleted,
    View,
    Delete,
    DeleteHint,
    ImportPhotos,
    ImportReadingFiles,
    ImportPreparing,
    ImportReviewHint,
    ImportDiscard,
    ImportCancelRemaining,
    ImportDoneExisting,
    ImportRestoreRequired,
    ImportCancelled,
    ImportRetryFailed,
    ImportCollection,
    NoCollection,
    ErrorBatchBusy,
    CollectionEmpty,
    CollectionEmptyHint,
    NewCollection,
    NewTag,
    TagEmpty,
    TagEmptyHint,
    Restored,
    RestoreStickerTitle,
    Restore,
    RestoreHint,
    FailurePermission,
    FailureIo,
    FailureFull,
    FailureFormat,
    FailureImage,
    FailureLimit,
    FailureBatch,
    FailureBusy,
    FailureMissing,
    FailureLibrary,
    FailureUnknown,
    FailureChanged,
    FailureDeleted,
    FailureInput,
    FailureColorProfile,
    Paste,
    PasteHint,
    DropToImport,
    DirectorySkipped,
    ClipboardEmpty,
    UntitledImage,
    NoHome,
}

pub fn text(key: Key) -> &'static str {
    text_in(language(), key)
}

pub fn text_in(language: Language, key: Key) -> &'static str {
    let (zh, en) = pair(key);
    match language {
        Language::Zh => zh,
        Language::En => en,
    }
}

pub fn core(code: ErrorCode) -> &'static str {
    let key = match code {
        ErrorCode::InvalidInput => Key::FailureInput,
        ErrorCode::UnsupportedFormat => Key::FailureFormat,
        ErrorCode::UnsupportedColorProfile => Key::FailureColorProfile,
        ErrorCode::InvalidImage => Key::FailureImage,
        ErrorCode::ResourceLimit => Key::FailureLimit,
        ErrorCode::Busy => Key::FailureBusy,
        ErrorCode::Cancelled => Key::ImportCancelled,
        ErrorCode::NotFound => Key::FailureMissing,
        ErrorCode::EntityDeleted => Key::FailureDeleted,
        ErrorCode::Conflict => Key::FailureChanged,
        ErrorCode::PermissionDenied => Key::FailurePermission,
        ErrorCode::StorageFull => Key::FailureFull,
        ErrorCode::Io | ErrorCode::Network | ErrorCode::Timeout | ErrorCode::RateLimited => {
            Key::FailureIo
        }
        ErrorCode::Unauthorized => Key::FailurePermission,
        ErrorCode::UnsupportedSchema | ErrorCode::CorruptData | ErrorCode::Database => {
            Key::FailureLibrary
        }
        ErrorCode::AlreadyOpen | ErrorCode::Closed | ErrorCode::Internal => Key::FailureUnknown,
    };
    text(key)
}

pub fn share_first_frame(format: &str) -> String {
    match language() {
        Language::Zh => format!("仅发送首帧（{format}）"),
        Language::En => format!("Share first frame only ({format})"),
    }
}

pub fn import_review_title(count: usize) -> String {
    match language() {
        Language::Zh => format!("导入 {count} 张表情"),
        Language::En => format!("Import {count} Stickers"),
    }
}

pub fn import_running_progress(done: usize, total: usize) -> String {
    match language() {
        Language::Zh => format!("正在添加 {done} / {total}"),
        Language::En => format!("Importing {done} of {total}"),
    }
}

pub fn import_unreadable(count: usize) -> String {
    match language() {
        Language::Zh => format!("{count} 张图片无法读取，已跳过"),
        Language::En => format!("{count} unreadable items skipped"),
    }
}

pub fn import_add_count(count: usize) -> String {
    match language() {
        Language::Zh => format!("添加 {count} 张"),
        Language::En => format!("Import {count}"),
    }
}

pub fn import_done(count: usize) -> String {
    match language() {
        Language::Zh => format!("已添加 {count} 张表情"),
        Language::En => format!("{count} stickers added"),
    }
}

pub fn import_done_reused(added: usize, existing: usize) -> String {
    match language() {
        Language::Zh => format!("已添加 {added} 张，{existing} 张已存在"),
        Language::En => format!("{added} added, {existing} already exist"),
    }
}

pub fn import_done_partial(added: usize, failed: usize) -> String {
    match language() {
        Language::Zh => format!("已添加 {added} 张，{failed} 张添加失败"),
        Language::En => format!("{added} added, {failed} failed"),
    }
}

pub fn import_stopped(count: usize) -> String {
    match language() {
        Language::Zh => format!("导入已停止，已添加 {count} 张"),
        Language::En => format!("Import stopped. {count} stickers added."),
    }
}

pub fn import_problems_title(count: usize) -> String {
    match language() {
        Language::Zh => format!("{count} 张表情添加失败"),
        Language::En => format!("{count} stickers failed to import"),
    }
}

pub fn import_problems_hint(count: usize) -> String {
    match language() {
        Language::Zh => format!("其余 {count} 张已成功添加"),
        Language::En => format!("The remaining {count} stickers have been added"),
    }
}

pub fn detail_dimensions(width: u32, height: u32) -> String {
    format!("{width} × {height}")
}

pub fn file_size(bytes: u64) -> String {
    let (value, unit) = if bytes < 1024 {
        (bytes as f64, "B")
    } else if bytes < 1024 * 1024 {
        (bytes as f64 / 1024.0, "KB")
    } else if bytes < 1024 * 1024 * 1024 {
        (bytes as f64 / 1024.0 / 1024.0, "MB")
    } else {
        (bytes as f64 / 1024.0 / 1024.0 / 1024.0, "GB")
    };
    if unit == "B" {
        format!("{bytes} B")
    } else {
        let tenths = (value * 10.0).round();
        if tenths.rem_euclid(10.0) == 0.0 {
            format!("{} {unit}", (tenths / 10.0) as u64)
        } else {
            format!("{:.1} {unit}", tenths / 10.0)
        }
    }
}

fn pair(key: Key) -> (&'static str, &'static str) {
    match key {
        Key::Settings => ("设置", "Settings"),
        Key::Appearance => ("外观", "Appearance"),
        Key::Language => ("语言", "Language"),
        Key::System => ("跟随系统", "System"),
        Key::Light => ("浅色", "Light"),
        Key::Dark => ("深色", "Dark"),
        Key::Chinese => ("中文", "Chinese"),
        Key::English => ("英文", "English"),
        Key::Library => ("图片库", "Library"),
        Key::OriginalCount => ("原图数量", "Original count"),
        Key::OriginalBytes => ("原图占用", "Original storage"),
        Key::CacheBytes => ("缓存占用", "Cache storage"),
        Key::SettingsFailed => (
            "无法读取或保存设置，请重试。",
            "Could not read or save settings. Please retry.",
        ),
        Key::Backup => ("备份与恢复", "Backup and restore"),
        Key::CreateBackup => ("创建备份", "Create backup"),
        Key::RestoreBackup => ("恢复备份", "Restore backup"),
        Key::BackupHint => (
            "备份包含表情、合集、标签和原图。",
            "Back up stickers, collections, tags, and originals.",
        ),
        Key::BackupPreview => ("备份内容", "Backup contents"),
        Key::Merge => ("合并到当前库", "Merge into library"),
        Key::Replace => ("替换当前库", "Replace library"),
        Key::ReplaceHint => (
            "当前图片库将被备份内容替换，继续吗？",
            "Replace the current library with this backup?",
        ),
        Key::BackupSaved => ("备份已保存", "Backup saved"),
        Key::BackupRestored => ("恢复完成", "Restore complete"),
        Key::Working => ("正在处理…", "Working…"),
        Key::Telegram => ("Telegram 导入", "Import from Telegram"),
        Key::Token => ("Bot Token", "Bot Token"),
        Key::TokenHint => (
            "通过 Telegram 的 @BotFather 创建机器人，即可获取 Token。",
            "Create a bot with @BotFather on Telegram to get a token.",
        ),
        Key::RememberToken => ("记住 Token", "Remember token"),
        Key::ClearToken => ("清除 Token", "Clear token"),
        Key::TokenFailed => (
            "无法读取或保存 Token，请重试或清除。",
            "Could not read or save the token. Retry or clear it.",
        ),
        Key::StickerPack => ("贴纸包名称或链接", "Sticker pack name or link"),
        Key::ViewStickers => ("查看贴纸", "View stickers"),
        Key::ChooseStickers => ("选一些喜欢的贴纸带回来", "Pick some stickers to bring back"),
        Key::SelectNew => ("选择未导入的", "Select new stickers"),
        Key::ClearSelection => ("清空选择", "Clear selection"),
        Key::ImportSelected => ("导入所选", "Import selected"),
        Key::ChangePack => ("换一个贴纸包", "Change pack"),
        Key::AlreadyImported => ("已导入", "Imported"),
        Key::InTrash => ("在回收站", "In trash"),
        Key::DownloadAgain => ("可重新下载", "Download again"),
        Key::NoPreview => ("暂无预览", "No preview"),
        Key::ImportFailed => ("导入失败", "Import failed"),
        Key::ImportFinished => ("导入完成", "Import complete"),
        Key::ImportStopped => ("已停止导入", "Import stopped"),
        Key::NetworkFailed => (
            "暂时无法连接 Telegram，请检查网络后重试。",
            "Could not connect to Telegram. Check your connection and retry.",
        ),
        Key::TokenInvalid => (
            "Token 无效，请检查后重试。",
            "Invalid token. Check it and retry.",
        ),
        Key::RateLimited => (
            "请求太频繁，请稍后再试。",
            "Too many requests. Please try again later.",
        ),
        Key::PackMissing => (
            "找不到这个贴纸包，请检查名称或链接。",
            "Sticker pack not found. Check the name or link.",
        ),
        Key::LeaveImport => ("停止并退出？", "Stop and leave?"),
        Key::LeaveImportHint => (
            "未完成的导入会停止，已经导入的表情会保留。",
            "Unfinished imports will stop. Stickers already imported will stay.",
        ),
        Key::FileTokenHint => (
            "系统密钥环不可用，Token 将保存在仅当前用户可读的本机文件中。",
            "The system keyring is unavailable. The token will be stored in a local file readable only by you.",
        ),
        Key::RestoreHintNew => (
            "恢复前请先停止导入；正在分享或复制的文件可能需要先释放。",
            "Stop imports before restoring. Files being shared or copied may need to be released first.",
        ),
        Key::Back => ("返回", "Back"),
        Key::Cancel => ("取消", "Cancel"),
        Key::Done => ("完成", "Done"),
        Key::Retry => ("重试", "Retry"),
        Key::Name => ("名称", "Name"),
        Key::TabStickers => ("表情", "Stickers"),
        Key::TabCollections => ("合集", "Collections"),
        Key::TagsTitle => ("标签", "Tags"),
        Key::Trash => ("回收站", "Trash"),
        Key::TrashEmpty => ("回收站为空", "Trash is empty"),
        Key::TrashEmptyHint => (
            "已删除的表情、合集和标签会暂存在这里，可随时恢复",
            "Deleted stickers, collections, and tags are kept here and can be restored anytime",
        ),
        Key::LibraryAdd => ("添加", "Add"),
        Key::LibrarySearch => ("搜索表情", "Search stickers"),
        Key::ClearSearch => ("清空搜索", "Clear search"),
        Key::FavoritesOnly => ("星标", "Starred"),
        Key::FilterRecent => ("最近使用", "Recent"),
        Key::FilterAll => ("全部", "All"),
        Key::Rename => ("重命名", "Rename"),
        Key::MoveUp => ("向前移动", "Move up"),
        Key::MoveDown => ("向后移动", "Move down"),
        Key::SelectItems => ("选择", "Select"),
        Key::BatchOrganize => ("批量整理", "Organize selection"),
        Key::AssignCollection => ("归入合集", "Move to collection"),
        Key::ClearCollection => ("取消归类", "Uncategorize"),
        Key::AddTags => ("添加标签", "Add tags"),
        Key::RemoveTags => ("移除标签", "Remove tags"),
        Key::Organize => ("整理", "Organize"),
        Key::OrganizeHint => ("添加合集与标签", "Add collection & tag"),
        Key::LibraryEmpty => ("暂无表情", "No stickers yet"),
        Key::LibraryEmptyHint => (
            "支持从相册或本地文件导入表情",
            "Import stickers from photos or files",
        ),
        Key::LibraryNoMatches => ("未找到相关表情", "No results found"),
        Key::LibraryNoMatchesHint => ("请尝试更换搜索词", "Try searching for different keywords"),
        Key::LibraryLoadTitle => ("表情加载失败", "Failed to load stickers"),
        Key::StarredEmpty => ("暂无星标表情", "No starred stickers"),
        Key::StarredEmptyHint => (
            "长按表情添加星标，常用表情会显示在此处",
            "Touch and hold a sticker to star it. Starred stickers will appear here.",
        ),
        Key::AnimatedImage => ("动图", "GIF"),
        Key::DetailLoadTitle => ("无法加载表情", "Failed to load sticker"),
        Key::DetailFileTitle => ("详细信息", "Details"),
        Key::DetailSize => ("尺寸", "Dimensions"),
        Key::DetailBytes => ("大小", "Size"),
        Key::DetailFormat => ("格式", "Format"),
        Key::DetailOriginalName => ("原文件名", "Original file"),
        Key::AddNote => ("添加备注", "Add note"),
        Key::PreviewUnavailable => ("预览加载失败", "Preview unavailable"),
        Key::PauseAnimation => ("暂停", "Pause"),
        Key::PlayAnimation => ("播放", "Play"),
        Key::ApngFirstFrame => (
            "当前仅显示首帧预览，发送时仍为动图",
            "Only the first frame is previewed. Animation will be preserved when shared.",
        ),
        Key::Favorite => ("星标", "Star"),
        Key::Unfavorite => ("取消星标", "Unstar"),
        Key::CopyImage => ("复制图片", "Copy Image"),
        Key::CopyFile => ("复制原文件", "Copy Original File"),
        Key::SaveAs => ("另存为", "Save As"),
        Key::SaveImage => ("保存", "Save"),
        Key::Share => ("分享", "Share"),
        Key::ShareOriginal => ("分享原图", "Share Original"),
        Key::SharePreparing => ("正在准备…", "Preparing…"),
        Key::ShareFirstFrame => ("分享首帧", "Share First Frame"),
        Key::QuickAnimatedOriginal => (
            "动图按原图发送，保留动效",
            "Animated stickers are sent as originals to preserve animation",
        ),
        Key::ExportChoose => ("发送格式", "Export Format"),
        Key::ExportOriginal => ("原图", "Original"),
        Key::ExportPng => ("透明 PNG", "Transparent PNG"),
        Key::ExportWhite => ("白底 PNG", "White PNG"),
        Key::ExportSmall => ("小图", "Small JPG"),
        Key::ExportOriginalHint => (
            "保持原始文件，保留透明通道与动效",
            "Original file with transparent background and animation preserved",
        ),
        Key::ExportPngHint => (
            "保留透明通道，最长边限制 1024px",
            "Preserves transparency, max dimension 1024px",
        ),
        Key::ExportWhiteHint => (
            "填充白色背景，保持原始尺寸",
            "White background, original dimensions preserved",
        ),
        Key::ExportSmallHint => (
            "缩放至最长边 512px，填充白色背景",
            "Max dimension 512px, white background",
        ),
        Key::ImageCopied => ("已复制到剪贴板", "Copied to clipboard"),
        Key::Saved => ("已保存", "Saved"),
        Key::StickerDeleted => ("已移至回收站", "Moved to Trash"),
        Key::View => ("查看", "View"),
        Key::Delete => ("删除", "Delete"),
        Key::DeleteHint => (
            "删除后可在回收站找回",
            "Deleted items can be restored from Trash",
        ),
        Key::ImportPhotos => ("添加表情", "Import Stickers"),
        Key::ImportReadingFiles => ("正在读取…", "Reading files…"),
        Key::ImportPreparing => ("正在处理…", "Processing…"),
        Key::ImportReviewHint => (
            "来自外部应用分享，确认后加入表情库",
            "Shared from external app. Confirm to add to your sticker library.",
        ),
        Key::ImportDiscard => ("取消导入", "Discard"),
        Key::ImportCancelRemaining => ("停止", "Stop"),
        Key::ImportDoneExisting => ("所选表情均已存在", "All selected stickers already exist"),
        Key::ImportRestoreRequired => ("该表情已在回收站中", "Item is currently in Trash"),
        Key::ImportCancelled => ("已取消", "Cancelled"),
        Key::ImportRetryFailed => ("重试失败项", "Retry failed items"),
        Key::ImportCollection => ("加入合集", "Add to Collection"),
        Key::NoCollection => ("不加入合集", "No collection"),
        Key::ErrorBatchBusy => (
            "当前有正在处理的导入任务，请完成后再试",
            "An import task is already running. Please complete or cancel it first.",
        ),
        Key::CollectionEmpty => ("合集为空", "Collection is empty"),
        Key::CollectionEmptyHint => (
            "长按表情选择「整理」可添加到合集",
            "Touch and hold a sticker and select Organize to add it to a collection.",
        ),
        Key::NewCollection => ("新建合集", "New Collection"),
        Key::NewTag => ("新建标签", "New Tag"),
        Key::TagEmpty => ("暂无相关表情", "No stickers with this tag"),
        Key::TagEmptyHint => (
            "长按表情选择「整理」可添加标签",
            "Touch and hold a sticker and select Organize to add tags",
        ),
        Key::Restored => ("已恢复", "Restored"),
        Key::RestoreStickerTitle => ("恢复表情", "Restore Sticker"),
        Key::Restore => ("恢复", "Restore"),
        Key::RestoreHint => (
            "恢复后需要重新添加到合集或标签",
            "Collections and tags will need to be reassigned after restoring",
        ),
        Key::FailurePermission => (
            "缺少存储访问权限，请重试",
            "Storage permission denied. Please try again.",
        ),
        Key::FailureIo => ("文件读写失败，请重试", "File I/O error. Please try again."),
        Key::FailureFull => ("设备存储空间不足", "Device storage is full"),
        Key::FailureFormat => ("不支持的图片格式", "Unsupported image format"),
        Key::FailureImage => ("图片文件损坏", "Corrupted image file"),
        Key::FailureLimit => ("图片大小超出限制", "Image exceeds size limit"),
        Key::FailureBatch => ("单次导入最多支持 200 张", "Maximum 200 items per import"),
        Key::FailureBusy => (
            "系统繁忙，请稍后重试",
            "System busy. Please try again later.",
        ),
        Key::FailureMissing => ("图片文件不存在或已丢失", "Image file not found"),
        Key::FailureLibrary => (
            "表情库读取失败，请稍后重试",
            "Failed to read library. Please try again later.",
        ),
        Key::FailureUnknown => ("操作失败，请重试", "Operation failed. Please try again."),
        Key::FailureChanged => (
            "数据已变更，请刷新后重试",
            "Data changed. Please refresh and try again.",
        ),
        Key::FailureDeleted => ("该项目已在回收站中", "Item is already in Trash"),
        Key::FailureInput => (
            "输入内容无效，请检查后重试",
            "Invalid input. Please check and try again.",
        ),
        Key::FailureColorProfile => (
            "图片颜色配置暂不支持转换，请直接发送原图",
            "Color profile conversion unsupported. Please share as original.",
        ),
        Key::Paste => ("粘贴", "Paste"),
        Key::PasteHint => ("从剪贴板导入", "Import from the clipboard"),
        Key::DropToImport => ("松开以导入", "Release to import"),
        Key::DirectorySkipped => ("目录不会导入", "Folders are not imported"),
        Key::ClipboardEmpty => ("剪贴板里没有图片", "No image on the clipboard"),
        Key::UntitledImage => ("未命名图片", "Untitled image"),
        Key::NoHome => (
            "没有可用的主目录，无法确定库的位置",
            "No home directory is available, so the library location cannot be determined",
        ),
    }
}

pub fn selection_count(count: usize) -> String {
    match language() {
        Language::Zh => format!("已选 {count} 张"),
        Language::En => format!("{count} selected"),
    }
}
pub fn collection_count(count: u64) -> String {
    match language() {
        Language::Zh => format!("{count} 张表情"),
        Language::En => format!("{count} stickers"),
    }
}
pub fn batch_result(done: usize, failed: usize, remaining: usize) -> String {
    match language() {
        Language::Zh => format!("已完成 {done} 张，失败 {failed} 张，未处理 {remaining} 张"),
        Language::En => format!("{done} completed, {failed} failed, {remaining} remaining"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, Language, file_size, language_from_env, language_from_tag, text_in};

    #[test]
    fn english_tags_select_english_and_everything_else_stays_chinese() {
        assert_eq!(language_from_tag("en"), Language::En);
        assert_eq!(language_from_tag("en_US.UTF-8"), Language::En);
        assert_eq!(language_from_tag("zh_CN.UTF-8"), Language::Zh);
        assert_eq!(language_from_tag("fr_FR"), Language::Zh);
        assert_eq!(
            language_from_env(Some("en_US:zh_CN"), None, None, Some("zh_CN.UTF-8")),
            Language::En
        );
        assert_eq!(
            language_from_env(Some("C"), Some("POSIX"), None, Some("zh_CN.UTF-8")),
            Language::Zh
        );
        assert_eq!(language_from_env(None, None, None, None), Language::Zh);
    }

    #[test]
    fn catalog_matches_the_android_strings() {
        assert_eq!(text_in(Language::Zh, Key::LibraryEmpty), "暂无表情");
        assert_eq!(text_in(Language::En, Key::LibraryEmpty), "No stickers yet");
        assert_eq!(text_in(Language::Zh, Key::NoCollection), "不加入合集");
        assert_eq!(
            text_in(Language::Zh, Key::RestoreHint),
            "恢复后需要重新添加到合集或标签"
        );
        assert_eq!(file_size(202), "202 B");
        assert_eq!(file_size(1024), "1 KB");
        assert_eq!(file_size(1536), "1.5 KB");
    }
}
