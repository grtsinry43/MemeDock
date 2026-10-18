using System.Globalization;
using System.Buffers.Binary;
using System.IO.Compression;
using System.Text.RegularExpressions;
using System.Xml.Linq;
using MemeDock.Bridge;
using MemeDock.Windows;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;

internal static class Program
{
    private static async Task<int> Main()
    {
        try
        {
            QueryScopes();
            RequestGenerations();
            EditDrafts();
            OutputPolicies();
            ResourceParity(FindRepository());
            await PreferencesAsync();
            await SessionQueriesAsync();
            Console.WriteLine("PASS: Windows browsing state, localization and preference checks");
            return 0;
        }
        catch (Exception error) { Console.Error.WriteLine(error); return 1; }
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }

    private static void QueryScopes()
    {
        var all = LibraryFilter.All.Query("request", "  猫 cat  ", StickerSort.Recent);
        Check(all.Text == "猫 cat" && all.CollectionId is null && all.TagIds.Length == 0 && all.Starred is null && !all.Deleted,
            "All-library search acquired an implicit organization or deletion filter.");
        var starred = new LibraryFilter(LibraryScope.Starred).Query("request", "", StickerSort.Recent);
        Check(starred.Starred == true && starred.CollectionId is null, "Starred filter did not reach the native query.");
        var recent = new LibraryFilter(LibraryScope.Recent).Query("request", "", StickerSort.Recent);
        Check(recent.Sort == StickerSort.LastUsed, "Recently used was queried as recently imported.");
        var collection = new LibraryFilter(LibraryScope.Collection, "collection-id", "合集");
        var ordered = collection.Query("request", "猫", StickerSort.CollectionOrder);
        Check(ordered.CollectionId == "collection-id" && ordered.Text == "猫" && ordered.Sort == StickerSort.CollectionOrder,
            "Search escaped its collection or lost collection ordering.");
        var tag = new LibraryFilter(LibraryScope.Tag, "tag-id").Query("request", "", StickerSort.CollectionOrder);
        Check(tag.TagIds.SequenceEqual(["tag-id"]) && tag.Sort == StickerSort.Recent && tag.CollectionId is null,
            "Tag filtering leaked collection ordering.");
        try
        {
            new LibraryFilter(LibraryScope.Collection).Query("request", "", StickerSort.Recent);
            throw new InvalidOperationException("An unidentified collection became an all-library query.");
        }
        catch (ArgumentException) { }
        Console.WriteLine("PASS: native query scoping and sort rules");
    }

    private static void RequestGenerations()
    {
        var state = new BrowseRequestState();
        var oldSearch = state.Begin();
        var newSearch = state.Begin();
        Check(!state.IsCurrent(oldSearch) && state.IsCurrent(newSearch), "A late search could replace newer results.");
        state.Invalidate();
        Check(!state.IsCurrent(newSearch), "Closing the library still admitted a pending result.");
        Console.WriteLine("PASS: stale search and shutdown result rejection");
    }

    private static string FindRepository()
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory); directory is not null; directory = directory.Parent)
            if (File.Exists(Path.Combine(directory.FullName, "AGENTS.md")) && Directory.Exists(Path.Combine(directory.FullName, "apps/android")))
                return directory.FullName;
        throw new DirectoryNotFoundException("Run the checks from a checkout of MemeDock.");
    }

    private static void OutputPolicies()
    {
        var animated = StickerOutputPolicy.Options(true, StickerOutputKind.Image);
        Check(animated.Preset == ExportPreset.Original && animated.Animation == AnimationPolicy.Preserve
            && !animated.StripMetadata && animated.MaxEdge is null, "Default animated copy would flatten or re-encode the original.");
        var image = StickerOutputPolicy.Options(false, StickerOutputKind.Image);
        Check(image.Preset == ExportPreset.CompatiblePng && image.MaxEdge == 1024 && image.BackgroundRgba is null
            && image.StripMetadata, "Static copy lost the approved transparent PNG limits.");
        foreach (var isAnimated in new[] { false, true })
        {
            var original = StickerOutputPolicy.Options(isAnimated, StickerOutputKind.OriginalFile);
            Check(original.Preset == ExportPreset.Original && original.Animation == AnimationPolicy.Preserve && !original.StripMetadata,
                "File transfer would modify original bytes.");
            var frame = StickerOutputPolicy.Options(isAnimated, StickerOutputKind.FirstFrame);
            Check(frame.Preset == ExportPreset.CompatiblePng && frame.Animation == AnimationPolicy.FirstFrame && frame.MaxEdge == 1024,
                "First-frame conversion was implicit or lost its bounds.");
        }
        var metadata = new StickerMetadata("sticker", "title", "image.gif", "", false, new(2, 1, 0, 0, null));
        var target = new StickerOutputTarget("sticker", 2);
        target.Validate(metadata);
        try { target.Validate(metadata with { Lifecycle = metadata.Lifecycle with { Generation = 3 } }); throw new InvalidOperationException("Stale output target accepted."); }
        catch (BridgeException.Failure failure) { Check(failure.code == ErrorCode.Conflict, "Wrong stale-target error."); }
        try { target.Validate(metadata with { Lifecycle = metadata.Lifecycle with { DeletedAt = 1 } }); throw new InvalidOperationException("Deleted target accepted."); }
        catch (BridgeException.Failure failure) { Check(failure.code == ErrorCode.EntityDeleted, "Wrong deleted-target error."); }
        Console.WriteLine("PASS: original/PNG/first-frame output policy and captured-target validation");
    }

    private static void EditDrafts()
    {
        var original = new StickerMetadata("sticker", "old", "original.png", "note", false, new EntityLifecycle(1, 1, 0, 0, null, null));
        var draft = new StickerEditState(original) { Title = "sent" };
        var sent = draft.Capture();
        Check(sent.Target == new EntityReference("sticker", 1) && sent.Edit.Note is null, "Patch included unrelated draft fields.");
        draft.Title = "typed while saving";
        draft.Note = "new note";
        var saved = original with { Title = "sent", Lifecycle = original.Lifecycle with { Revision = 2 } };
        Check(draft.Accept(saved, sent) && draft.Title == "typed while saving" && draft.Note == "new note" && draft.IsDirty,
            "A save completion overwrote edits typed after submission.");
        Check(!draft.Accept(saved with { Id = "other" }, sent), "A different sticker accepted this draft.");
        Check(!draft.Accept(saved with { Lifecycle = saved.Lifecycle with { Generation = 2 } }, sent), "Restored generation accepted a stale draft.");
        Check(!draft.Accept(original, sent), "A late revision replaced newer saved metadata.");
        draft.Discard();
        Check(!draft.IsDirty && draft.Title == "sent" && draft.Note == "note", "Discard lost the accepted baseline.");
        draft.Title = "  trimmed  ";
        sent = draft.Capture();
        Check(draft.Accept(saved with { Title = "trimmed", Lifecycle = saved.Lifecycle with { Revision = 3 } }, sent)
            && !draft.IsDirty && draft.Title == "trimmed", "Canonical saved text was not accepted.");
        var reverted = new StickerEditState(original) { Title = "sent", Note = "sent note" };
        sent = reverted.Capture();
        reverted.Title = original.Title;
        reverted.Note = original.Note;
        Check(reverted.Accept(saved with { Note = "sent note" }, sent) && reverted.Title == original.Title
            && reverted.Note == original.Note && reverted.IsDirty, "Typing the original text during a save was lost.");
        Console.WriteLine("PASS: draft ownership, in-flight typing, generation/revision rejection and discard");
    }

    private static Dictionary<string, string> ReadResw(string path)
    {
        var entries = XDocument.Load(path).Root!.Elements("data").ToArray();
        Check(entries.Length == entries.Select(entry => (string)entry.Attribute("name")!).Distinct().Count(), $"Duplicate resource keys: {path}");
        return entries.ToDictionary(entry => (string)entry.Attribute("name")!, entry => entry.Element("value")!.Value);
    }

    private static void ResourceParity(string root)
    {
        var chinese = ReadResw(Path.Combine(root, "apps/windows/app/Strings/zh-CN/Resources.resw"));
        var english = ReadResw(Path.Combine(root, "apps/windows/app/Strings/en-US/Resources.resw"));
        Check(chinese.Keys.Order().SequenceEqual(english.Keys.Order()), "Chinese and English resource keys differ.");
        foreach (var file in Directory.EnumerateFiles(Path.Combine(root, "apps/windows/app"), "*.cs", SearchOption.AllDirectories)
            .Where(path => !path.Contains(Path.DirectorySeparatorChar + "obj" + Path.DirectorySeparatorChar, StringComparison.Ordinal)))
            foreach (Match match in Regex.Matches(File.ReadAllText(file), "(?:Strings|_strings)\\[\"([^\"]+)\"\\]"))
                Check(chinese.ContainsKey(match.Groups[1].Value), $"UI references missing resource {match.Groups[1].Value}: {file}");
        foreach (var key in chinese.Keys)
        {
            var zhArguments = Regex.Matches(chinese[key], @"\{\d+\}").Select(match => match.Value).Order();
            var enArguments = Regex.Matches(english[key], @"\{\d+\}").Select(match => match.Value).Order();
            Check(zhArguments.SequenceEqual(enArguments), $"Placeholder mismatch: {key}");
            Check(!string.IsNullOrWhiteSpace(chinese[key]) && !string.IsNullOrWhiteSpace(english[key]), $"Empty resource: {key}");
        }
        var desktopKeys = new Dictionary<string, string>
        {
            ["desktop_settings"] = "settings", ["desktop_sort"] = "sortHint", ["desktop_sort_recent"] = "sortRecent",
            ["desktop_sort_collection"] = "sortCollectionOrder", ["desktop_select_all"] = "selectAll",
            ["desktop_clear_selection"] = "clearSelection", ["desktop_selection_loaded_only"] = "selectionLoadedOnly",
            ["desktop_details"] = "details", ["desktop_selection_over_limit"] = "selectionOverLimit"
        };
        var macos = File.ReadAllText(Path.Combine(root, "apps/macos/Sources/MemeDock/Support/Localization.swift"));
        var split = macos.IndexOf("private static let chinese:", StringComparison.Ordinal);
        foreach (var (locale, resources, desktopSection) in new[]
        {
            ("values", chinese, macos[split..]), ("values-en", english, macos[..split])
        })
        {
            var android = XDocument.Load(Path.Combine(root, $"apps/android/app/src/main/res/{locale}/strings.xml"))
                .Root!.Elements("string").ToDictionary(entry => (string)entry.Attribute("name")!, entry => entry.Value);
            foreach (var (key, value) in resources)
            {
                if (desktopKeys.TryGetValue(key, out var sourceKey))
                {
                    var match = Regex.Match(desktopSection, "\\." + sourceKey + ": \"([^\"]*)\"");
                    var position = 0;
                    var desktop = Regex.Replace(match.Groups[1].Value, "%d", _ => "{" + position++ + "}");
                    Check(match.Success && desktop == value, $"Desktop copy differs from existing macOS copy: {key}/{locale}");
                    continue;
                }
                if (key == "desktop_collection_delete_hint")
                {
                    Check(android["organize_collections_footer"].EndsWith(value, StringComparison.Ordinal), "Collection deletion hint differs from Android.");
                    continue;
                }
                Check(android.TryGetValue(key, out var source), $"Resource has no approved Android/desktop source: {key}");
                var expected = Regex.Replace(source!, @"%(\d+)\$[ds]", match =>
                    "{" + (int.Parse(match.Groups[1].Value, CultureInfo.InvariantCulture) - 1) + "}");
                Check(expected == value, $"Windows copy differs from Android: {key}/{locale}");
            }
        }
        Console.WriteLine($"PASS: {chinese.Count} bilingual resource keys, placeholders and source-copy parity");
    }

    private static async Task SessionQueriesAsync()
    {
        var parent = Path.TrimEndingDirectorySeparator(Path.GetFullPath(Path.GetTempPath()));
        var root = Path.Combine(parent, "MemeDock-Windows-session-" + Guid.NewGuid().ToString("N"));
        try
        {
            var configuration = new LibraryConfiguration(Path.Combine(root, "data"), Path.Combine(root, "cache"),
                Path.Combine(root, "share"), MemeDockNative.DefaultResourceConfiguration());
            string collectionId;
            string tagId;
            using (var library = await MemeDockNative.OpenLibrary(configuration))
            {
                using var collection = library.CreateCollection("中文合集", null);
                collectionId = (await collection.AwaitResult()).Id;
                using var tag = library.CreateTag("猫");
                tagId = (await tag.AwaitResult()).Id;
                await library.Shutdown();
            }
            var redPath = Path.Combine(root, "猫🙂.png");
            var greenPath = Path.Combine(root, "green.png");
            await File.WriteAllBytesAsync(redPath, Png(255, 0, 0));
            await File.WriteAllBytesAsync(greenPath, Png(0, 255, 0));
            await using (var session = await LibrarySession.OpenAsync(root))
            {
                var first = await session.ImportAsync(redPath, collectionId, CancellationToken.None);
                await session.ImportAsync(greenPath, null, CancellationToken.None);
                Check((await session.CollectionsAsync(CancellationToken.None)).Single().Name == "中文合集", "Collection catalog did not cross the adapter.");
                Check((await session.TagsAsync(CancellationToken.None)).Single().Id == tagId, "Tag catalog did not cross the adapter.");
                using var all = await session.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, CancellationToken.None);
                Check(all.Stickers.Length == 2, "Adapter did not return both real imports.");
                using var scoped = await session.QueryAsync(new(LibraryScope.Collection, collectionId), "猫", StickerSort.CollectionOrder, null, CancellationToken.None);
                Check(scoped.Stickers.Length == 1 && scoped.Stickers[0].Id == first.Sticker.Id, "Collection search returned unrelated stickers.");
                using var noMatch = await session.QueryAsync(new(LibraryScope.Collection, collectionId), "green", StickerSort.Recent, null, CancellationToken.None);
                Check(noMatch.Stickers.Length == 0, "Collection search escaped into the rest of the library.");
                using var tagQuery = await session.QueryAsync(new(LibraryScope.Tag, tagId), "", StickerSort.Recent, null, CancellationToken.None);
                Check(tagQuery.Stickers.Length == 0, "An empty tag returned untagged stickers.");
                Check(File.Exists(await session.ThumbnailAsync(first.Sticker.Id, CancellationToken.None)), "Adapter did not resolve a real thumbnail.");
                using var cancelled = new CancellationTokenSource();
                cancelled.Cancel();
                try
                {
                    using var result = await session.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, cancelled.Token);
                    throw new InvalidOperationException("An already cancelled query was admitted.");
                }
                catch (OperationCanceledException) { }
                await ManagementAsync(session, first.Sticker, redPath);
                await OutputContractsAsync(session, first.Sticker.Id, redPath);
            }
            await using (var reopened = await LibrarySession.OpenAsync(root))
            {
                using var page = await reopened.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, CancellationToken.None);
                Check(page.Stickers.Length == 2, "Adapter shutdown/reopen lost data or retained the library lock.");
            }
            Console.WriteLine("PASS: real session import, scoped search, catalogs, cancellation, thumbnail and reopen");
        }
        finally
        {
            if (Directory.Exists(root) && string.Equals(Path.GetDirectoryName(root), parent, StringComparison.OrdinalIgnoreCase)
                && Path.GetFileName(root).StartsWith("MemeDock-Windows-session-", StringComparison.Ordinal))
                Directory.Delete(root, recursive: true);
        }
    }

    private static async Task ManagementAsync(LibrarySession session, StickerMetadata first, string source)
    {
        var token = CancellationToken.None;
        static EntityReference Ref(StickerMetadata item) => new(item.Id, item.Lifecycle.Generation);
        var c1 = await session.CreateCollectionAsync("整理一", token);
        var c2 = await session.CreateCollectionAsync("整理二", token);
        c1 = await session.RenameCollectionAsync(new(c1.Id, c1.Lifecycle.Generation), "Renamed", token);
        await session.MoveCollectionAsync(new(c2.Id, c2.Lifecycle.Generation), c1.Id, token);
        var catalog = await session.CollectionsAsync(token);
        Check(Array.FindIndex(catalog, item => item.Id == c2.Id) < Array.FindIndex(catalog, item => item.Id == c1.Id), "Collection order did not persist.");
        var tag = await session.CreateTagAsync("待整理", token);
        tag = await session.RenameTagAsync(new(tag.Id, tag.Lifecycle.Generation), "Reviewed", token);
        var tagRef = new EntityReference(tag.Id, tag.Lifecycle.Generation);
        var c1Ref = new EntityReference(c1.Id, c1.Lifecycle.Generation);
        var edited = await session.PatchAsync(Ref(first), new("Edited title", "中文 note", true), token);
        await session.OrganizeAsync(Ref(edited), true, c1Ref, [tagRef], token);
        var detail = await session.DetailAsync(first.Id, token);
        Check(detail.Sticker.Title == "Edited title" && detail.Sticker.Note == "中文 note" && detail.Sticker.Starred
            && detail.Collection?.Id == c1.Id && detail.Tags.Single().Id == tag.Id, "Editing or organization lost fields.");
        Check((await File.ReadAllBytesAsync(detail.OriginalPath!)).SequenceEqual(await File.ReadAllBytesAsync(source)), "Organization modified the original.");
        using var page = await session.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, token);
        var second = page.Stickers.Single(item => item.Id != first.Id);
        await session.OrganizeAsync(Ref(second), true, c1Ref, [], token);
        await session.MoveCollectionItemAsync(c1Ref, Ref(second), first.Id, token);
        using var ordered = await session.QueryAsync(new(LibraryScope.Collection, c1.Id), "", StickerSort.CollectionOrder, null, token);
        Check(ordered.Stickers.Select(item => item.Id).SequenceEqual([second.Id, first.Id]), "Collection item order did not persist.");
        var deleted = await session.DeleteStickerAsync(Ref(edited), token);
        var restored = await session.RestoreStickerAsync(Ref(deleted), deleted.Lifecycle.Revision, token);
        Check(restored.Lifecycle.Generation > first.Lifecycle.Generation, "Restore did not advance generation.");
        using (var batch = await session.StartBatchAsync([new(Ref(first), null), new(Ref(second), null)], new BatchAction.Star(true), token))
        {
            var report = await batch.AwaitResult();
            Check(report.Items.Single(item => item.Id == first.Id).Outcome == BatchOutcome.Failed
                && report.Items.Single(item => item.Id == second.Id).Outcome == BatchOutcome.Applied,
                "Batch did not distinguish stale-generation failure from successful items.");
        }
        using (var batch = await session.StartBatchAsync([new(Ref(restored), null), new(Ref(second), null)], new BatchAction.AddTags([tagRef]), token))
        {
            Check((await batch.AwaitResult()).Items.All(item => item.Outcome is BatchOutcome.Applied or BatchOutcome.Unchanged), "Batch tag assignment failed.");
        }
        await session.OrganizeAsync(Ref(restored), true, null, [], token);
        detail = await session.DetailAsync(first.Id, token);
        Check(detail.Collection is null && detail.Tags.Length == 0, "Clearing organization retained relations.");
        var removedTag = await session.DeleteTagAsync(tagRef, token);
        Check(!(await session.TagsAsync(token)).Any(item => item.Id == tag.Id), "Deleted tag stayed active.");
        await session.RestoreTagAsync(new(tag.Id, removedTag.Lifecycle.Generation), removedTag.Lifecycle.Revision, token);
        var removedCollection = await session.DeleteCollectionAsync(c1Ref, token);
        Check(!(await session.CollectionsAsync(token)).Any(item => item.Id == c1.Id), "Deleted collection stayed active.");
        await session.RestoreCollectionAsync(new(c1.Id, removedCollection.Lifecycle.Generation), removedCollection.Lifecycle.Revision, token);
        Check((await session.TagsAsync(token)).Any(item => item.Id == tag.Id) && (await session.CollectionsAsync(token)).Any(item => item.Id == c1.Id),
            "Catalog undo failed to restore entries.");
        Check((await session.DetailAsync(second.Id, token)).Sticker.Starred, "Deleting a collection deleted its sticker.");
        Console.WriteLine("PASS: real detail editing, organization, order, soft-delete/restore, original preservation and partial batch failure");
    }

    private static async Task OutputContractsAsync(LibrarySession session, string id, string source)
    {
        var token = CancellationToken.None;
        var detail = await session.DetailAsync(id, token);
        var target = new StickerOutputTarget(id, detail.Sticker.Lifecycle.Generation);
        target.Validate(detail.Sticker);
        using var original = await session.ExportAsync(id, StickerOutputPolicy.Options(false, StickerOutputKind.OriginalFile), token);
        var artifact = await session.PrepareHandoffAsync(original, token);
        Check((await File.ReadAllBytesAsync(artifact.Path)).SequenceEqual(await File.ReadAllBytesAsync(source)), "Original handoff changed bytes.");
        Check(artifact.RetainedUntil > DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(), "Handoff did not durably retain its file.");
        var pin = await session.ProtectClipboardAsync(original, token);
        await session.ReconcileClipboardAsync(pin);
        var failedPublication = await session.ProtectClipboardAsync(original, token);
        await session.AbortClipboardAsync(failedPublication);
        await session.ReconcileClipboardAsync(pin);
        using var png = await session.ExportAsync(id, StickerOutputPolicy.Options(false, StickerOutputKind.Image), token);
        var converted = await session.PrepareHandoffAsync(png, token);
        Check(converted.Mime == "image/png" && !converted.Animated && File.Exists(converted.Path), "PNG export did not publish a verified static file.");
        var replacement = await session.ProtectClipboardAsync(png, token);
        await session.ReconcileClipboardAsync(replacement);
        await session.ReconcileClipboardAsync(null);
        await session.RecordUseAsync(id, UsageAction.CopyImage);
        Check((await File.ReadAllBytesAsync(detail.OriginalPath!)).SequenceEqual(await File.ReadAllBytesAsync(source)), "Copy changed library originals.");
        var gif = AnimatedGif();
        var gifPath = Path.Combine(Path.GetDirectoryName(source)!, "animated.gif");
        await File.WriteAllBytesAsync(gifPath, gif);
        var imported = await session.ImportAsync(gifPath, null, token);
        var moving = await session.DetailAsync(imported.Sticker.Id, token);
        Check(moving.Asset.Animated, "Two-frame fixture was not recognized as animated.");
        using var movingOriginal = await session.ExportAsync(imported.Sticker.Id,
            StickerOutputPolicy.Options(moving.Asset.Animated, StickerOutputKind.Image), token);
        var movingArtifact = await session.PrepareHandoffAsync(movingOriginal, token);
        Check(movingArtifact.Animated && (await File.ReadAllBytesAsync(movingArtifact.Path)).SequenceEqual(gif),
            "Default animated copy changed the original GIF or lost animation.");
        using var firstFrame = await session.ExportAsync(imported.Sticker.Id,
            StickerOutputPolicy.Options(true, StickerOutputKind.FirstFrame), token);
        var frameArtifact = await session.PrepareHandoffAsync(firstFrame, token);
        var frameBytes = await File.ReadAllBytesAsync(frameArtifact.Path);
        Check(!frameArtifact.Animated && frameArtifact.Mime == "image/png"
            && frameBytes.Take(8).SequenceEqual(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 }),
            "Explicit first-frame copy did not produce static PNG.");
        Check((await File.ReadAllBytesAsync(moving.OriginalPath!)).SequenceEqual(gif), "First-frame conversion changed the animated original.");
        await session.DeleteStickerAsync(new(imported.Sticker.Id, imported.Sticker.Lifecycle.Generation), token);
        Console.WriteLine("PASS: real export, durable handoff, clipboard replacement/abort, usage recording and immutable originals");
        Console.WriteLine("PASS: real animated GIF preservation and explicit first-frame PNG conversion");
    }

    // GIF89a: 1px red/green frames, a two-color global palette and 100ms delays.
    // Each LZW block contains clear(4), one palette index, end(5), packed LSB-first.
    private static byte[] AnimatedGif()
    {
        using var output = new MemoryStream();
        output.Write("GIF89a"u8);
        output.Write(new byte[] { 1, 0, 1, 0, 0x80, 0, 0, 255, 0, 0, 0, 255, 0 });
        foreach (var codes in new byte[] { 0x44, 0x4c })
        {
            output.Write(new byte[] { 0x21, 0xf9, 4, 0, 10, 0, 0, 0 });
            output.Write(new byte[] { 0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0 });
            output.Write(new byte[] { 2, 2, codes, 1, 0 });
        }
        output.WriteByte(0x3b);
        return output.ToArray();
    }

    // A valid 1px RGBA PNG, using the same fixture construction as NativeContracts.
    private static byte[] Png(byte red, byte green, byte blue)
    {
        using var output = new MemoryStream();
        output.Write(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 });
        var header = new byte[13];
        BinaryPrimitives.WriteUInt32BigEndian(header.AsSpan(0, 4), 1);
        BinaryPrimitives.WriteUInt32BigEndian(header.AsSpan(4, 4), 1);
        header[8] = 8;
        header[9] = 6;
        Chunk(output, "IHDR"u8, header);
        using var compressed = new MemoryStream();
        using (var zlib = new ZLibStream(compressed, CompressionLevel.Fastest, leaveOpen: true))
            zlib.Write(new byte[] { 0, red, green, blue, 255 });
        Chunk(output, "IDAT"u8, compressed.ToArray());
        Chunk(output, "IEND"u8, []);
        return output.ToArray();
    }

    private static void Chunk(Stream output, ReadOnlySpan<byte> type, ReadOnlySpan<byte> bytes)
    {
        Span<byte> word = stackalloc byte[4];
        BinaryPrimitives.WriteUInt32BigEndian(word, (uint)bytes.Length);
        output.Write(word);
        output.Write(type);
        output.Write(bytes);
        uint crc = uint.MaxValue;
        foreach (var value in type.ToArray().Concat(bytes.ToArray()))
        {
            crc ^= value;
            for (var bit = 0; bit < 8; bit++) crc = (crc >> 1) ^ (0xedb88320u & (0u - (crc & 1)));
        }
        BinaryPrimitives.WriteUInt32BigEndian(word, ~crc);
        output.Write(word);
    }

    private static async Task PreferencesAsync()
    {
        var parent = Path.GetFullPath(Path.GetTempPath());
        var root = Path.Combine(parent, "MemeDock-Windows-preferences-" + Guid.NewGuid().ToString("N"));
        try
        {
            var preferences = new AppPreferences(root);
            await preferences.LoadAsync();
            Check(preferences.Language == LanguageMode.System, "A fresh install did not follow the system language.");
            await preferences.SaveLanguageAsync(LanguageMode.Chinese);
            var reopened = new AppPreferences(root);
            await reopened.LoadAsync();
            Check(reopened.Language == LanguageMode.Chinese, "Language did not survive a process restart.");
            await Task.WhenAll(preferences.SaveLanguageAsync(LanguageMode.English), preferences.SaveLanguageAsync(LanguageMode.System));
            await reopened.LoadAsync();
            Check(reopened.Language == LanguageMode.System, "Concurrent preference writes lost the final selection.");
            await File.WriteAllTextAsync(Path.Combine(root, "preferences.json"), "broken json");
            var damaged = new AppPreferences(root);
            await damaged.LoadAsync();
            Check(damaged.Language == LanguageMode.System, "Damaged preferences prevented system-language fallback.");
            Console.WriteLine("PASS: preference persistence, serialized writes and damaged-file recovery");
        }
        finally
        {
            if (Directory.Exists(root) && string.Equals(Path.GetDirectoryName(root), Path.TrimEndingDirectorySeparator(parent), StringComparison.OrdinalIgnoreCase)
                && Path.GetFileName(root).StartsWith("MemeDock-Windows-preferences-", StringComparison.Ordinal))
                Directory.Delete(root, recursive: true);
        }
    }
}
