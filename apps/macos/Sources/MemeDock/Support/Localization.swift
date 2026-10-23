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
    case cancel, confirm, delete, restore, rename, done, retry, ok, close, empty, applying, more
    case purge, emptyTrash, purgeConfirmation, purgedCount, retryPurgeCleanup
    case purgePartial, purgeCleanupDone
    case hideSidebar, showSidebar
    case expanded, collapsed, removeTag, animatedUsesOriginal
    case copyImage, copyFile, share, saveAs, revealInFinder, copy
    case title, note, originalName, size, dimensions, format, animated, staticImage
    case createdAt, lastUsed, useCount, emptyLibrary, noResults, loading, loadMore, selection
    case trash, backup, telegram, originalCount, originalBytes, cacheBytes
    case createCheckpoint, cleanExports, libraryId, deviceId, dataDirectory, cacheDirectory, exportDirectory
    case checkpointCreated, exportsCleaned, imageCopied, fileCopied, saved, shareLaunched
    case importDone, importDoneReused, importDonePartial, importStopped, importFailed, importPreparing
    case importDropHere, importDropHint, directorySkipped, restoreRequired, thumbnailFailed, busy
    case exportOriginal, exportCompatiblePng, exportWhiteBackground, exportSmallJpeg, exportCompatibleGif
    case exportOriginalHint, exportCompatiblePngHint, exportWhiteBackgroundHint, exportSmallJpegHint, exportCompatibleGifHint
    case exportPreset, export
    case errorNetwork, errorTimeout, errorUnauthorized, errorRateLimited, errorInvalidInput
    case errorUnsupportedFormat, errorUnsupportedColorProfile, errorInvalidImage, errorResourceLimit
    case errorAlreadyOpen, errorClosed, errorBusy, errorCancelled, errorNotFound, errorEntityDeleted
    case errorConflict, errorUnsupportedSchema, errorCorruptData, errorPermissionDenied
    case errorStorageFull, errorIo, errorDatabase, errorInternal, errorUnknown
    case libraryAdd, newCollection, newTag, moveUp, moveDown, clearSearch, failedToLoad
    case libraryEmpty, libraryEmptyHint, starredEmpty, starredEmptyHint, tagEmpty, tagEmptyHint
    case trashEmpty, trashEmptyHint, collectionEmpty, collectionEmptyHint, noMatchesHint
    case selectItems, selectAll, selectedCount, batchOrganize, assignCollection, clearCollection
    case selectionHint, selectionLoadedOnly, selectionOverLimit
    case addTags, removeTags, star, unstar, batchResult, batchRunning
    case details, fileInfo, organization, noCollection, addNote, firstFrame, restoreOrganization
    case pauseAnimation, playAnimation, animatedFirstFrame
    case deleteStickerTitle, deletedSticker, restoreStickerTitle, restoreHint, restored
    case telegramToken, telegramTokenHint, rememberToken, clearToken, tokenFailed, tokenInvalid
    case stickerPack, viewStickers, chooseStickers, selectNew, clearSelection, importSelected
    case changePack, alreadyImported, inTrash, packMissing
    case importReadingFiles, importRetryFailed, importPendingRestore
    case createBackup, restoreBackup, backupHint, backupPreview, merge, replace, replaceHint
    case backupSaved, backupRestored, archiveContains
    case backupMergeHint, backupMergeSummary
    case outputTab, backupTab, about
    case outputDefaults, outputCopy, outputCopyHint, outputShare, outputShareHint, outputFooter
    case outputDrag, outputDragHint
    case outputStatic, outputAnimated, exportBestCompatibility, exportBestCompatibilityHint, exportAnimatedCompatibilityHint
    case dragPreparing, dragReady
    case followDefaults, followDefaultsHint
    case tokenSaved, tokenNotSaved, forget, importFromTelegram, importFromTelegramHint, telegramSettingsHint
    case open, backUp, backupSavedTo, restoreHintShort, chooseFile, restoreMode, discard
    case checkpointHint, create, space, cleanUp, clean, cleanExportsHint, locations
    case version, licenses, mitLicense, quicksandFont, openFontLicense, view, idCopied
    case importFromFiles, importFromTelegramPack, useSavedToken, change, packPlaceholder, stickerCount
    case largerStickers, smallerStickers, defaultStickerSize
    case nothingToPaste, quickLookUnavailable
    case noHome
    case syncTab, syncServer, syncToken, syncTokenHint, syncConnect, syncNow, syncPause, syncResume
    case syncDisconnect, syncDisconnectTitle, syncDisconnectHint, syncPending, syncBlocked, syncBlockedHint
    case syncLastSuccess, syncNever, syncReady, syncRunning, syncPaused, syncStatus, syncMergeHint
    case syncKeychainFailed, syncReauthorize
}

enum L10n {
    private static let english: [L10nKey: String] = [
        .purge: "Delete permanently", .emptyTrash: "Empty Trash…",
        .purgeConfirmation: "Permanently delete %d items? This cannot be undone. Deletion will also sync to your other devices. Deleting collections or tags keeps their images.",
        .purgedCount: "Permanently deleted %d items", .retryPurgeCleanup: "Retry file cleanup",
        .purgePartial: "%d completed, %d unfinished: %@", .purgeCleanupDone: "File cleanup completed",
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
        .empty: "Nothing here", .applying: "Working", .more: "More",
        .hideSidebar: "Hide sidebar", .showSidebar: "Show sidebar",
        .expanded: "Expanded", .collapsed: "Collapsed", .removeTag: "Remove tag %@",
        .animatedUsesOriginal: "Animations keep their original format. Use Copy first frame for a still image.",
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
        .createCheckpoint: "Create checkpoint", .cleanExports: "Clean prepared shares",
        .libraryId: "Library ID", .deviceId: "Device ID",
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
        .exportCompatibleGif: "Compatible GIF",
        .exportCompatibleGifHint: "Animated GIF up to 512 px; colors and transparent edges may change",
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
        .libraryAdd: "Add", .newCollection: "New collection", .newTag: "New tag",
        .moveUp: "Move up", .moveDown: "Move down", .clearSearch: "Clear search",
        .failedToLoad: "Could not load this section",
        .libraryEmpty: "No stickers yet", .libraryEmptyHint: "Import images to start your library",
        .starredEmpty: "No starred stickers", .starredEmptyHint: "Star a sticker to keep it here",
        .tagEmpty: "No stickers use this tag", .tagEmptyHint: "Add the tag to a sticker to see it here",
        .trashEmpty: "The trash is empty",
        .trashEmptyHint: "Deleted stickers stay here until you restore them",
        .collectionEmpty: "This collection is empty",
        .collectionEmptyHint: "Select stickers and add them to this collection",
        .noMatchesHint: "Try a different search term",
        .selectItems: "Select", .selectAll: "Select all", .selectedCount: "%d selected",
        .selectionHint: "Click stickers to add them to the selection",
        .selectionLoadedOnly: "Select all covers loaded stickers only",
        .selectionOverLimit: "%d selected, above the %d per batch limit",
        .batchOrganize: "Organise selected", .assignCollection: "Add to collection",
        .clearCollection: "Remove from collection", .addTags: "Add tags", .removeTags: "Remove tags",
        .star: "Star", .unstar: "Remove star",
        .batchResult: "Updated %d, %d failed, %d skipped", .batchRunning: "Working on %d stickers",
        .details: "Details", .fileInfo: "File", .organization: "Organisation",
        .noCollection: "No collection", .addNote: "Add a note", .firstFrame: "Copy first frame",
        .pauseAnimation: "Pause", .playAnimation: "Play",
        .animatedFirstFrame: "Only the first frame is previewed. Animation will be preserved when shared.",
        .restoreOrganization: "Restore its previous collection and tags",
        .deleteStickerTitle: "Move to trash", .deletedSticker: "Moved to trash",
        .restoreStickerTitle: "Restore sticker",
        .restoreHint: "Its collection and tags were not kept",
        .restored: "Restored",
        .telegramToken: "Bot token", .telegramTokenHint: "Create a bot with @BotFather and paste its token here",
        .rememberToken: "Remember in the keychain", .clearToken: "Forget the saved token",
        .tokenFailed: "The token could not be read from or written to the keychain",
        .tokenInvalid: "Telegram rejected this token", .packMissing: "That sticker pack was not found",
        .stickerPack: "Sticker pack", .viewStickers: "Load stickers", .chooseStickers: "Choose stickers",
        .selectNew: "Select the new ones", .clearSelection: "Clear selection",
        .importSelected: "Import selected", .changePack: "Use another pack",
        .alreadyImported: "Already in the library", .inTrash: "In the trash",
        .importReadingFiles: "Reading the dropped files",
        .importRetryFailed: "Retry the failed files",
        .importPendingRestore: "%d waiting to be restored",
        .createBackup: "Create a backup", .restoreBackup: "Restore from a backup",
        .backupHint: "A backup holds the whole library in one file",
        .backupPreview: "Backup contents", .merge: "Merge", .replace: "Replace",
        .replaceHint: "Replacing removes everything the library holds now, and a safety snapshot of it is kept in the library folder",
        .backupMergeHint: "Only what the library does not have is added, and the current organization is kept. Stickers in the trash are not restored",
        .backupMergeSummary: "%d to be added, %d already present",
        .backupSaved: "Backup saved", .backupRestored: "Backup restored",
        .archiveContains: "%d stickers, %d collections, %d tags",
        .outputTab: "Copy & drag", .backupTab: "Backup & restore", .about: "About",
        .outputDefaults: "Default formats",
        .outputCopy: "Copy", .outputCopyHint: "Copy image, copy file and ⌘C",
        .outputShare: "Share and save", .outputShareHint: "The share menu and Save as",
        .outputDrag: "Drag", .outputDragHint: "Files dragged into other apps",
        .outputFooter: "Best compatibility produces PNG for still images and GIF for animations. The format menu temporarily overrides copy and share. Dragging uses its own defaults and prepares the file before hand-off.",
        .outputStatic: "Still", .outputAnimated: "Animated",
        .exportBestCompatibility: "Best compatibility",
        .exportBestCompatibilityHint: "Still: PNG up to 1024 px; animated: GIF up to 512 px / 30 FPS",
        .exportAnimatedCompatibilityHint: "GIF up to 512 px / 30 FPS; colors and transparent edges may change",
        .dragPreparing: "Preparing the file for dragging. Please try again when ready.",
        .dragReady: "File ready. You can drag it now.",
        .followDefaults: "Use defaults", .followDefaultsHint: "Copy and share use the formats chosen in Settings",
        .tokenSaved: "Saved in the keychain", .tokenNotSaved: "Not saved", .forget: "Forget",
        .importFromTelegram: "Import from Telegram",
        .importFromTelegramHint: "Look up a pack by name or link, then pick what to bring in",
        .telegramSettingsHint: "Get a token by creating a bot with @BotFather. A remembered token is kept only in this Mac's keychain.",
        .open: "Open…", .backUp: "Back up…", .backupSavedTo: "Saved to",
        .restoreHintShort: "See what the backup holds before anything is written",
        .chooseFile: "Choose file…", .restoreMode: "Restore mode", .discard: "Discard",
        .checkpointHint: "Saves a snapshot of the library database in the library folder. Original files are not included.",
        .create: "Create", .space: "Space", .cleanUp: "Clean up", .clean: "Clean",
        .cleanExportsHint: "Removes expired temporary files made for copying and sharing; files still on the clipboard are kept. Thumbnails are managed by the app and rebuilt when needed. Originals are never touched.",
        .locations: "Locations",
        .version: "Version %@", .licenses: "Licenses", .mitLicense: "MIT License",
        .quicksandFont: "Quicksand typeface", .openFontLicense: "SIL Open Font License", .view: "View",
        .idCopied: "Copied to the clipboard",
        .importFromFiles: "Import from files…", .importFromTelegramPack: "Import Telegram sticker pack…",
        .useSavedToken: "Using the saved token", .change: "Change",
        .packPlaceholder: "Pack name or t.me/addstickers/… link", .stickerCount: "%d stickers",
        .largerStickers: "Larger stickers", .smallerStickers: "Smaller stickers",
        .defaultStickerSize: "Default sticker size",
        .nothingToPaste: "The clipboard holds no image to import",
        .quickLookUnavailable: "Quick Look cannot show this sticker",
        .noHome: "The home folder could not be located",
        .syncTab: "Sync", .syncServer: "Server address", .syncToken: "Device token",
        .syncTokenHint: "Add a device in the web console and paste its token here.",
        .syncConnect: "Connect and sync", .syncNow: "Sync now", .syncPause: "Pause sync", .syncResume: "Resume sync",
        .syncDisconnect: "Disconnect", .syncDisconnectTitle: "Disconnect sync?",
        .syncDisconnectHint: "Syncing will stop. Stickers on this Mac will not be affected.",
        .syncPending: "Pending", .syncBlocked: "Sync issues",
        .syncBlockedHint: "Some changes could not be synced due to conflicts. Check the web console for details.",
        .syncLastSuccess: "Last synced", .syncNever: "Never", .syncReady: "Up to date",
        .syncRunning: "Syncing…", .syncPaused: "Paused", .syncStatus: "Sync status",
        .syncMergeHint: "Connecting for the first time merges stickers on this Mac with the server. Deletions also sync across devices.",
        .syncKeychainFailed: "Could not access credentials in Keychain. Please allow Keychain access and reconnect.",
        .syncReauthorize: "Update token…",
    ]

    private static let chinese: [L10nKey: String] = [
        .purge: "彻底删除", .emptyTrash: "清空回收站…",
        .purgeConfirmation: "彻底删除这 %d 项内容？此操作无法撤销，并会同步到其他设备。删除合集或标签不会删除其中的图片。",
        .purgedCount: "已彻底删除 %d 项内容", .retryPurgeCleanup: "重试文件清理",
        .purgePartial: "已完成 %d 项，%d 项未完成：%@", .purgeCleanupDone: "文件清理已完成",
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
        .empty: "暂无内容", .applying: "处理中", .more: "更多",
        .hideSidebar: "收起侧栏", .showSidebar: "展开侧栏",
        .expanded: "已展开", .collapsed: "已收起", .removeTag: "移除标签 %@",
        .animatedUsesOriginal: "动图保留原格式；需要静图时使用“复制首帧”。",
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
        .createCheckpoint: "创建检查点", .cleanExports: "清理待分享副本",
        .libraryId: "素材库 ID", .deviceId: "设备 ID",
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
        .exportCompatibleGif: "兼容 GIF",
        .exportCompatibleGifHint: "保留动画，最长边 512 像素；颜色和透明边缘可能变化",
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
        .libraryAdd: "添加", .newCollection: "新建合集", .newTag: "新建标签",
        .moveUp: "上移", .moveDown: "下移", .clearSearch: "清空搜索",
        .failedToLoad: "无法加载该区域",
        .libraryEmpty: "暂无表情", .libraryEmptyHint: "导入图片来建立素材库",
        .starredEmpty: "没有已收藏的表情", .starredEmptyHint: "给表情加星标后会显示在这里",
        .tagEmpty: "没有使用该标签的表情", .tagEmptyHint: "给表情添加该标签后会显示在这里",
        .trashEmpty: "回收站是空的", .trashEmptyHint: "删除的表情会留在这里，直到你恢复",
        .collectionEmpty: "该合集是空的", .collectionEmptyHint: "选择表情后加入该合集",
        .noMatchesHint: "换个关键词试试",
        .selectItems: "选择", .selectAll: "全选", .selectedCount: "已选 %d 张",
        .selectionHint: "点击表情加入所选",
        .selectionLoadedOnly: "全选仅含已加载的表情",
        .selectionOverLimit: "已选 %d 张，超过单次 %d 张上限",
        .batchOrganize: "批量整理", .assignCollection: "加入合集",
        .clearCollection: "移出合集", .addTags: "添加标签", .removeTags: "移除标签",
        .star: "加星标", .unstar: "取消星标",
        .batchResult: "已更新 %d 张，失败 %d 张，跳过 %d 张", .batchRunning: "正在处理 %d 张",
        .details: "详细信息", .fileInfo: "文件", .organization: "整理",
        .noCollection: "未加入合集", .addNote: "添加备注", .firstFrame: "复制首帧",
        .pauseAnimation: "暂停", .playAnimation: "播放",
        .animatedFirstFrame: "当前仅显示首帧预览，发送时仍为动图",
        .restoreOrganization: "恢复原先的合集与标签",
        .deleteStickerTitle: "移至回收站", .deletedSticker: "已移至回收站",
        .restoreStickerTitle: "恢复表情",
        .restoreHint: "恢复后不会带回原有的合集与标签",
        .restored: "已恢复",
        .telegramToken: "机器人 Token", .telegramTokenHint: "用 @BotFather 创建机器人，把它的 Token 粘贴到这里",
        .rememberToken: "记住 Token（钥匙串）", .clearToken: "清除已保存的 Token",
        .tokenFailed: "无法从钥匙串读取或写入 Token",
        .tokenInvalid: "Telegram 拒绝了这个 Token", .packMissing: "未找到该贴纸包",
        .stickerPack: "贴纸包", .viewStickers: "查看贴纸", .chooseStickers: "选择贴纸",
        .selectNew: "选择新增项", .clearSelection: "清空选择",
        .importSelected: "导入所选", .changePack: "更换贴纸包",
        .alreadyImported: "素材库中已有", .inTrash: "在回收站中",
        .importReadingFiles: "正在读取拖入的文件",
        .importRetryFailed: "重试失败的文件",
        .importPendingRestore: "%d 张待恢复",
        .createBackup: "创建备份", .restoreBackup: "从备份恢复",
        .backupHint: "备份会把整个素材库打包成一个文件",
        .backupPreview: "备份内容", .merge: "合并", .replace: "替换",
        .replaceHint: "替换会清除素材库中现有的全部内容，同时会在素材库目录里保留一份安全快照",
        .backupMergeHint: "仅导入本地不存在的内容，保留现有整理结构。回收站中的表情不会恢复",
        .backupMergeSummary: "将新增 %d 项，跳过 %d 项已存在内容",
        .backupSaved: "已导出备份", .backupRestored: "已从备份恢复",
        .archiveContains: "%d 张表情、%d 个合集、%d 个标签",
        .outputTab: "复制与拖拽", .backupTab: "备份与恢复", .about: "关于",
        .outputDefaults: "默认格式",
        .outputCopy: "复制", .outputCopyHint: "复制图片、复制文件和 ⌘C",
        .outputShare: "分享与另存为", .outputShareHint: "分享菜单和另存为",
        .outputDrag: "拖拽", .outputDragHint: "拖到其他应用时的格式",
        .outputFooter: "最佳兼容会将静态图片转为 PNG，动图转为 GIF。素材的格式菜单可以临时覆盖复制与分享。拖拽使用独立的默认设置，并提前准备文件。",
        .outputStatic: "静态", .outputAnimated: "动图",
        .exportBestCompatibility: "最佳兼容",
        .exportBestCompatibilityHint: "静态：最长边 1024 的 PNG；动图：最长边 512、最高 30 FPS 的 GIF",
        .exportAnimatedCompatibilityHint: "输出最长边 512、最高 30 FPS 的 GIF；颜色和透明边缘可能变化",
        .dragPreparing: "正在准备拖拽文件，就绪后请再拖一次",
        .dragReady: "文件已就绪，现在可以拖拽",
        .followDefaults: "跟随默认", .followDefaultsHint: "复制与分享使用设置中的默认格式",
        .tokenSaved: "已保存在钥匙串", .tokenNotSaved: "未保存", .forget: "清除",
        .importFromTelegram: "从 Telegram 导入",
        .importFromTelegramHint: "按名称或链接查找贴纸包，挑选后再导入",
        .telegramSettingsHint: "用 @BotFather 创建机器人即可获得 Token。记住的 Token 只保存在本机钥匙串中。",
        .open: "打开…", .backUp: "备份…", .backupSavedTo: "已保存到",
        .restoreHintShort: "先预览备份内容，确认后才会写入",
        .chooseFile: "选择文件…", .restoreMode: "恢复方式", .discard: "放弃",
        .checkpointHint: "在素材库目录保存一份数据库快照，不包含原图文件。",
        .create: "创建", .space: "空间", .cleanUp: "清理", .clean: "清理",
        .cleanExportsHint: "删除复制、分享时生成且已过期的临时文件，剪贴板正在使用的会保留。缩略图由应用管理，需要时自动重建。原图不受影响。",
        .locations: "位置",
        .version: "版本 %@", .licenses: "许可", .mitLicense: "MIT 许可证",
        .quicksandFont: "Quicksand 字体", .openFontLicense: "SIL 开放字体许可证", .view: "查看",
        .idCopied: "已复制到剪贴板",
        .importFromFiles: "从文件导入…", .importFromTelegramPack: "从 Telegram 贴纸包导入…",
        .useSavedToken: "使用已保存的 Token", .change: "更换",
        .packPlaceholder: "贴纸包名称或 t.me/addstickers/… 链接", .stickerCount: "%d 张贴纸",
        .largerStickers: "放大贴纸", .smallerStickers: "缩小贴纸",
        .defaultStickerSize: "默认贴纸大小",
        .nothingToPaste: "剪贴板里没有可导入的图片",
        .quickLookUnavailable: "无法用快速查看显示这张贴纸",
        .noHome: "无法定位用户主目录",
        .syncTab: "同步", .syncServer: "服务器地址", .syncToken: "设备 Token",
        .syncTokenHint: "在网页端添加设备后，将生成的 Token 填入此处。",
        .syncConnect: "连接并同步", .syncNow: "立即同步", .syncPause: "暂停同步", .syncResume: "恢复同步",
        .syncDisconnect: "断开连接", .syncDisconnectTitle: "断开同步连接？",
        .syncDisconnectHint: "断开后将停止同步，本机已有的表情不会受影响。",
        .syncPending: "待同步", .syncBlocked: "未能同步",
        .syncBlockedHint: "部分内容因冲突未能同步，可在网页端查看详情。",
        .syncLastSuccess: "上次同步", .syncNever: "尚未同步", .syncReady: "已同步",
        .syncRunning: "正在同步…", .syncPaused: "已暂停", .syncStatus: "同步状态",
        .syncMergeHint: "首次连接会自动合并本机与云端的表情。删除操作也会同步到其他设备。",
        .syncKeychainFailed: "无法访问钥匙串中的凭证，请允许访问钥匙串后重新连接。",
        .syncReauthorize: "更换 Token…",
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
