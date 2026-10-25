using System.Globalization;
using System.Buffers.Binary;
using System.IO.Compression;
using System.Text.RegularExpressions;
using System.Threading.Channels;
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
            await ShutdownSequenceAsync();
            ResourceParity(FindRepository());
            await PreferencesAsync();
            await SyncStateMachineAsync();
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
        var trash = new LibraryFilter(LibraryScope.Trash).Query("request", "猫", StickerSort.CollectionOrder);
        Check(trash.Deleted && trash.CollectionId is null && trash.TagIds.Length == 0 && trash.Starred is null
            && trash.Sort == StickerSort.Recent && trash.Text == "猫",
            "The trash scope leaked an organization filter, kept collection ordering or dropped its search text.");
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

    // Shutdown is the last chance to release the library, so a failing step must
    // not skip the rest and a cancelled step is not a failure.
    private static async Task ShutdownSequenceAsync()
    {
        var ran = new List<string>();
        var observerFailure = new InvalidOperationException("observer failed");
        var failure = await ShutdownRun.SequenceAsync(
            () => { ran.Add("observer"); return Task.FromException(observerFailure); },
            () => { ran.Add("reload"); return Task.CompletedTask; },
            () => { ran.Add("trash"); return Task.FromException(new OperationCanceledException()); },
            () => { ran.Add("session"); return Task.FromException(new InvalidOperationException("session failed")); });
        Check(ran.SequenceEqual(["observer", "reload", "trash", "session"]),
            "A failed shutdown step skipped the steps after it.");
        Check(failure is AggregateException { InnerExceptions.Count: 2 } aggregate
            && ReferenceEquals(aggregate.InnerExceptions[0], observerFailure),
            "Shutdown lost the first failure or dropped a later one.");
        Check(await ShutdownRun.SequenceAsync(() => Task.FromException(new OperationCanceledException())) is null,
            "Cancelling during shutdown was reported as a failure.");
        var ordered = new List<string>();
        Check(await ShutdownRun.SequenceAsync(
                () => { ordered.Add("first"); return Task.CompletedTask; },
                () => { ordered.Add("second"); return Task.CompletedTask; }) is null
            && ordered.SequenceEqual(["first", "second"]),
            "A clean shutdown did not run its steps in order.");
        Console.WriteLine("PASS: shutdown keeps releasing resources after a failed background task");
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
                await TrashAsync(session, root);
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
        var syncStatus = await session.SyncStatusAsync(token);
        Check(!syncStatus.Connected && syncStatus.Pending == 0 && syncStatus.Blocked == 0 && syncStatus.Error is null,
            "A fresh library reported an unexpected sync binding.");
        var unbound = await session.SetSyncPausedAsync(false, token);
        Check(!unbound.Connected, "Touching sync on an unbound library created a binding.");
        Console.WriteLine("PASS: real detail editing, organization, order, soft-delete/restore, original preservation, unbound sync state and partial batch failure");
    }

    // Exercises the recycle bin against a real library: the trash listing is a
    // native query, restore and purge are core operations, and every failure
    // must leave the row and its file where they were.
    private static async Task TrashAsync(LibrarySession session, string root)
    {
        var token = CancellationToken.None;
        static EntityReference Ref(StickerMetadata item) => new(item.Id, item.Lifecycle.Generation);
        static string Key(Exception error) => error is BridgeException.Failure failure
            ? failure.code.ToString() : error.GetType().Name;

        var missing = new string('0', 64);
        var collection = await session.CreateCollectionAsync("回收测试", token);
        var tag = await session.CreateTagAsync("trash-tag", token);
        var purplePath = Path.Combine(root, "purple.png");
        var cyanPath = Path.Combine(root, "cyan.png");
        await File.WriteAllBytesAsync(purplePath, Png(160, 0, 200));
        await File.WriteAllBytesAsync(cyanPath, Png(0, 200, 200));
        var purple = await session.ImportAsync(purplePath, collection.Id, token);
        var cyan = await session.ImportAsync(cyanPath, null, token);
        var purpleItem = await session.PatchAsync(Ref(purple.Sticker), new("紫色贴纸", "trash note", true), token);
        await session.OrganizeAsync(Ref(purpleItem), true, new(collection.Id, collection.Lifecycle.Generation),
            [new(tag.Id, tag.Lifecycle.Generation)], token);

        var deletedPurple = await session.DeleteStickerAsync(Ref(purpleItem), token);
        var deletedCyan = await session.DeleteStickerAsync(Ref(cyan.Sticker), token);

        using (var active = await session.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, token))
            Check(!active.Stickers.Any(item => item.Id == purple.Sticker.Id || item.Id == cyan.Sticker.Id),
                "A deleted sticker stayed in the normal listing.");
        using (var trash = await session.QueryAsync(new(LibraryScope.Trash), "", StickerSort.Recent, null, token))
        {
            Check(trash.Stickers.Select(item => item.Id).OrderBy(id => id)
                    .SequenceEqual(new[] { purple.Sticker.Id, cyan.Sticker.Id }.OrderBy(id => id)),
                "The trash listing did not contain exactly the deleted stickers.");
            Check(trash.Next is null, "A two-item trash page returned another cursor.");
        }
        using (var search = await session.QueryAsync(new(LibraryScope.Trash), "紫色", StickerSort.Recent, null, token))
            Check(search.Stickers.Length == 1 && search.Stickers[0].Id == purple.Sticker.Id,
                "Searching the trash did not reach the native query.");

        var restored = await TrashOperations.RestoreAsync(session, [Entry(deletedPurple)], Key, token);
        Check(restored is { Succeeded: 1, Failed: 0, ErrorKey: null }, "Restoring a trashed sticker did not succeed once.");
        using (var trash = await session.QueryAsync(new(LibraryScope.Trash), "", StickerSort.Recent, null, token))
            Check(!trash.Stickers.Any(item => item.Id == purple.Sticker.Id), "A restored sticker stayed in the trash listing.");
        using (var active = await session.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, token))
            Check(active.Stickers.Any(item => item.Id == purple.Sticker.Id), "A restored sticker did not return to the normal listing.");
        var detail = await session.DetailAsync(purple.Sticker.Id, token);
        Check(detail.Sticker.Title == "紫色贴纸" && detail.Sticker.Note == "trash note" && detail.Sticker.Starred,
            "Restoring a sticker lost metadata or the star.");
        Check(detail.Sticker.Lifecycle.Generation > deletedPurple.Lifecycle.Generation, "Restore did not advance the generation.");
        // Restore raises the generation, so the pre-deletion collection and tag
        // relations stop being effective (the core keeps them as suggestions).
        // Reassigning is a separate core write, which the client performs when
        // the user wants the placement back.
        Check(detail.Collection is null && detail.Tags.Length == 0,
            "Restore silently reattached organization the core detaches.");
        await session.OrganizeAsync(new(purple.Sticker.Id, detail.Sticker.Lifecycle.Generation), true,
            new(collection.Id, collection.Lifecycle.Generation), [new(tag.Id, tag.Lifecycle.Generation)], token);
        detail = await session.DetailAsync(purple.Sticker.Id, token);
        Check(detail.Collection?.Id == collection.Id && detail.Tags.Any(item => item.Id == tag.Id),
            "Reassigning organization after restore failed.");
        Check(File.Exists(detail.OriginalPath), "Restore did not bring the original back.");

        var activeEntry = new TrashEntry(purple.Sticker.Id, detail.Sticker.Lifecycle.Generation, detail.Sticker.Lifecycle.Revision);
        var onActive = await TrashOperations.PurgeAsync(session, [activeEntry], Key, token);
        Check(onActive is { Succeeded: 0, Failed: 1, ErrorKey: "Conflict" }, "An active sticker was purged instead of rejected.");
        Check(File.Exists(detail.OriginalPath), "A rejected purge deleted the original file.");
        var restoredAgain = await TrashOperations.RestoreAsync(session, [activeEntry], Key, token);
        Check(restoredAgain is { Succeeded: 0, Failed: 1, ErrorKey: "Conflict" }, "Restoring an active sticker was not rejected.");
        var mixed = await TrashOperations.RestoreAsync(session,
            [activeEntry, new TrashEntry(missing, 1, 1)], Key, token);
        Check(mixed is { Succeeded: 0, Failed: 2, ErrorKey: "Conflict" },
            "A batch stopped at the first failure or lost the first error.");

        var cyanDetail = await session.DetailAsync(cyan.Sticker.Id, token);
        var cyanOriginal = cyanDetail.OriginalPath;
        Check(cyanOriginal is not null && File.Exists(cyanOriginal), "A trashed sticker lost its original too early.");
        var purged = await TrashOperations.PurgeAsync(session, [Entry(deletedCyan)], Key, token);
        Check(purged is { Succeeded: 1, Failed: 0, ErrorKey: null }, "Purging a trashed sticker did not succeed once.");
        using (var active = await session.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, token))
            Check(!active.Stickers.Any(item => item.Id == cyan.Sticker.Id), "A purged sticker was still listed as active.");
        using (var trash = await session.QueryAsync(new(LibraryScope.Trash), "", StickerSort.Recent, null, token))
            Check(!trash.Stickers.Any(item => item.Id == cyan.Sticker.Id), "A purged sticker was still listed in the trash.");
        Check(!File.Exists(cyanOriginal), "Purging a sticker left its original file behind.");
        // The tombstone row stays for sync, so the detail still resolves; the
        // original is gone and the core reports why instead of a path.
        var purgedDetail = await session.DetailAsync(cyan.Sticker.Id, token);
        Check(purgedDetail.OriginalPath is null && purgedDetail.OriginalError is not null,
            "A purged sticker still reported a readable original.");

        var deletedAgain = await session.DeleteStickerAsync(
            new(purple.Sticker.Id, detail.Sticker.Lifecycle.Generation), token);
        var stale = await TrashOperations.PurgeAsync(session,
            [new TrashEntry(purple.Sticker.Id, deletedAgain.Lifecycle.Generation, deletedAgain.Lifecycle.Revision - 1)], Key, token);
        Check(stale is { Succeeded: 0, Failed: 1, ErrorKey: "Conflict" }, "A stale purge revision was accepted.");
        using (var trash = await session.QueryAsync(new(LibraryScope.Trash), "", StickerSort.Recent, null, token))
            Check(trash.Stickers.Any(item => item.Id == purple.Sticker.Id), "A rejected purge dropped the sticker from the trash.");
        var staleDetail = await session.DetailAsync(purple.Sticker.Id, token);
        Check(File.Exists(staleDetail.OriginalPath), "A rejected purge deleted the original file.");

        var current = new TrashEntry(purple.Sticker.Id, staleDetail.Sticker.Lifecycle.Generation, staleDetail.Sticker.Lifecycle.Revision);
        Check((await TrashOperations.PurgeAsync(session, [current], Key, token)) is { Succeeded: 1, Failed: 0 },
            "Purging with the current revision failed.");
        Check((await TrashOperations.PurgeAsync(session, [current], Key, token)) is { Succeeded: 1, Failed: 0 },
            "Repeating a purge on an already purged sticker was reported as a failure.");
        Check((await TrashOperations.PurgeAsync(session, [new TrashEntry(missing, 1, 1)], Key, token))
            is { Succeeded: 0, Failed: 1, ErrorKey: "NotFound" }, "Purging an unknown sticker did not report not-found.");
        Check((await TrashOperations.RestoreAsync(session, [new TrashEntry(missing, 1, 1)], Key, token))
            is { Succeeded: 0, Failed: 1, ErrorKey: "NotFound" }, "Restoring an unknown sticker did not report not-found.");
        Check((await TrashOperations.RestoreAsync(session, [new TrashEntry("missing-sticker", 1, 1)], Key, token))
            is { Succeeded: 0, Failed: 1, ErrorKey: "InvalidInput" }, "A malformed sticker id was not rejected as invalid input.");

        using (var trash = await session.QueryAsync(new(LibraryScope.Trash), "", StickerSort.Recent, null, token))
            Check(trash.Stickers.Length == 0 && trash.Next is null, "The emptied trash still listed stickers.");
        using (var active = await session.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, token))
            Check(active.Stickers.Length == 2, "Trash operations changed the active listing of unrelated stickers.");

        await session.DeleteCollectionAsync(new(collection.Id, collection.Lifecycle.Generation), token);
        await session.DeleteTagAsync(new(tag.Id, tag.Lifecycle.Generation), token);
        Console.WriteLine("PASS: real trash listing, search, restore, purge, idempotence and failure consistency");
    }

    // The trash entry uses the revision observed when the sticker was listed, so
    // a concurrent change fails for that entry only.
    private static TrashEntry Entry(StickerMetadata item) => new(item.Id, item.Lifecycle.Generation, item.Lifecycle.Revision);

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

    private static async Task SyncStateMachineAsync()
    {
        const string library = "library-under-test";
        var tokens = new FakeSyncTokens();
        var adapter = new FakeSyncAdapter();
        var sync = new SyncCoordinator(adapter, tokens, library, TimeSpan.FromMilliseconds(10));

        await sync.StartAsync(runLoop: false);
        Check(sync.State.Loaded && !sync.State.HasCredential && !sync.State.Status.Connected
              && SyncPresentation.StatusKey(sync.State) == "sync_authorize"
              && SyncPresentation.NeedsAuthorization(sync.State),
            "A fresh library did not report the disconnected sync state.");
        Check(adapter.Subscriptions.Count == 1, "Sync status was not observed exactly once.");

        adapter.Status = SyncSnapshot(connected: true, pending: 2);
        await sync.ConnectAsync("  https://sync.example.com/  ", "mdk_first");
        Check(adapter.ConnectCalls == 1 && adapter.LastOrigin == "https://sync.example.com/"
              && adapter.LastToken == "mdk_first", "Connecting did not trim or forward the server address.");
        Check(tokens.Saved.TryGetValue(library + "|https://sync.example.com/", out var stored) && stored == "mdk_first",
            "Connecting did not persist the device token.");
        Check(sync.State.HasCredential && sync.State.Status.Connected
              && SyncPresentation.StatusKey(sync.State) == "sync_ready",
            "A successful connection did not produce the ready state.");

        adapter.ConnectGate = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var second = sync.ConnectAsync("https://sync.example.com/", "mdk_second");
        await WaitForAsync(() => adapter.ConnectCalls == 2, "the second connect to start");
        var third = sync.ConnectAsync("https://sync.example.com/", "mdk_third");
        Check(ReferenceEquals(second, third),
            "A repeated connect request started a separate command instead of reusing the running one.");
        adapter.ConnectGate.SetResult();
        adapter.ConnectGate = null;
        await Task.WhenAll(second, third);
        Check(adapter.ConnectCalls == 2 && adapter.LastToken == "mdk_second",
            "Repeated connect requests created more than one native session.");

        adapter.Status = SyncSnapshot(connected: true, paused: true);
        await sync.SetPausedAsync(true);
        Check(sync.State.Status.Paused && SyncPresentation.StatusKey(sync.State) == "sync_paused",
            "Pausing was not reflected in the sync state.");
        adapter.Status = SyncSnapshot(connected: true);
        await sync.SetPausedAsync(false);
        Check(!sync.State.Status.Paused && SyncPresentation.StatusKey(sync.State) == "sync_ready",
            "Resuming was not reflected in the sync state.");

        adapter.Status = SyncSnapshot(connected: true, pending: 5);
        adapter.Push(adapter.Status);
        await WaitForAsync(() => sync.State.Status.Pending == 5, "native status updates");
        Check(sync.State.Status.Pending == 5, "A pushed native status did not reach the state.");

        adapter.Status = SyncSnapshot(connected: true, lastSuccess: 1_700_000_000_000);
        await sync.SynchronizeNowAsync();
        Check(adapter.SynchronizeCalls == 1 && sync.State.Status.LastSuccessMs == 1_700_000_000_000
              && SyncPresentation.StatusKey(sync.State) == "sync_ready",
            "A successful manual sync did not update the state.");

        adapter.Status = SyncSnapshot(connected: true, error: "network");
        adapter.SynchronizeFailure = new InvalidOperationException("offline");
        await sync.SynchronizeNowAsync();
        Check(SyncPresentation.ErrorKey(sync.State) == "sync_failure_network"
              && SyncPresentation.StatusKey(sync.State) == "sync_failed"
              && !SyncPresentation.IsTerminal(sync.State),
            "A network failure did not produce a retryable sync error.");

        adapter.SynchronizeFailure = null;
        adapter.Status = SyncSnapshot(connected: true, lastSuccess: 1_700_000_100_000);
        await sync.SynchronizeNowAsync();
        Check(sync.State.Status.LastSuccessMs == 1_700_000_100_000 && SyncPresentation.ErrorKey(sync.State) is null,
            "Retrying after a network failure did not clear the error.");

        var callsBeforeTerminal = adapter.SynchronizeCalls;
        adapter.Status = SyncSnapshot(connected: true, error: "unauthorized");
        adapter.SynchronizeFailure = new InvalidOperationException("expired");
        await sync.SynchronizeNowAsync();
        Check(SyncPresentation.ErrorKey(sync.State) == "sync_failure_unauthorized"
              && SyncPresentation.IsTerminal(sync.State)
              && SyncPresentation.NeedsAuthorization(sync.State),
            "An unauthorized response did not ask for a new token.");
        await sync.SynchronizeNowAsync();
        Check(adapter.SynchronizeCalls == callsBeforeTerminal + 1,
            "A terminal authorization failure kept retrying automatically.");

        // The adapter reports failures through BridgeException.Failure, which the
        // coordinator presents from the exception rather than from a status string.
        var unauthorized = new SyncState(
            SyncSnapshot(connected: true), Loaded: true, Running: false, HasCredential: true, Changing: false,
            Error: new BridgeException.Failure(ErrorCode.Unauthorized, "expired"));
        Check(SyncPresentation.NeedsAuthorization(unauthorized)
              && SyncPresentation.IsTerminal(unauthorized)
              && !SyncPresentation.CanSynchronize(unauthorized)
              && SyncPresentation.StatusKey(unauthorized) == "sync_authorize"
              && SyncPresentation.ErrorKey(unauthorized) == "sync_failure_unauthorized",
            "A bridge authorization failure was not presented as a re-authorization request.");

        var credentialFailure = new SyncState(
            SyncSnapshot(connected: true), Loaded: true, Running: false, HasCredential: true, Changing: false,
            Error: new SyncCredentialException("credential store unavailable"));
        Check(SyncPresentation.ErrorKey(credentialFailure) == "sync_failure_credential"
              && SyncPresentation.StatusKey(credentialFailure) == "sync_authorize"
              && !SyncPresentation.CanSynchronize(credentialFailure),
            "A credential store failure was not presented as a re-authorization request.");

        adapter.Push(SyncSnapshot(connected: true));
        await WaitForAsync(() => sync.State.Status.Error is null, "the cleared sync error");
        adapter.Status = SyncSnapshot(connected: true);
        adapter.SynchronizeFailure = null;
        adapter.SynchronizeGate = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var first = sync.SynchronizeNowAsync();
        await WaitForAsync(() => adapter.SynchronizeCalls == callsBeforeTerminal + 2, "the first sync to start");
        var duplicate = sync.SynchronizeNowAsync();
        await Task.Delay(50);
        Check(adapter.SynchronizeCalls == callsBeforeTerminal + 2,
            "A repeated sync request started a second native sync.");
        adapter.SynchronizeGate.SetResult();
        await Task.WhenAll(first, duplicate);
        adapter.SynchronizeGate = null;

        tokens.FailSave = true;
        adapter.Status = SyncSnapshot(connected: true);
        await sync.ConnectAsync("https://sync.example.com/", "mdk_unsaved");
        Check(adapter.LastPaused && !sync.State.HasCredential
              && SyncPresentation.ErrorKey(sync.State) == "sync_failure_credential"
              && SyncPresentation.StatusKey(sync.State) == "sync_authorize",
            "A token that could not be stored left sync enabled.");
        tokens.FailSave = false;

        adapter.Status = SyncSnapshot(connected: true);
        await sync.ConnectAsync("https://sync.example.com/", "mdk_stored");
        Check(sync.State.HasCredential, "A restorable connection was not established before the credential test.");

        tokens.FailLoad = true;
        await sync.SynchronizeNowAsync();
        Check(!sync.State.HasCredential
              && SyncPresentation.ErrorKey(sync.State) == "sync_failure_credential",
            "A credential read failure was not reported as a credential error.");
        tokens.FailLoad = false;

        adapter.Status = SyncSnapshot(connected: true);
        await sync.ConnectAsync("https://sync.example.com/", "mdk_final");
        await sync.DisconnectAsync();
        Check(!sync.State.HasCredential && adapter.LastPaused
              && SyncPresentation.StatusKey(sync.State) == "sync_authorize"
              && !tokens.Saved.ContainsKey(library + "|https://sync.example.com/"),
            "Disconnecting kept syncing or left the token behind.");

        await sync.DisposeAsync();
        var connectsBeforeLateCall = adapter.ConnectCalls;
        await sync.ConnectAsync("https://sync.example.com/", "mdk_late");
        Check(adapter.ConnectCalls == connectsBeforeLateCall,
            "A disposed coordinator still accepted commands.");
        await sync.DisposeAsync();
        Check(adapter.Subscriptions.TrueForAll(subscription => subscription.Closed),
            "Disposing the coordinator left the status subscription open.");

        await SyncAutomaticLoopAsync();
        await SyncShutdownAsync();
        await SyncCredentialScopeAsync();
        Console.WriteLine("PASS: sync state machine, credential handling, deduplication and disposal");
    }

    private static async Task SyncAutomaticLoopAsync()
    {
        const string library = "loop-library";
        const string origin = "https://sync.example.com/";
        var tokens = new FakeSyncTokens();
        tokens.Saved[library + "|" + origin] = "mdk_loop";
        var adapter = new FakeSyncAdapter { Status = SyncSnapshot(connected: true) };
        // A ten-second floor keeps the core's 20 ms cadence from masquerading as a
        // wake-up: any sync inside the waits below can only come from a signal.
        var sync = new SyncCoordinator(adapter, tokens, library, TimeSpan.FromSeconds(10));
        await sync.StartAsync();
        try
        {
            await WaitForAsync(() => adapter.SynchronizeCalls == 1, "the first automatic sync");
            Check(sync.State.Status.Connected && sync.State.HasCredential,
                "The automatic loop did not keep the connected state.");
            await Task.Delay(200);
            Check(adapter.SynchronizeCalls == 1,
                "The automatic loop ignored the retry cadence the core asked for.");
            adapter.Push(SyncSnapshot(connected: true, pending: 3, retry: 30_000));
            await Task.Delay(200);
            Check(adapter.SynchronizeCalls == 1,
                "A pushed status shortened the retry cadence, so a failure could be retried immediately.");
            sync.NotifyContentChanged();
            await WaitForAsync(() => adapter.SynchronizeCalls == 2, "a sync after a local change");
        }
        finally
        {
            await sync.DisposeAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(5));
        }
        Console.WriteLine("PASS: automatic sync cadence and local-change wake-up");
    }

    private static async Task SyncShutdownAsync()
    {
        var tokens = new FakeSyncTokens();
        tokens.Saved["shutdown-library|" + "https://sync.example.com/"] = "mdk_shutdown";
        var adapter = new FakeSyncAdapter
        {
            Status = SyncSnapshot(connected: true),
            SynchronizeGate = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously)
        };
        var sync = new SyncCoordinator(adapter, tokens, "shutdown-library", TimeSpan.FromMilliseconds(10));
        await sync.StartAsync(runLoop: false);
        try
        {
            var running = sync.SynchronizeNowAsync();
            await WaitForAsync(() => adapter.SynchronizeCalls == 1, "an in-flight sync");
            var duplicate = sync.SynchronizeNowAsync();
            Check(ReferenceEquals(running, duplicate),
                "A coalesced sync request started a separate command instead of reusing the running one.");
            sync.RequestStop();
            await sync.DisposeAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(5));
            await Task.WhenAll(running, duplicate).WaitAsync(TimeSpan.FromSeconds(5));
            Check(!sync.State.Running && adapter.Subscriptions.TrueForAll(subscription => subscription.Closed),
                "Shutdown left the running flag or the status stream open.");
            Console.WriteLine("PASS: in-flight sync cancellation and shutdown");
        }
        finally
        {
            adapter.SynchronizeGate.TrySetResult();
        }
    }

    private static async Task SyncCredentialScopeAsync()
    {
        const string origin = "https://sync.example.com/";
        const string library = "scope-library";
        var tokens = new FakeSyncTokens();
        tokens.Saved[library + "|" + origin] = "mdk_scope";
        tokens.Saved["other-library|" + origin] = "mdk_other";
        var adapter = new FakeSyncAdapter { Status = SyncSnapshot(connected: true) };
        var sync = new SyncCoordinator(adapter, tokens, library, TimeSpan.FromMilliseconds(10));
        await sync.StartAsync(runLoop: false);
        try
        {
            await sync.DisconnectAsync();
            Check(!tokens.Saved.ContainsKey(library + "|" + origin)
                  && tokens.Saved.ContainsKey("other-library|" + origin),
                "Disconnecting one library removed another library's token.");
            Check(!sync.State.HasCredential && sync.State.Status.Paused && adapter.LastPaused,
                "Disconnecting did not pause the binding or drop the local credential flag.");
            Console.WriteLine("PASS: credential scoping across libraries");
        }
        finally
        {
            await sync.DisposeAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(5));
        }
    }

    private static SyncStatus SyncSnapshot(
        string? origin = "https://sync.example.com/",
        bool connected = true,
        bool paused = false,
        ulong pending = 0,
        ulong blocked = 0,
        long cursor = 0,
        long? lastSuccess = null,
        string? error = null,
        ulong retry = 20)
        => new(origin, connected ? "device-under-test" : null, connected, paused, pending, blocked,
            cursor, lastSuccess, error, retry);

    private static async Task WaitForAsync(Func<bool> condition, string message, int timeoutMilliseconds = 3000)
    {
        var deadline = DateTime.UtcNow.AddMilliseconds(timeoutMilliseconds);
        while (!condition())
        {
            if (DateTime.UtcNow > deadline) throw new InvalidOperationException($"Timed out waiting for {message}.");
            await Task.Delay(10);
        }
    }

    private sealed class FakeSyncTokens : ISyncTokenStore
    {
        public Dictionary<string, string> Saved { get; } = new(StringComparer.Ordinal);
        public bool FailLoad { get; set; }
        public bool FailSave { get; set; }

        public Task<string?> LoadAsync(string libraryId, string origin, CancellationToken cancellation)
            => FailLoad
                ? Task.FromException<string?>(new IOException("credential store unavailable"))
                : Task.FromResult(Saved.TryGetValue(Key(libraryId, origin), out var token) ? token : null);

        public Task SaveAsync(string libraryId, string origin, string token, CancellationToken cancellation)
        {
            if (FailSave) return Task.FromException(new IOException("credential store unavailable"));
            Saved[Key(libraryId, origin)] = token;
            return Task.CompletedTask;
        }

        public Task ClearAsync(string libraryId, string origin, CancellationToken cancellation)
        {
            Saved.Remove(Key(libraryId, origin));
            return Task.CompletedTask;
        }

        private static string Key(string libraryId, string origin) => libraryId + "|" + origin;
    }

    private sealed class FakeSyncAdapter : ISyncAdapter
    {
        public SyncStatus Status { get; set; } = SyncSnapshot(connected: false, origin: null);
        public List<FakeSyncSubscription> Subscriptions { get; } = [];
        public int ConnectCalls { get; private set; }
        public int SynchronizeCalls { get; private set; }
        public string? LastOrigin { get; private set; }
        public string? LastToken { get; private set; }
        public bool LastPaused { get; private set; }
        public Exception? SynchronizeFailure { get; set; }
        public TaskCompletionSource? SynchronizeGate { get; set; }
        public TaskCompletionSource? ConnectGate { get; set; }

        public Task<SyncStatus> StatusAsync(CancellationToken cancellation) => Task.FromResult(Status);

        public async Task<SyncStatus> ConnectAsync(string origin, string token, CancellationToken cancellation)
        {
            ConnectCalls++;
            LastOrigin = origin;
            LastToken = token;
            if (ConnectGate is not null) await ConnectGate.Task.WaitAsync(cancellation);
            Status = SyncSnapshot(origin: origin);
            return Status;
        }

        public async Task<SyncStatus> SynchronizeAsync(string token, CancellationToken cancellation)
        {
            SynchronizeCalls++;
            LastToken = token;
            if (SynchronizeGate is not null) await SynchronizeGate.Task.WaitAsync(cancellation);
            if (SynchronizeFailure is not null) throw SynchronizeFailure;
            return Status;
        }

        public Task<SyncStatus> SetPausedAsync(bool paused, CancellationToken cancellation)
        {
            LastPaused = paused;
            Status = SyncSnapshot(origin: Status.Origin, paused: paused);
            return Task.FromResult(Status);
        }

        public ISyncSubscription Subscribe()
        {
            var subscription = new FakeSyncSubscription();
            Subscriptions.Add(subscription);
            return subscription;
        }

        public void Push(SyncStatus status)
        {
            Status = status;
            foreach (var subscription in Subscriptions) subscription.Push(status);
        }
    }

    private sealed class FakeSyncSubscription : ISyncSubscription
    {
        private readonly Channel<SyncStatus?> _channel = Channel.CreateUnbounded<SyncStatus?>();
        public bool Closed { get; private set; }

        public async Task<SyncStatus?> NextAsync()
        {
            try { return await _channel.Reader.ReadAsync(); }
            catch (ChannelClosedException) { return null; }
        }

        public void Push(SyncStatus? status) => _channel.Writer.TryWrite(status);

        public void Close()
        {
            Closed = true;
            _channel.Writer.TryComplete();
        }

        public void Dispose() => Close();
    }
}
