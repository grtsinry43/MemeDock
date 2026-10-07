use memedock_core::ErrorCode;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Zh,
    En,
}

/// Same three modes as the Android language setting. System follows the process locale.
/// The settings page is the caller; it is not built yet.
#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn set_language_mode(mode: LanguageMode) {
    let value = match mode {
        LanguageMode::System => MODE_SYSTEM,
        LanguageMode::Chinese => MODE_CHINESE,
        LanguageMode::English => MODE_ENGLISH,
    };
    MODE.store(value, Ordering::Relaxed);
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
        ErrorCode::Io => Key::FailureIo,
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
        Key::Back => ("返回", "Back"),
        Key::Cancel => ("取消", "Cancel"),
        Key::Done => ("完成", "Done"),
        Key::Retry => ("重试", "Retry"),
        Key::Name => ("名称", "Name"),
        Key::TabStickers => ("表情", "Stickers"),
        Key::TabCollections => ("收藏夹", "Collections"),
        Key::TagsTitle => ("标签", "Tags"),
        Key::Trash => ("回收站", "Trash"),
        Key::TrashEmpty => ("回收站为空", "Trash is empty"),
        Key::TrashEmptyHint => (
            "已删除的表情、收藏夹和标签会暂存在这里，可随时恢复",
            "Deleted stickers, collections, and tags are kept here and can be restored anytime",
        ),
        Key::LibraryAdd => ("添加", "Add"),
        Key::LibrarySearch => ("搜索表情", "Search stickers"),
        Key::ClearSearch => ("清空搜索", "Clear search"),
        Key::FavoritesOnly => ("星标", "Starred"),
        Key::FilterRecent => ("最近使用", "Recent"),
        Key::FilterAll => ("全部", "All"),
        Key::Organize => ("整理", "Organize"),
        Key::OrganizeHint => ("添加收藏夹与标签", "Add collection & tag"),
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
        Key::ImportCollection => ("加入收藏夹", "Add to Collection"),
        Key::NoCollection => ("不加入收藏夹", "No collection"),
        Key::ErrorBatchBusy => (
            "当前有正在处理的导入任务，请完成后再试",
            "An import task is already running. Please complete or cancel it first.",
        ),
        Key::CollectionEmpty => ("收藏夹为空", "Collection is empty"),
        Key::CollectionEmptyHint => (
            "长按表情选择「整理」可添加到收藏夹",
            "Touch and hold a sticker and select Organize to add it to a collection.",
        ),
        Key::NewCollection => ("新建收藏夹", "New Collection"),
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
            "恢复后需要重新添加到收藏夹或标签",
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
        assert_eq!(text_in(Language::Zh, Key::NoCollection), "不加入收藏夹");
        assert_eq!(
            text_in(Language::Zh, Key::RestoreHint),
            "恢复后需要重新添加到收藏夹或标签"
        );
        assert_eq!(file_size(202), "202 B");
        assert_eq!(file_size(1024), "1 KB");
        assert_eq!(file_size(1536), "1.5 KB");
    }
}
