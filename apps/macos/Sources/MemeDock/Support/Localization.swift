import Foundation

enum Language {
    case english
    case chinese
}

/// Interface strings live in Swift tables instead of localized bundles: the
/// language follows an in-app choice, and switching must not reload the app.
enum L10nKey: String {
    case app, library, stickers, collections, tags, all, starred, recentlyUsed, deleted
    case search, sort, sortRecent, sortCollectionOrder, sortLastUsed, sortHint
    case importAction, importReview, importProgress, importCancel, importSummary, importExisting
    case settings, general, storage, appearance, language, system, light, dark, chinese, english
    case cancel, confirm, delete, restore, rename, done, retry, ok, close, empty, applying
    case copyImage, copyFile, share, saveAs, revealInFinder, copy
    case title, note, originalName, size, dimensions, format, animated, staticImage
    case createdAt, lastUsed, useCount, emptyLibrary, noResults, loading, loadMore, selection
    case trash, backup, telegram, originalCount, originalBytes, cacheBytes, shareBytes
    case createCheckpoint, cleanExports, libraryId, deviceId, dataDirectory, cacheDirectory, exportDirectory
    case checkpointCreated, exportsCleaned, imageCopied, fileCopied, saved, shareLaunched
    case importDone, importDoneReused, importDonePartial, importStopped, importFailed, importPreparing
    case importDropHere, importDropHint, directorySkipped, restoreRequired, thumbnailFailed, busy
    case exportOriginal, exportCompatiblePng, exportWhiteBackground, exportSmallJpeg
    case exportOriginalHint, exportCompatiblePngHint, exportWhiteBackgroundHint, exportSmallJpegHint
    case exportPreset, export
    case errorNetwork, errorTimeout, errorUnauthorized, errorRateLimited, errorInvalidInput
    case errorUnsupportedFormat, errorUnsupportedColorProfile, errorInvalidImage, errorResourceLimit
    case errorAlreadyOpen, errorClosed, errorBusy, errorCancelled, errorNotFound, errorEntityDeleted
    case errorConflict, errorUnsupportedSchema, errorCorruptData, errorPermissionDenied
    case errorStorageFull, errorIo, errorDatabase, errorInternal, errorUnknown
}

enum L10n {
    private static let english: [L10nKey: String] = [
        .app: "MemeDock", .library: "Library", .stickers: "Stickers", .collections: "Collections",
        .tags: "Tags", .all: "All stickers", .starred: "Starred", .recentlyUsed: "Recently used",
        .deleted: "Deleted",
        .search: "Search", .sort: "Sort", .sortRecent: "Recently added",
        .sortCollectionOrder: "Collection order", .sortLastUsed: "Recently used",
        .sortHint: "Sort order",
        .importAction: "Import", .importReview: "Import stickers", .importProgress: "Importing",
        .importCancel: "Cancel import", .importSummary: "Import result", .importExisting: "Sticker already in the library",
        .settings: "Settings", .general: "General", .storage: "Storage", .appearance: "Appearance",
        .language: "Language", .system: "System", .light: "Light", .dark: "Dark",
        .chinese: "Chinese", .english: "English",
        .cancel: "Cancel", .confirm: "Confirm", .delete: "Delete", .restore: "Restore",
        .rename: "Rename", .done: "Done", .retry: "Retry", .ok: "OK", .close: "Close",
        .empty: "Nothing here", .applying: "Working",
        .copyImage: "Copy image", .copyFile: "Copy file", .share: "Share",
        .saveAs: "Save as", .revealInFinder: "Reveal in Finder", .copy: "Copy",
        .title: "Title", .note: "Note", .originalName: "File name", .size: "Size",
        .dimensions: "Dimensions", .format: "Format", .animated: "Animated", .staticImage: "Static",
        .createdAt: "Added", .lastUsed: "Last used", .useCount: "Times used",
        .emptyLibrary: "Import your first sticker to get started",
        .noResults: "No stickers match the current search", .loading: "Loading",
        .loadMore: "Load more", .selection: "Selection",
        .trash: "Trash", .backup: "Backup", .telegram: "Telegram",
        .originalCount: "Originals", .originalBytes: "Original size", .cacheBytes: "Thumbnails and cache",
        .shareBytes: "Prepared shares",
        .createCheckpoint: "Create checkpoint", .cleanExports: "Clean prepared shares",
        .libraryId: "Library", .deviceId: "Device",
        .dataDirectory: "Library folder", .cacheDirectory: "Cache folder", .exportDirectory: "Share folder",
        .checkpointCreated: "Checkpoint created", .exportsCleaned: "Prepared shares cleaned",
        .imageCopied: "Image copied", .fileCopied: "File copied", .saved: "Saved",
        .shareLaunched: "Share ready",
        .importDone: "Imported %d sticker(s)", .importDoneReused: "Imported %d, reused %d",
        .importDonePartial: "Imported %d, %d failed", .importStopped: "Stopped after %d sticker(s)",
        .importFailed: "Import failed", .importPreparing: "Preparing",
        .importDropHere: "Drop images to import", .importDropHint: "Images and animations are copied into the library",
        .directorySkipped: "Folders are skipped", .restoreRequired: "Original file missing",
        .thumbnailFailed: "Preview unavailable", .busy: "Another task is running",
        .exportOriginal: "Original", .exportCompatiblePng: "Compatible PNG",
        .exportWhiteBackground: "White background", .exportSmallJpeg: "Small JPEG",
        .exportOriginalHint: "Upload the stored file unchanged",
        .exportCompatiblePngHint: "Flatten to PNG at up to 1024 px",
        .exportWhiteBackgroundHint: "Opaque PNG for apps that ignore transparency",
        .exportSmallJpegHint: "512 px JPEG for size-limited targets",
        .exportPreset: "Format", .export: "Export",
        .errorNetwork: "The network is unavailable", .errorTimeout: "The request timed out",
        .errorUnauthorized: "Credentials are missing or rejected", .errorRateLimited: "Too many requests",
        .errorInvalidInput: "The input is not valid", .errorUnsupportedFormat: "This format is not supported",
        .errorUnsupportedColorProfile: "This colour profile is not supported",
        .errorInvalidImage: "The image could not be decoded", .errorResourceLimit: "A resource limit was reached",
        .errorAlreadyOpen: "The library is already open", .errorClosed: "The library is closed",
        .errorBusy: "Another task is running", .errorCancelled: "The task was cancelled",
        .errorNotFound: "Not found", .errorEntityDeleted: "This item is deleted",
        .errorConflict: "The item changed elsewhere", .errorUnsupportedSchema: "The library needs a newer app",
        .errorCorruptData: "Stored data is damaged", .errorPermissionDenied: "Permission denied",
        .errorStorageFull: "Storage is full", .errorIo: "The file could not be read or written",
        .errorDatabase: "The library database failed", .errorInternal: "Something went wrong",
        .errorUnknown: "Something went wrong",
    ]

    private static let chinese: [L10nKey: String] = [
        .app: "MemeDock", .library: "素材库", .stickers: "表情", .collections: "合集",
        .tags: "标签", .all: "全部表情", .starred: "已收藏", .recentlyUsed: "最近使用",
        .deleted: "回收站",
        .search: "搜索", .sort: "排序", .sortRecent: "最近添加",
        .sortCollectionOrder: "合集顺序", .sortLastUsed: "最近使用",
        .sortHint: "排序方式",
        .importAction: "导入", .importReview: "导入表情", .importProgress: "正在导入",
        .importCancel: "取消导入", .importSummary: "导入结果", .importExisting: "素材库中已有该表情",
        .settings: "设置", .general: "通用", .storage: "存储", .appearance: "外观",
        .language: "语言", .system: "跟随系统", .light: "浅色", .dark: "深色",
        .chinese: "简体中文", .english: "English",
        .cancel: "取消", .confirm: "确定", .delete: "删除", .restore: "恢复",
        .rename: "重命名", .done: "完成", .retry: "重试", .ok: "好", .close: "关闭",
        .empty: "暂无内容", .applying: "处理中",
        .copyImage: "复制图片", .copyFile: "复制文件", .share: "分享",
        .saveAs: "另存为", .revealInFinder: "在访达中显示", .copy: "复制",
        .title: "标题", .note: "备注", .originalName: "文件名", .size: "大小",
        .dimensions: "尺寸", .format: "格式", .animated: "动图", .staticImage: "静态",
        .createdAt: "添加时间", .lastUsed: "最近使用", .useCount: "使用次数",
        .emptyLibrary: "先导入一张表情开始使用",
        .noResults: "没有符合当前搜索的表情", .loading: "加载中",
        .loadMore: "加载更多", .selection: "所选",
        .trash: "回收站", .backup: "备份", .telegram: "Telegram",
        .originalCount: "原图数量", .originalBytes: "原图占用", .cacheBytes: "缩略图与缓存",
        .shareBytes: "待分享副本",
        .createCheckpoint: "创建检查点", .cleanExports: "清理待分享副本",
        .libraryId: "素材库", .deviceId: "设备",
        .dataDirectory: "素材库目录", .cacheDirectory: "缓存目录", .exportDirectory: "分享目录",
        .checkpointCreated: "已创建检查点", .exportsCleaned: "已清理待分享副本",
        .imageCopied: "已复制图片", .fileCopied: "已复制文件", .saved: "已保存",
        .shareLaunched: "已准备分享",
        .importDone: "已导入 %d 张表情", .importDoneReused: "已导入 %d 张，复用 %d 张",
        .importDonePartial: "已导入 %d 张，%d 张失败", .importStopped: "已停止，完成 %d 张",
        .importFailed: "导入失败", .importPreparing: "准备中",
        .importDropHere: "拖入图片即可导入", .importDropHint: "图片与动图会被复制进素材库",
        .directorySkipped: "已跳过文件夹", .restoreRequired: "原图文件缺失",
        .thumbnailFailed: "预览不可用", .busy: "已有任务在进行",
        .exportOriginal: "原图", .exportCompatiblePng: "兼容 PNG",
        .exportWhiteBackground: "白色背景", .exportSmallJpeg: "小尺寸 JPEG",
        .exportOriginalHint: "直接使用已保存的原图",
        .exportCompatiblePngHint: "转换为最长边 1024 的 PNG",
        .exportWhiteBackgroundHint: "不透明 PNG，适合不识别透明的应用",
        .exportSmallJpegHint: "最长边 512 的 JPEG，适合有体积限制的场景",
        .exportPreset: "格式", .export: "导出",
        .errorNetwork: "网络不可用", .errorTimeout: "请求超时",
        .errorUnauthorized: "凭据缺失或无效", .errorRateLimited: "请求过于频繁",
        .errorInvalidInput: "输入不合法", .errorUnsupportedFormat: "不支持该格式",
        .errorUnsupportedColorProfile: "不支持该色彩配置",
        .errorInvalidImage: "无法解码该图片", .errorResourceLimit: "已触达资源上限",
        .errorAlreadyOpen: "素材库已打开", .errorClosed: "素材库已关闭",
        .errorBusy: "已有任务在进行", .errorCancelled: "任务已取消",
        .errorNotFound: "未找到内容", .errorEntityDeleted: "该项已删除",
        .errorConflict: "内容已在别处变更", .errorUnsupportedSchema: "素材库需要更新版本的客户端",
        .errorCorruptData: "存储数据已损坏", .errorPermissionDenied: "没有权限",
        .errorStorageFull: "存储空间已满", .errorIo: "文件读写失败",
        .errorDatabase: "素材库数据库出错", .errorInternal: "出现异常",
        .errorUnknown: "出现异常",
    ]

    /// Set by `Preferences` so views read the active language at render time.
    static var language: Language = .english

    static func text(_ key: L10nKey) -> String {
        let table = language == .chinese ? chinese : english
        return table[key] ?? key.rawValue
    }

    static func text(_ key: L10nKey, _ arguments: CVarArg...) -> String {
        String(format: text(key), arguments: arguments)
    }
}
