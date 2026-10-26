using System.Globalization;
using System.Buffers.Binary;
using System.ComponentModel;
using System.IO.Compression;
using System.Text.RegularExpressions;
using System.Threading.Channels;
using System.Xml.Linq;
using MemeDock.Bridge;
using MemeDock.Windows;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using MemeDock.Windows.ViewModels;

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
            OutputPresetTable();
            VersionMetadata(FindRepository());
            CatalogFeedback();
            await RefreshMergingAsync();
            await TrashGroupsModelAsync();
            await ShutdownSequenceAsync();
            ResourceParity(FindRepository());
            SyncStatusBrushes(FindRepository());
            await PreferencesAsync();
            SyncStatusMapping();
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

    private static async Task<ErrorCode> ExpectFailure(Func<Task> operation)
    {
        try { await operation(); }
        catch (BridgeException.Failure failure) { return failure.code; }
        throw new InvalidOperationException("Expected a bridge failure.");
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
        Check(OutputDefaults.CopyDefault.Still == ExportPreset.BestCompatibility
              && OutputDefaults.CopyDefault.Animated == ExportPreset.Original,
            "Copy defaults drifted from best-compatible stills and unchanged animations.");
        Check(OutputDefaults.DragDefault.Still == ExportPreset.Original
              && OutputDefaults.DragDefault.Animated == ExportPreset.Original,
            "Drag defaults must keep handing over the original bytes.");

        var animated = StickerOutputPolicy.Options(true, StickerOutputKind.Image, OutputDefaults.CopyDefault);
        Check(animated.Preset == ExportPreset.Original && animated.Animation == AnimationPolicy.Preserve
            && !animated.StripMetadata && animated.MaxEdge is null, "Default animated copy would flatten or re-encode the original.");
        var image = StickerOutputPolicy.Options(false, StickerOutputKind.Image, OutputDefaults.CopyDefault);
        Check(image.Preset == ExportPreset.BestCompatibility && image.MaxEdge is null && image.BackgroundRgba is null
            && image.StripMetadata, "Static copy lost the approved best-compatibility limits.");
        // An untouched drag follows the drag defaults, which are the original bytes,
        // in both content kinds.
        foreach (var isAnimated in new[] { false, true })
        {
            var dragged = StickerOutputPolicy.Options(isAnimated, StickerOutputKind.Image, OutputDefaults.DragDefault);
            Check(dragged.Preset == ExportPreset.Original && !dragged.StripMetadata && dragged.Animation == AnimationPolicy.Preserve,
                "Default drag changed the original bytes.");
        }
        foreach (var defaults in new[] { OutputDefaults.CopyDefault, OutputDefaults.DragDefault })
        {
            foreach (var isAnimated in new[] { false, true })
            {
                var original = StickerOutputPolicy.Options(isAnimated, StickerOutputKind.OriginalFile, defaults);
                Check(original.Preset == ExportPreset.Original && original.Animation == AnimationPolicy.Preserve && !original.StripMetadata,
                    "File transfer would modify original bytes.");
                var frame = StickerOutputPolicy.Options(isAnimated, StickerOutputKind.FirstFrame, defaults);
                Check(frame.Preset == ExportPreset.CompatiblePng && frame.Animation == AnimationPolicy.FirstFrame && frame.MaxEdge == 1024,
                    "First-frame conversion was implicit, lost its bounds or followed a preset.");
            }
        }
        var metadata = new StickerMetadata("sticker", "title", "image.gif", "", false, new(2, 1, 0, 0, null, null));
        var target = new StickerOutputTarget("sticker", 2);
        target.Validate(metadata);
        try { target.Validate(metadata with { Lifecycle = metadata.Lifecycle with { Generation = 3 } }); throw new InvalidOperationException("Stale output target accepted."); }
        catch (BridgeException.Failure failure) { Check(failure.code == ErrorCode.Conflict, "Wrong stale-target error."); }
        try { target.Validate(metadata with { Lifecycle = metadata.Lifecycle with { DeletedAt = 1 } }); throw new InvalidOperationException("Deleted target accepted."); }
        catch (BridgeException.Failure failure) { Check(failure.code == ErrorCode.EntityDeleted, "Wrong deleted-target error."); }
        Console.WriteLine("PASS: original/PNG/first-frame output policy and captured-target validation");
    }

    /// <summary>
    /// The settings page writes one of these presets, so the full parameter tuple is
    /// pinned here. <c>crates/domain/src/export.rs::ExportOptions::for_preset</c> is the
    /// authority; a mismatch means the app would ask for options the domain rejects.
    /// </summary>
    private static void OutputPresetTable()
    {
        Check(StickerOutputPresets.For(false).SequenceEqual(
                [ExportPreset.Original, ExportPreset.CompatiblePng, ExportPreset.SmallJpeg, ExportPreset.BestCompatibility]),
            "The still-image preset list changed.");
        Check(StickerOutputPresets.For(true).SequenceEqual([ExportPreset.Original, ExportPreset.CompatibleGif]),
            "The animated preset list must not offer still-only presets.");

        (ExportPreset Preset, uint? MaxEdge, uint? Background, bool Strip)[] stills =
        [
            (ExportPreset.Original, null, null, false),
            (ExportPreset.CompatiblePng, 1024, null, true),
            (ExportPreset.SmallJpeg, 512, 0xFFFFFFFF, true),
            (ExportPreset.BestCompatibility, null, null, true),
        ];
        foreach (var (preset, maxEdge, background, strip) in stills)
        {
            var options = StickerOutputPresets.Options(preset, animated: false);
            Check(options.Preset == preset && options.MaxEdge == maxEdge && options.BackgroundRgba == background
                  && options.StripMetadata == strip && options.Animation == AnimationPolicy.Preserve,
                $"Still preset {preset} does not match ExportOptions::for_preset.");
        }
        (ExportPreset Preset, uint? MaxEdge, uint? Background, bool Strip)[] animations =
        [
            (ExportPreset.Original, null, null, false),
            (ExportPreset.CompatibleGif, 512, null, true),
        ];
        foreach (var (preset, maxEdge, background, strip) in animations)
        {
            var options = StickerOutputPresets.Options(preset, animated: true);
            Check(options.Preset == preset && options.MaxEdge == maxEdge && options.BackgroundRgba == background
                  && options.StripMetadata == strip && options.Animation == AnimationPolicy.Preserve,
                $"Animated preset {preset} does not match ExportOptions::for_preset.");
        }
        Check(!StickerOutputPresets.Supports(ExportPreset.CompatibleGif, animated: false)
              && !StickerOutputPresets.Supports(ExportPreset.SmallJpeg, animated: true)
              && !StickerOutputPresets.Supports(ExportPreset.CompatiblePng, animated: true),
            "A content kind accepted a preset the domain would reject.");
        Check(new OutputDefaults(ExportPreset.CompatibleGif, ExportPreset.SmallJpeg).Sanitized(OutputDefaults.CopyDefault)
                == OutputDefaults.CopyDefault,
            "A hand-edited file with swapped presets was not sanitized.");
        Check(new OutputDefaults(ExportPreset.SmallJpeg, ExportPreset.CompatibleGif).Sanitized(OutputDefaults.CopyDefault)
                == new OutputDefaults(ExportPreset.SmallJpeg, ExportPreset.CompatibleGif),
            "Valid presets were replaced during sanitizing.");

        // Every label and hint the settings page can render must exist in both files.
        var root = FindRepository();
        var english = ReadResw(Path.Combine(root, "apps/windows/app/Strings/en-US/Resources.resw"));
        var chinese = ReadResw(Path.Combine(root, "apps/windows/app/Strings/zh-CN/Resources.resw"));
        foreach (var animated in new[] { false, true })
        {
            foreach (var preset in StickerOutputPresets.For(animated))
            {
                var label = StickerOutputPresets.LabelKey(preset);
                var hint = StickerOutputPresets.HintKey(preset, animated);
                Check(english.ContainsKey(label) && chinese.ContainsKey(label), $"Missing preset label: {label}");
                Check(english.ContainsKey(hint) && chinese.ContainsKey(hint), $"Missing preset hint: {hint}");
            }
        }
        Console.WriteLine("PASS: every output preset maps to the domain parameters and to bilingual copy");
    }

    private static void VersionMetadata(string root)
    {
        Check(AppVersion.Display("0.2.0+3f2a1c", new Version(9, 9, 9)) == "0.2.0",
            "The commit suffix leaked into the displayed version.");
        Check(AppVersion.Display(" 0.2.0 ", null) == "0.2.0", "The displayed version kept surrounding whitespace.");
        Check(AppVersion.Display(null, new Version(1, 2, 3, 4)) == "1.2.3", "The assembly version lost its patch number.");
        Check(AppVersion.Display("", null) == AppVersion.Unknown, "A missing version did not fall back to the placeholder.");

        var workspace = AppVersion.WorkspaceVersion(File.ReadAllText(Path.Combine(root, "Cargo.toml")));
        Check(workspace is { Length: > 0 }, "The workspace version could not be read from Cargo.toml.");
        var project = XDocument.Load(Path.Combine(root, "apps/windows/app/MemeDock.Windows.csproj"))
            .Root!.Elements("PropertyGroup").Elements("Version").Select(element => element.Value).FirstOrDefault();
        Check(project == workspace, $"Windows app version {project} differs from the workspace version {workspace}.");
        Check(AppVersion.FromAssembly(typeof(Program).Assembly).Length > 0, "Assembly metadata produced no version.");
        Console.WriteLine("PASS: version display, workspace alignment and commit-suffix trimming");
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

    // The operation the user asked for and the refresh that follows it are two
    // results. A failed refresh must not read as a failed restore or purge, and
    // only a completed operation may announce success.
    private static void CatalogFeedback()
    {
        Check(TrashGroupsPresentation.VisibleErrorKey(null, null) is null,
            "A clean catalog reported an error.");
        Check(TrashGroupsPresentation.VisibleErrorKey(null, "E_RUNTIME") == TrashGroupsPresentation.RefreshFailureKey,
            "A failed refresh was not reported as a list failure.");
        Check(TrashGroupsPresentation.VisibleErrorKey("E_CONFLICT", null) == "E_CONFLICT",
            "A failed operation lost its own error.");
        Check(TrashGroupsPresentation.VisibleErrorKey("E_CONFLICT", "E_RUNTIME") == "E_CONFLICT",
            "A refresh failure replaced the operation failure instead of staying secondary.");
        Check(TrashGroupsPresentation.ShowsMessage(true, "restored")
              && !TrashGroupsPresentation.ShowsMessage(false, "restored")
              && !TrashGroupsPresentation.ShowsMessage(true, null),
            "Success was announced for a failed operation or without a message.");
        Check(TrashGroupsPresentation.ExtraPurgeHintKey(true) == "desktop_collection_delete_hint"
              && TrashGroupsPresentation.ExtraPurgeHintKey(false) is null,
            "The collection-only purge hint leaked into the tag dialog.");
        Console.WriteLine("PASS: trash operation/refresh feedback separation and purge copy");
    }

    // A catalog notification must never be dropped, but a burst must not turn
    // into a burst of reads either: one operation ends with one refreshed list.
    private static async Task RefreshMergingAsync()
    {
        var heldPasses = 0;
        var held = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var queue = new RefreshQueue(async () =>
        {
            Interlocked.Increment(ref heldPasses);
            await held.Task;
            return true;
        });
        queue.Suspend();
        var firstQueued = queue.Request();
        var secondQueued = queue.Request();
        Check(firstQueued.IsCompleted && secondQueued.IsCompleted,
            "A refresh requested during an operation waited for a pass that had not started.");
        Check(Volatile.Read(ref heldPasses) == 0, "A suspended queue refreshed while the operation was running.");
        var resumed = queue.Resume();
        Check(Volatile.Read(ref heldPasses) == 1, "Finishing an operation did not run the refresh it held back.");
        held.SetResult();
        await resumed;
        Check(Volatile.Read(ref heldPasses) == 1,
            "Two notifications during one operation produced more than one refresh.");

        // A notification during a running pass latches exactly one follow-up,
        // and the caller settles after that follow-up instead of the stale pass.
        var started = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var release = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var livePasses = 0;
        var live = new RefreshQueue(async () =>
        {
            var index = Interlocked.Increment(ref livePasses);
            if (index == 1) started.SetResult();
            if (index == 1) await release.Task;
            return true;
        });
        var running = live.Request();
        await started.Task;
        var mergedOne = live.Request();
        var mergedTwo = live.Request();
        Check(!mergedOne.IsCompleted, "A refresh requested during a pass settled before the follow-up ran.");
        release.SetResult();
        await running;
        Check(Volatile.Read(ref livePasses) == 2,
            "A burst during a running pass did not collapse into exactly one follow-up pass.");
        Check(mergedOne.IsCompleted && mergedTwo.IsCompleted, "A merged refresh was never completed.");

        // A pass that fails reports through the view model; the queue itself
        // must survive so a later notification still refreshes the list.
        var attempts = 0;
        var recovering = new RefreshQueue(() => Interlocked.Increment(ref attempts) == 1
            ? Task.FromException<bool>(new InvalidOperationException("pass failed"))
            : Task.FromResult(true));
        await recovering.Request();
        Check(Volatile.Read(ref attempts) == 1, "The failing pass did not run.");
        await recovering.Request();
        Check(Volatile.Read(ref attempts) == 2, "A failed pass ended the queue and lost later notifications.");

        // Closing discards what is queued, still waits for the pass in flight,
        // and accepts nothing afterwards.
        var closingStart = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var closingRelease = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var closingPasses = 0;
        var closing = new RefreshQueue(async () =>
        {
            Interlocked.Increment(ref closingPasses);
            closingStart.SetResult();
            await closingRelease.Task;
            return true;
        });
        var inFlight = closing.Request();
        await closingStart.Task;
        _ = closing.Request();
        closing.Stop();
        Check(!closing.Completion.IsCompleted, "Stopping released a pass that was still querying.");
        closingRelease.SetResult();
        await inFlight;
        await closing.Completion;
        Check(Volatile.Read(ref closingPasses) == 1, "A queued refresh ran after the panel stopped.");
        await closing.Resume();
        await closing.Request();
        Check(Volatile.Read(ref closingPasses) == 1, "A stopped queue accepted a refresh during shutdown.");
        Console.WriteLine("PASS: trash refresh coalescing, failure recovery and shutdown discard");
    }

    // The view model wiring behind those rules: a committed operation keeps its
    // success even when the follow-up read fails, a new operation does not
    // inherit the previous banner, and a closing panel publishes nothing.
    private static async Task TrashGroupsModelAsync()
    {
        var catalog = new FakeTrashCatalog();
        var strings = new FakeCatalogStrings();
        var collection = new CollectionMetadata(
            "c-restore", "合集", "a", new EntityLifecycle(1, 0, 0, 0, 100, null));

        // Operation succeeds, refresh fails: success message plus a list error.
        var reloads = 0;
        catalog.RestoreCollection = _ => Task.FromResult(collection);
        catalog.Collections = _ => Task.FromException<CollectionMetadata[]>(new InvalidOperationException("refresh failed"));
        await using (var model = new TrashGroupsViewModel(catalog, strings, () =>
        {
            reloads++;
            return Task.CompletedTask;
        }))
        {
            Check(await model.RestoreCollectionAsync(collection), "A committed restore was reported as failed.");
            Check(model.MessageKey == "restored", "A committed restore did not announce success.");
            Check(model.MessageText.Length > 0, "The success message was not resolved.");
            Check(model.ErrorKey == TrashGroupsPresentation.RefreshFailureKey,
                "A refresh failure turned a committed restore into an operation failure.");
            Check(reloads == 1, "A restore did not reload the library catalogs.");
        }

        // Both fail: the operation's reason stays the visible one.
        catalog = new FakeTrashCatalog
        {
            RestoreCollection = _ => Task.FromException<CollectionMetadata>(new InvalidOperationException("restore failed")),
            Collections = _ => Task.FromException<CollectionMetadata[]>(new InvalidOperationException("refresh failed"))
        };
        await using (var model = new TrashGroupsViewModel(catalog, strings, () => Task.CompletedTask))
        {
            Check(!await model.RestoreCollectionAsync(collection), "A failed restore was reported as committed.");
            Check(model.ErrorKey == "restore failed", "The refresh failure replaced the operation failure.");
            Check(model.MessageKey is null, "A failed operation announced success.");
        }

        // Operation fails, refresh works: only the operation error remains.
        catalog = new FakeTrashCatalog
        {
            RestoreCollection = _ => Task.FromException<CollectionMetadata>(new InvalidOperationException("restore failed")),
            Collections = _ => Task.FromResult(Array.Empty<CollectionMetadata>())
        };
        await using (var model = new TrashGroupsViewModel(catalog, strings, () => Task.CompletedTask))
        {
            Check(!await model.RestoreCollectionAsync(collection), "A failed restore was reported as committed.");
            Check(model.ErrorKey == "restore failed", "A failed operation lost its error after a clean refresh.");
            Check(model.MessageKey is null, "A failed operation announced success.");
        }

        // The previous failure must not survive the next successful action.
        catalog = new FakeTrashCatalog
        {
            RestoreCollection = _ => Task.FromException<CollectionMetadata>(new InvalidOperationException("first failure")),
            Collections = _ => Task.FromResult(Array.Empty<CollectionMetadata>())
        };
        await using (var model = new TrashGroupsViewModel(catalog, strings, () => Task.CompletedTask))
        {
            Check(!await model.RestoreCollectionAsync(collection), "A failed restore was reported as committed.");
            Check(model.ErrorKey == "first failure", "The failed operation did not report its own reason.");
            catalog.RestoreCollection = _ => Task.FromResult(collection);
            Check(await model.RestoreCollectionAsync(collection), "The retried restore did not commit.");
            Check(model.ErrorKey is null, "An old operation error stayed visible after the next success.");
            Check(model.MessageKey == "restored", "The retried restore did not announce success.");
        }

        // A notification subscriber that throws is isolated: the committed
        // result stands, the subscribers after it still receive the
        // notification, and the operation's lifecycle (busy flag, held-back
        // refresh, disposal) is untouched.
        var escapedReads = 0;
        catalog = new FakeTrashCatalog { RestoreCollection = _ => Task.FromResult(collection) };
        catalog.Collections = _ =>
        {
            Interlocked.Increment(ref escapedReads);
            return Task.FromResult(Array.Empty<CollectionMetadata>());
        };
        var escaped = new TrashGroupsViewModel(catalog, strings, () => Task.CompletedTask);
        var failed = false;
        PropertyChangedEventArgs? aborted = null;
        escaped.PropertyChanged += (_, args) =>
        {
            if (args.PropertyName != nameof(TrashGroupsViewModel.MessageText)
                || escaped.MessageKey is null || failed) return;
            failed = true;
            aborted = args;
            throw new InvalidOperationException("subscriber failed");
        };
        // Registered after the failing subscriber. It counts only the very
        // notification the previous subscriber aborted, so the longer dispatch
        // cannot be mistaken for a later notification.
        var deliveredSame = false;
        escaped.PropertyChanged += (_, args) => deliveredSame |= ReferenceEquals(args, aborted);
        // Interface-typed subscribers (the XAML binding engine) have to reach
        // the same event, not the one inherited from ObservableObject.
        var seenThroughInterface = 0;
        ((INotifyPropertyChanged)escaped).PropertyChanged += (_, _) => seenThroughInterface++;
        Check(await escaped.RestoreCollectionAsync(collection),
            "A notification failure turned a committed restore into an operation failure.");
        Check(failed, "The failing subscriber was never reached.");
        Check(deliveredSame, "A failing subscriber hid the notification from the subscribers after it.");
        Check(seenThroughInterface > 0,
            "A subscriber bound through INotifyPropertyChanged received no notification.");
        Check(escaped.MessageKey == "restored", "A notification failure discarded the success message.");
        Check(!escaped.IsBusy, "A notification failure left the panel busy.");
        Check(Volatile.Read(ref escapedReads) == 1,
            "A notification failure skipped the refresh the committed operation held back.");
        await escaped.RefreshAsync();
        Check(Volatile.Read(ref escapedReads) == 2,
            "The queue stayed suspended after a notification failure.");
        // Nor may it resurface at close: a faulted operation task would be
        // rethrown by DisposeAsync long after the subscriber failed.
        Exception? disposal = null;
        try { await escaped.DisposeAsync(); }
        catch (Exception error) { disposal = error; }
        Check(disposal is null, $"A notification failure faulted disposal: {disposal?.Message}");

        // A burst of notifications during an operation becomes one read.
        var gate = new TaskCompletionSource<CollectionMetadata>(TaskCreationOptions.RunContinuationsAsynchronously);
        var reads = 0;
        catalog = new FakeTrashCatalog { RestoreCollection = _ => gate.Task };
        catalog.Collections = _ =>
        {
            Interlocked.Increment(ref reads);
            return Task.FromResult(Array.Empty<CollectionMetadata>());
        };
        await using (var model = new TrashGroupsViewModel(catalog, strings, () => Task.CompletedTask))
        {
            var operation = model.RestoreCollectionAsync(collection);
            var first = model.RefreshAsync();
            var second = model.RefreshAsync();
            Check(first.IsCompleted && second.IsCompleted,
                "A refresh waited for a pass the running operation held back.");
            gate.SetResult(collection);
            Check(await operation, "The gated restore did not commit.");
            Check(Volatile.Read(ref reads) == 1,
                "Notifications during one operation produced more than one catalog read.");
            Check(model.MessageKey == "restored", "The gated restore did not announce success.");
        }

        // Closing during an operation drops the queued read and reports nothing.
        var closingGate = new TaskCompletionSource<CollectionMetadata>(TaskCreationOptions.RunContinuationsAsynchronously);
        var closingReads = 0;
        catalog = new FakeTrashCatalog { RestoreCollection = _ => closingGate.Task };
        catalog.Collections = _ =>
        {
            Interlocked.Increment(ref closingReads);
            return Task.FromResult(Array.Empty<CollectionMetadata>());
        };
        var stopping = new TrashGroupsViewModel(catalog, strings, () => Task.CompletedTask);
        var stoppedOperation = stopping.RestoreCollectionAsync(collection);
        _ = stopping.RefreshAsync();
        stopping.RequestStop();
        closingGate.SetResult(collection);
        await stoppedOperation;
        await stopping.DisposeAsync();
        Check(Volatile.Read(ref closingReads) == 0, "A stopped panel read the catalog after shutdown.");
        Check(stopping.MessageKey is null && stopping.ErrorKey is null, "A stopped panel published a banner.");
        await stopping.RefreshAsync();
        Check(Volatile.Read(ref closingReads) == 0, "A stopped panel accepted a refresh after disposal.");
        Console.WriteLine("PASS: trash operation/refresh combinations, stale errors and close discard");
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

    /// <summary>
    /// The status line picks one of three brushes from this window's resource
    /// dictionary. ResourceDictionary's C#/WinRT projection exposes only the
    /// IDictionary members (indexer, ContainsKey, TryGetValue); the ABI names
    /// Lookup/HasKey are not callable from C# (CS1061), which is invisible to
    /// this test project because it never compiles the WinUI sources.
    /// </summary>
    private static void SyncStatusBrushes(string root)
    {
        var xaml = File.ReadAllText(Path.Combine(root, "apps/windows/app/MainWindow.xaml"));
        var code = File.ReadAllText(Path.Combine(root, "apps/windows/app/MainWindow.xaml.cs"));
        foreach (var key in new[] { "SyncIdleBrush", "SyncAttentionBrush", "SyncFailureBrush" })
        {
            Check(xaml.Contains($"x:Key=\"{key}\"", StringComparison.Ordinal),
                $"MainWindow.xaml no longer declares the {key} resource.");
            Check(code.Contains($"\"{key}\"", StringComparison.Ordinal),
                $"MainWindow.xaml.cs no longer reads the {key} resource.");
        }
        Check(!code.Contains(".HasKey(", StringComparison.Ordinal)
              && !code.Contains(".Lookup(", StringComparison.Ordinal),
            "ResourceDictionary has no C# HasKey/Lookup; read the indexer instead.");
        Console.WriteLine("PASS: sync status brushes stay declared in XAML and read through the indexer");
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
        // The shared key promises a background loop that survives the window,
        // which Windows does not have; it must stay replaced by the lifecycle hint.
        Check(!chinese.ContainsKey("sync_background_hint"),
            "Windows must not claim background sync; use sync_lifecycle_hint.");
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
        // Windows-only copy for the desktop settings page. Mobile has no drag
        // hand-off, a GIF preset, or a device identity panel, so these keys have no
        // Android source; they must not be invented there either.
        var windowsOnly = new HashSet<string>(StringComparer.Ordinal)
        {
            "output_copy_group", "output_drag_group", "output_still", "output_animated",
            "export_gif", "export_gif_hint", "settings_library_id", "settings_device_id",
            "settings_license_text", "about_copy", "about_copy_failed"
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
                // Windows has no tray process: MainWindow.OnClosing stops the sync
                // loop, so Android's "runs in the background" copy would promise a
                // behaviour this platform does not have. The key stays Windows-only
                // and must name both halves of the lifecycle: syncing while the app
                // runs, stopped once it is paused or closed.
                if (key == "sync_lifecycle_hint")
                {
                    Check(!android.ContainsKey(key), "The Windows-only sync lifecycle hint leaked into Android.");
                    Check(value.Contains(locale == "values" ? "运行" : "running", StringComparison.OrdinalIgnoreCase)
                          && value.Contains(locale == "values" ? "关闭" : "close", StringComparison.OrdinalIgnoreCase),
                        $"Sync lifecycle hint does not describe the Windows lifecycle: {key}/{locale}");
                    continue;
                }
                if (windowsOnly.Contains(key))
                {
                    Check(!android.ContainsKey(key), $"Windows-only resource leaked into Android: {key}");
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
                await TrashCatalogRestartAsync();
                await TrashCatalogRelationsAsync(reopened, root);
                await LargeExportAsync(reopened, root);
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
            var listed = trash.Stickers.Select(item => item.Id).ToHashSet();
            Check(listed.Contains(purple.Sticker.Id) && listed.Contains(cyan.Sticker.Id),
                "The trash listing did not contain the deleted stickers.");
            Check(trash.Next is null, "Two deleted stickers returned another cursor.");
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
        // A library with no sync binding keeps every row at Revision::LOCAL, so the
        // conflict this test can reach is a revision the tombstone does not carry,
        // such as the one a peer reports after syncing ahead.
        var observed = deletedAgain.Lifecycle.Revision;
        var conflict = await TrashOperations.PurgeAsync(session,
            [new TrashEntry(purple.Sticker.Id, deletedAgain.Lifecycle.Generation, observed + 1)], Key, token);
        Check(conflict is { Succeeded: 0, Failed: 1, ErrorKey: "Conflict" }, "A purge with a conflicting revision was accepted.");
        // One below the local revision falls outside the persisted range, so the
        // bridge rejects it as invalid input before any revision comparison runs.
        var malformed = await TrashOperations.PurgeAsync(session,
            [new TrashEntry(purple.Sticker.Id, deletedAgain.Lifecycle.Generation, observed - 1)], Key, token);
        Check(malformed is { Succeeded: 0, Failed: 1, ErrorKey: "InvalidInput" },
            "An out-of-range purge revision was not rejected as invalid input.");
        using (var trash = await session.QueryAsync(new(LibraryScope.Trash), "", StickerSort.Recent, null, token))
            Check(trash.Stickers.Any(item => item.Id == purple.Sticker.Id), "A rejected purge dropped the sticker from the trash.");
        var staleDetail = await session.DetailAsync(purple.Sticker.Id, token);
        Check(File.Exists(staleDetail.OriginalPath), "A rejected purge deleted the original file.");
        Check(staleDetail.Sticker.Lifecycle.PurgedAt is null, "A rejected purge marked the sticker as purged.");

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
            Check(!trash.Stickers.Any(item => item.Id == purple.Sticker.Id || item.Id == cyan.Sticker.Id),
                "The emptied trash still listed a purged sticker.");
        using (var active = await session.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, token))
            Check(!active.Stickers.Any(item => item.Id == purple.Sticker.Id || item.Id == cyan.Sticker.Id),
                "Trash operations changed the active listing of the stickers they touched.");

        // Only the objects this test created are cleaned up here; the restart and
        // relation tests build their own fixtures instead of reading these.
        var deletedCollection = await session.DeleteCollectionAsync(
            new(collection.Id, collection.Lifecycle.Generation), token);
        var deletedTag = await session.DeleteTagAsync(new(tag.Id, tag.Lifecycle.Generation), token);
        await session.PurgeCollectionAsync(
            new(deletedCollection.Id, deletedCollection.Lifecycle.Generation), deletedCollection.Lifecycle.Revision, token);
        await session.PurgeTagAsync(
            new(deletedTag.Id, deletedTag.Lifecycle.Generation), deletedTag.Lifecycle.Revision, token);
        Console.WriteLine("PASS: real trash listing, search, restore, purge, idempotence and failure consistency");
    }

    // The trash entry uses the revision observed when the sticker was listed, so
    // a concurrent change fails for that entry only.
    private static TrashEntry Entry(StickerMetadata item) => new(item.Id, item.Lifecycle.Generation, item.Lifecycle.Revision);

    // This test owns its library: it creates the deleted metadata, closes, and
    // reopens to prove the core persisted it. Nothing here reads fixtures left by
    // another test, so it can run alone or in any order.
    private static async Task TrashCatalogRestartAsync()
    {
        var token = CancellationToken.None;
        var parent = Path.TrimEndingDirectorySeparator(Path.GetFullPath(Path.GetTempPath()));
        var root = Path.Combine(parent, "MemeDock-Windows-restart-" + Guid.NewGuid().ToString("N"));
        try
        {
            string collectionId;
            string tagId;
            await using (var created = await LibrarySession.OpenAsync(root))
            {
                var collection = await created.CreateCollectionAsync("重启合集", token);
                var tag = await created.CreateTagAsync("重启标签", token);
                collectionId = collection.Id;
                tagId = tag.Id;
                await created.DeleteCollectionAsync(new(collection.Id, collection.Lifecycle.Generation), token);
                await created.DeleteTagAsync(new(tag.Id, tag.Lifecycle.Generation), token);
                Check(!(await created.CollectionsAsync(token)).Any(item => item.Id == collectionId)
                      && !(await created.TagsAsync(token)).Any(item => item.Id == tagId),
                    "Deleting a catalog left it in the active catalog list.");
            }

            await using var session = await LibrarySession.OpenAsync(root);
            var deletedCollections = await session.CollectionsAsync(token, deleted: true);
            var deletedTags = await session.TagsAsync(token, deleted: true);
            var trashedCollection = deletedCollections.SingleOrDefault(item => item.Id == collectionId);
            var trashedTag = deletedTags.SingleOrDefault(item => item.Id == tagId);
            Check(trashedCollection is not null, "A deleted collection did not survive a library reopen.");
            Check(trashedTag is not null, "A deleted tag did not survive a library reopen.");
            Check(!(await session.CollectionsAsync(token)).Any(item => item.Id == trashedCollection!.Id),
                "A deleted collection leaked into the active catalog after reopen.");
            Check(!(await session.TagsAsync(token)).Any(item => item.Id == trashedTag!.Id),
                "A deleted tag leaked into the active catalog after reopen.");

            var restoredCollection = await session.RestoreCollectionAsync(
                new(trashedCollection!.Id, trashedCollection.Lifecycle.Generation),
                trashedCollection.Lifecycle.Revision, token);
            Check(restoredCollection.Id == trashedCollection.Id
                  && restoredCollection.Lifecycle.Generation > trashedCollection.Lifecycle.Generation,
                "Restoring a collection did not advance its generation.");
            Check((await session.CollectionsAsync(token)).Any(item => item.Id == restoredCollection.Id),
                "A restored collection did not return to the active catalog.");
            Check(!(await session.CollectionsAsync(token, deleted: true)).Any(item => item.Id == restoredCollection.Id),
                "A restored collection stayed in the deleted catalog.");

            Check(await ExpectFailure(() => session.PurgeCollectionAsync(
                    new(restoredCollection.Id, restoredCollection.Lifecycle.Generation),
                    restoredCollection.Lifecycle.Revision, token)) == ErrorCode.Conflict,
                "An active collection was permanently deleted instead of rejected.");
            Check(await ExpectFailure(() => session.PurgeCollectionAsync(
                    new(trashedCollection.Id, trashedCollection.Lifecycle.Generation),
                    trashedCollection.Lifecycle.Revision, token)) == ErrorCode.Conflict,
                "A purge with the pre-restore generation was accepted.");
            Check((await session.CollectionsAsync(token)).Any(item => item.Id == restoredCollection.Id),
                "A rejected collection purge removed the active entry.");

            var deletedAgain = await session.DeleteCollectionAsync(
                new(restoredCollection.Id, restoredCollection.Lifecycle.Generation), token);
            Check(await ExpectFailure(() => session.PurgeCollectionAsync(
                    new(deletedAgain.Id, deletedAgain.Lifecycle.Generation),
                    deletedAgain.Lifecycle.Revision + 1, token)) == ErrorCode.Conflict,
                "A purge with a conflicting revision was accepted.");
            await session.PurgeCollectionAsync(
                new(deletedAgain.Id, deletedAgain.Lifecycle.Generation),
                deletedAgain.Lifecycle.Revision, token);
            Check(!(await session.CollectionsAsync(token, deleted: true)).Any(item => item.Id == deletedAgain.Id),
                "A purged collection stayed in the deleted catalog.");

            var restoredTag = await session.RestoreTagAsync(
                new(trashedTag!.Id, trashedTag.Lifecycle.Generation),
                trashedTag.Lifecycle.Revision, token);
            Check(restoredTag.Id == trashedTag.Id
                  && restoredTag.Lifecycle.Generation > trashedTag.Lifecycle.Generation,
                "Restoring a tag did not advance its generation.");
            Check((await session.TagsAsync(token)).Any(item => item.Id == restoredTag.Id),
                "A restored tag did not return to the active catalog.");
            Check(await ExpectFailure(() => session.PurgeTagAsync(
                    new(restoredTag.Id, restoredTag.Lifecycle.Generation),
                    restoredTag.Lifecycle.Revision, token)) == ErrorCode.Conflict,
                "An active tag was permanently deleted instead of rejected.");

            var deletedTagAgain = await session.DeleteTagAsync(
                new(restoredTag.Id, restoredTag.Lifecycle.Generation), token);
            await session.PurgeTagAsync(
                new(deletedTagAgain.Id, deletedTagAgain.Lifecycle.Generation),
                deletedTagAgain.Lifecycle.Revision, token);
            Check(!(await session.TagsAsync(token, deleted: true)).Any(item => item.Id == deletedTagAgain.Id),
                "A purged tag stayed in the deleted catalog.");
            // Repeating a purge on a tombstone is not a failure.
            await session.PurgeTagAsync(
                new(deletedTagAgain.Id, deletedTagAgain.Lifecycle.Generation),
                deletedTagAgain.Lifecycle.Revision, token);

            const string missing = "0190f3a0-0000-7000-8000-000000000000";
            Check(await ExpectFailure(() => session.PurgeTagAsync(new(missing, 1), 1, token)) == ErrorCode.NotFound,
                "Purging an unknown tag did not report not-found.");
            Check(await ExpectFailure(() => session.RestoreCollectionAsync(new(missing, 1), 1, token)) == ErrorCode.NotFound,
                "Restoring an unknown collection did not report not-found.");
            Console.WriteLine("PASS: deleted collections/tags survive reopen, restore, conflicts, purge and missing entities");
        }
        finally
        {
            if (Directory.Exists(root) && string.Equals(Path.GetDirectoryName(root), parent, StringComparison.OrdinalIgnoreCase)
                && Path.GetFileName(root).StartsWith("MemeDock-Windows-restart-", StringComparison.Ordinal))
                Directory.Delete(root, recursive: true);
        }
    }

    private static async Task TrashCatalogRelationsAsync(LibrarySession session, string root)
    {
        var token = CancellationToken.None;
        var path = Path.Combine(root, "catalog-relations.png");
        await File.WriteAllBytesAsync(path, Png(12, 34, 56));
        var collection = await session.CreateCollectionAsync("关系合集", token);
        var tag = await session.CreateTagAsync("关系标签", token);
        var imported = await session.ImportAsync(path, collection.Id, token);
        var detail = await session.DetailAsync(imported.Sticker.Id, token);
        await session.OrganizeAsync(
            new(imported.Sticker.Id, detail.Sticker.Lifecycle.Generation), true,
            new(collection.Id, collection.Lifecycle.Generation),
            [new(tag.Id, tag.Lifecycle.Generation)], token);
        detail = await session.DetailAsync(imported.Sticker.Id, token);
        Check(detail.Collection?.Id == collection.Id && detail.Tags.Any(item => item.Id == tag.Id),
            "The catalog relation test did not establish its fixture.");

        var deletedCollection = await session.DeleteCollectionAsync(
            new(collection.Id, collection.Lifecycle.Generation), token);
        var deletedTag = await session.DeleteTagAsync(
            new(tag.Id, tag.Lifecycle.Generation), token);
        Check(!(await session.CollectionsAsync(token)).Any(item => item.Id == collection.Id)
              && !(await session.TagsAsync(token)).Any(item => item.Id == tag.Id),
            "Deleting a catalog left it in the active catalog list.");
        await session.PurgeCollectionAsync(
            new(deletedCollection.Id, deletedCollection.Lifecycle.Generation),
            deletedCollection.Lifecycle.Revision, token);
        await session.PurgeTagAsync(
            new(deletedTag.Id, deletedTag.Lifecycle.Generation),
            deletedTag.Lifecycle.Revision, token);

        var after = await session.DetailAsync(imported.Sticker.Id, token);
        Check(after.Sticker.Lifecycle.DeletedAt is null && after.Sticker.Lifecycle.PurgedAt is null,
            "Purging a collection or tag deleted its sticker.");
        Check(after.OriginalPath is not null && File.Exists(after.OriginalPath),
            "Purging a collection or tag deleted the sticker original.");
        Check(after.Collection is null && after.Tags.Length == 0,
            "Purging a collection or tag left an effective relation on the sticker.");
        Console.WriteLine("PASS: purging collections/tags keeps stickers and originals and removes effective relations");
    }

    private static async Task OutputContractsAsync(LibrarySession session, string id, string source)
    {
        var token = CancellationToken.None;
        var detail = await session.DetailAsync(id, token);
        var target = new StickerOutputTarget(id, detail.Sticker.Lifecycle.Generation);
        target.Validate(detail.Sticker);
        using var original = await session.ExportAsync(id,
            StickerOutputPolicy.Options(false, StickerOutputKind.OriginalFile, OutputDefaults.CopyDefault), token);
        var artifact = await session.PrepareHandoffAsync(original, token);
        Check((await File.ReadAllBytesAsync(artifact.Path)).SequenceEqual(await File.ReadAllBytesAsync(source)), "Original handoff changed bytes.");
        Check(artifact.RetainedUntil > DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(), "Handoff did not durably retain its file.");
        var pin = await session.ProtectClipboardAsync(original, token);
        await session.ReconcileClipboardAsync(pin);
        var failedPublication = await session.ProtectClipboardAsync(original, token);
        await session.AbortClipboardAsync(failedPublication);
        await session.ReconcileClipboardAsync(pin);
        using var png = await session.ExportAsync(id,
            StickerOutputPolicy.Options(false, StickerOutputKind.Image, OutputDefaults.CopyDefault), token);
        var converted = await session.PrepareHandoffAsync(png, token);
        Check(converted.Mime == "image/png" && !converted.Animated && File.Exists(converted.Path), "PNG export did not publish a verified static file.");
        // Dragging used to bypass preferences through StickerOutputKind.OriginalFile; the
        // default drag preset must still hand over the very same bytes without a re-encode.
        using var dragged = await session.ExportAsync(id,
            StickerOutputPolicy.Options(false, StickerOutputKind.Image, OutputDefaults.DragDefault), token);
        var draggedArtifact = await session.PrepareHandoffAsync(dragged, token);
        Check(!draggedArtifact.Animated
              && (await File.ReadAllBytesAsync(draggedArtifact.Path)).SequenceEqual(await File.ReadAllBytesAsync(source)),
            "Default drag changed the original still bytes.");
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
            StickerOutputPolicy.Options(moving.Asset.Animated, StickerOutputKind.Image, OutputDefaults.CopyDefault), token);
        var movingArtifact = await session.PrepareHandoffAsync(movingOriginal, token);
        Check(movingArtifact.Animated && (await File.ReadAllBytesAsync(movingArtifact.Path)).SequenceEqual(gif),
            "Default animated copy changed the original GIF or lost animation.");
        using var draggedMoving = await session.ExportAsync(imported.Sticker.Id,
            StickerOutputPolicy.Options(true, StickerOutputKind.Image, OutputDefaults.DragDefault), token);
        var draggedMovingArtifact = await session.PrepareHandoffAsync(draggedMoving, token);
        Check(draggedMovingArtifact.Animated
              && (await File.ReadAllBytesAsync(draggedMovingArtifact.Path)).SequenceEqual(gif),
            "Default animated drag changed the original GIF or lost animation.");
        using var firstFrame = await session.ExportAsync(imported.Sticker.Id,
            StickerOutputPolicy.Options(true, StickerOutputKind.FirstFrame, OutputDefaults.CopyDefault), token);
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

    /// <summary>
    /// The still default must stay a transparent PNG capped at 1024px through the real
    /// encoder, not only in the preset table.
    /// </summary>
    private static async Task LargeExportAsync(LibrarySession session, string root)
    {
        var token = CancellationToken.None;
        var source = Path.Combine(root, "large-output.png");
        await File.WriteAllBytesAsync(source, Png(1600, 900, 10, 20, 30, 128));
        try
        {
            var imported = await session.ImportAsync(source, null, token);
            using var exported = await session.ExportAsync(imported.Sticker.Id,
                StickerOutputPolicy.Options(false, StickerOutputKind.Image, OutputDefaults.CopyDefault), token);
            var artifact = await session.PrepareHandoffAsync(exported, token);
            var header = PngHeader(await File.ReadAllBytesAsync(artifact.Path));
            Check(artifact.Mime == "image/png" && !artifact.Animated, "The large still default was not a static PNG.");
            Check(header is { } size && Math.Max(size.Width, size.Height) == 1024 && size.ColorType == 6,
                $"The still default was not a transparent PNG capped at 1024px (got {header?.Width}x{header?.Height}, color type {header?.ColorType}).");
            await session.DeleteStickerAsync(new(imported.Sticker.Id, imported.Sticker.Lifecycle.Generation), token);
            Console.WriteLine("PASS: still default keeps an alpha PNG capped at 1024px through the real encoder");
        }
        finally
        {
            File.Delete(source);
        }
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

    private static byte[] Png(byte red, byte green, byte blue) => Png(1, 1, red, green, blue, 255);

    // A valid RGBA PNG of any size, using the same fixture construction as NativeContracts.
    private static byte[] Png(int width, int height, byte red, byte green, byte blue, byte alpha)
    {
        using var output = new MemoryStream();
        output.Write(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 });
        var header = new byte[13];
        BinaryPrimitives.WriteUInt32BigEndian(header.AsSpan(0, 4), (uint)width);
        BinaryPrimitives.WriteUInt32BigEndian(header.AsSpan(4, 4), (uint)height);
        header[8] = 8;
        header[9] = 6;
        Chunk(output, "IHDR"u8, header);
        var row = new byte[1 + width * 4];
        for (var x = 0; x < width; x++)
        {
            row[1 + x * 4] = red;
            row[2 + x * 4] = green;
            row[3 + x * 4] = blue;
            row[4 + x * 4] = alpha;
        }
        using var compressed = new MemoryStream();
        using (var zlib = new ZLibStream(compressed, CompressionLevel.Fastest, leaveOpen: true))
        {
            for (var y = 0; y < height; y++) zlib.Write(row);
        }
        Chunk(output, "IDAT"u8, compressed.ToArray());
        Chunk(output, "IEND"u8, []);
        return output.ToArray();
    }

    private static (int Width, int Height, byte ColorType)? PngHeader(byte[] bytes)
    {
        if (bytes.Length < 26
            || !bytes.AsSpan(0, 8).SequenceEqual(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 })
            || !bytes.AsSpan(12, 4).SequenceEqual("IHDR"u8))
            return null;
        return ((int)BinaryPrimitives.ReadUInt32BigEndian(bytes.AsSpan(16, 4)),
            (int)BinaryPrimitives.ReadUInt32BigEndian(bytes.AsSpan(20, 4)),
            bytes[25]);
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
            Directory.CreateDirectory(root);
            var path = Path.Combine(root, "preferences.json");
            // The appearance selector picks its row from the enum value, so the enum
            // order is part of the settings contract, not an incidental detail.
            Check((int)AppearanceMode.System == 0 && (int)AppearanceMode.Light == 1 && (int)AppearanceMode.Dark == 2,
                "The appearance selector order no longer matches the AppearanceMode values.");
            var preferences = new AppPreferences(root);
            await preferences.LoadAsync();
            Check(preferences.Language == LanguageMode.System && preferences.Appearance == AppearanceMode.System
                  && preferences.CopyOutput == OutputDefaults.CopyDefault && preferences.DragOutput == OutputDefaults.DragDefault,
                "A fresh install did not start from the approved defaults.");

            await preferences.SaveLanguageAsync(LanguageMode.Chinese);
            await preferences.SaveAppearanceAsync(AppearanceMode.Dark);
            await preferences.SaveOutputPresetAsync(OutputAction.Copy, animated: false, ExportPreset.SmallJpeg);
            await preferences.SaveOutputPresetAsync(OutputAction.Drag, animated: true, ExportPreset.CompatibleGif);
            var reopened = new AppPreferences(root);
            await reopened.LoadAsync();
            Check(reopened.Language == LanguageMode.Chinese && reopened.Appearance == AppearanceMode.Dark,
                "Language or appearance did not survive a process restart.");
            Check(reopened.CopyOutput == new OutputDefaults(ExportPreset.SmallJpeg, ExportPreset.Original)
                  && reopened.DragOutput == new OutputDefaults(ExportPreset.Original, ExportPreset.CompatibleGif),
                "An output preset did not survive a process restart.");

            // Enum names keep the file readable and immune to ordinal changes.
            var json = await File.ReadAllTextAsync(path);
            Check(json.Contains("\"Dark\"", StringComparison.Ordinal) && json.Contains("\"SmallJpeg\"", StringComparison.Ordinal),
                "Preferences were written as ordinals instead of enum names.");

            // Writes to different fields must compose, not overwrite each other.
            await Task.WhenAll(
                preferences.SaveLanguageAsync(LanguageMode.English),
                preferences.SaveAppearanceAsync(AppearanceMode.Light),
                preferences.SaveOutputPresetAsync(OutputAction.Copy, animated: false, ExportPreset.SmallJpeg));
            var merged = new AppPreferences(root);
            await merged.LoadAsync();
            Check(merged.Language == LanguageMode.English && merged.Appearance == AppearanceMode.Light
                  && merged.CopyOutput == new OutputDefaults(ExportPreset.SmallJpeg, ExportPreset.Original)
                  && merged.DragOutput == new OutputDefaults(ExportPreset.Original, ExportPreset.CompatibleGif),
                "Concurrent preference writes lost a field.");

            // A file written before this batch only has Language; the rest defaults.
            await File.WriteAllTextAsync(path, "{\"Language\":\"Chinese\"}");
            var legacy = new AppPreferences(root);
            await legacy.LoadAsync();
            Check(legacy.Language == LanguageMode.Chinese && legacy.Appearance == AppearanceMode.System
                  && legacy.CopyOutput == OutputDefaults.CopyDefault && legacy.DragOutput == OutputDefaults.DragDefault,
                "A language-only file did not migrate to the other defaults.");

            // Unknown enums, presets that do not fit their field and wrong value
            // kinds all fall back per field instead of failing the whole load.
            await File.WriteAllTextAsync(path,
                "{\"Language\":\"Klingon\",\"Appearance\":42,\"Output\":{\"Copy\":{\"Still\":\"CompatibleGif\",\"Animated\":\"SmallJpeg\"},\"Drag\":5}}");
            var sanitized = new AppPreferences(root);
            await sanitized.LoadAsync();
            Check(sanitized.Language == LanguageMode.System && sanitized.Appearance == AppearanceMode.System
                  && sanitized.CopyOutput == OutputDefaults.CopyDefault && sanitized.DragOutput == OutputDefaults.DragDefault,
                "Invalid persisted values did not fall back to their own defaults.");

            await File.WriteAllTextAsync(path, "{\"Appearance\":\"Dark\",\"Output\":{\"Copy\":{\"Animated\":\"Original\"}}}");
            var partial = new AppPreferences(root);
            await partial.LoadAsync();
            Check(partial.Appearance == AppearanceMode.Dark && partial.CopyOutput == OutputDefaults.CopyDefault,
                "A partial file changed fields it did not mention.");

            await File.WriteAllTextAsync(path, "broken json");
            var damaged = new AppPreferences(root);
            await damaged.LoadAsync();
            Check(damaged.Language == LanguageMode.System && damaged.CopyOutput == OutputDefaults.CopyDefault,
                "Damaged preferences prevented defaults from applying.");

            // A failed write must leave both the file and the instance on the old value.
            await File.WriteAllTextAsync(path, "{\"Appearance\":\"Dark\"}");
            var stable = new AppPreferences(root);
            await stable.LoadAsync();
            var temporary = path + ".tmp";
            Directory.CreateDirectory(temporary);
            try
            {
                await stable.SaveAppearanceAsync(AppearanceMode.Light);
                throw new InvalidOperationException("A directory blocking the temporary file did not fail the write.");
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException or NotSupportedException) { }
            finally
            {
                if (Directory.Exists(temporary)) Directory.Delete(temporary);
            }
            Check(stable.Appearance == AppearanceMode.Dark, "A failed write changed the in-memory appearance.");
            var intact = new AppPreferences(root);
            await intact.LoadAsync();
            Check(intact.Appearance == AppearanceMode.Dark, "A failed write damaged the preference file.");

            Console.WriteLine("PASS: preference defaults, restart round-trips, fault-tolerant migration, concurrent writes and failed writes");
        }
        finally
        {
            if (Directory.Exists(root) && string.Equals(Path.GetDirectoryName(root), Path.TrimEndingDirectorySeparator(parent), StringComparison.OrdinalIgnoreCase)
                && Path.GetFileName(root).StartsWith("MemeDock-Windows-preferences-", StringComparison.Ordinal))
                Directory.Delete(root, recursive: true);
        }
    }

    /// <summary>
    /// One table for every sync status the windows render: the phase, the text
    /// key and the count are the only inputs the status-bar entry and the
    /// settings page use, so they cannot disagree.
    /// </summary>
    private static void SyncStatusMapping()
    {
        static SyncState State(SyncStatus status, bool loaded = true, bool running = false,
            bool hasCredential = true, Exception? error = null)
            => new(status, loaded, running, hasCredential, Changing: false, error);

        // Idle, nothing queued, and a round has completed.
        var ready = State(SyncSnapshot(lastSuccess: 1_700_000_000_000));
        Check(SyncPresentation.Phase(ready) == SyncPhase.UpToDate
              && SyncPresentation.StatusKey(ready) == "sync_ready"
              && SyncPresentation.HintKey(ready) is null
              && SyncPresentation.IsVisible(ready),
            "An idle, successfully synced binding did not report the ready state.");

        // Connected, nothing queued, but no round has finished: not "up to date".
        var unsynced = State(SyncSnapshot());
        Check(SyncPresentation.Phase(unsynced) == SyncPhase.Unsynced
              && SyncPresentation.StatusKey(unsynced) == "sync_never"
              && SyncPresentation.StatusKey(unsynced) != "sync_ready",
            "A binding without a completed round claimed to be up to date.");

        // Local changes are still queued.
        var pending = State(SyncSnapshot(pending: 3, lastSuccess: 1_700_000_000_000));
        Check(SyncPresentation.Phase(pending) == SyncPhase.Pending
              && SyncPresentation.StatusKey(pending) == "sync_pending"
              && SyncPresentation.HintKey(pending) is null,
            "Queued changes were reported as up to date.");

        // Changes the server rejects have their own state and their own hint.
        var blocked = State(SyncSnapshot(pending: 4, blocked: 2));
        Check(SyncPresentation.Phase(blocked) == SyncPhase.Blocked
              && SyncPresentation.StatusKey(blocked) == "sync_blocked"
              && SyncPresentation.HintKey(blocked) == "sync_blocked_hint",
            "Blocked changes were not reported as blocked.");

        // A user pause is never a network error, and a retryable error recorded
        // before pausing is history rather than a live failure.
        var paused = State(SyncSnapshot(paused: true, pending: 6, error: "network"));
        Check(SyncPresentation.Phase(paused) == SyncPhase.Paused
              && SyncPresentation.StatusKey(paused) == "sync_paused"
              && SyncPresentation.ErrorKey(paused) is null,
            "A user pause was reported as a network failure.");

        // Pairs that can co-occur, pinned here so the precedence in Phase stays a
        // contract instead of something a future edit can reorder silently.
        // Pausing outranks blocked changes: the pause is why nothing moves, and
        // the settings page still reports the blocked count and its hint.
        var pausedWithBlocked = State(SyncSnapshot(paused: true, pending: 4, blocked: 2));
        Check(SyncPresentation.Phase(pausedWithBlocked) == SyncPhase.Paused
              && SyncPresentation.StatusKey(pausedWithBlocked) == "sync_paused"
              && SyncPresentation.HintKey(pausedWithBlocked) is null,
            "A pause with blocked changes reported something other than the pause.");

        // A rejected token is actionable whatever the pause says, so it wins over
        // the pause instead of waiting for a resume that cannot succeed.
        var pausedWithExpiredToken = State(SyncSnapshot(paused: true, error: "unauthorized"));
        Check(SyncPresentation.Phase(pausedWithExpiredToken) == SyncPhase.Authorization
              && SyncPresentation.StatusKey(pausedWithExpiredToken) == "sync_reauthorize"
              && SyncPresentation.HintKey(pausedWithExpiredToken) == "sync_failure_unauthorized",
            "A paused binding with a rejected token did not ask for a new token.");

        // Any other terminal status error is history while paused and only
        // becomes a failure once syncing resumes.
        var pausedWithCorruptData = State(SyncSnapshot(paused: true, error: "corrupt_data"));
        Check(SyncPresentation.Phase(pausedWithCorruptData) == SyncPhase.Paused
              && SyncPresentation.ErrorKey(pausedWithCorruptData) is null,
            "A paused binding with a terminal status error did not defer to the pause.");
        var resumedCorruptData = State(SyncSnapshot(error: "corrupt_data"));
        Check(SyncPresentation.Phase(resumedCorruptData) == SyncPhase.Failed
              && !SyncPresentation.CanSynchronize(resumedCorruptData),
            "Resuming did not surface the terminal status error recorded while paused.");

        // A request in flight only claims that a request was sent, even when the
        // failure is already known; the failure is shown once the request ends.
        var runningWithExpiredToken = State(SyncSnapshot(error: "unauthorized"), running: true);
        Check(SyncPresentation.Phase(runningWithExpiredToken) == SyncPhase.Running
              && SyncPresentation.StatusKey(runningWithExpiredToken) == "sync_running",
            "A running request was reported as something other than running.");
        var runningWithCredentialFailure = State(SyncSnapshot(), running: true,
            error: new SyncCredentialException("keychain"));
        Check(SyncPresentation.Phase(runningWithCredentialFailure) == SyncPhase.Running
              && SyncPresentation.StatusKey(runningWithCredentialFailure) == "sync_running",
            "A running request reported a credential failure before it finished.");
        var settledExpiredToken = State(SyncSnapshot(error: "unauthorized"));
        Check(SyncPresentation.StatusKey(settledExpiredToken) == "sync_reauthorize",
            "A finished request did not surface the rejected token.");

        // Changes can be queued before the first round ever completes: the queue
        // is the stronger fact, so the entry cannot claim it never synced.
        var pendingBeforeFirstRound = State(SyncSnapshot(pending: 1));
        Check(SyncPresentation.Phase(pendingBeforeFirstRound) == SyncPhase.Pending
              && SyncPresentation.StatusKey(pendingBeforeFirstRound) == "sync_pending",
            "Queued changes on a never-synced binding were not reported as pending.");

        // Blocked changes outlive a retryable error, so they own the status; the
        // retry detail stays available to the settings page's error line.
        var blockedWithNetwork = State(SyncSnapshot(pending: 3, blocked: 1, error: "network"));
        Check(SyncPresentation.Phase(blockedWithNetwork) == SyncPhase.Blocked
              && SyncPresentation.StatusKey(blockedWithNetwork) == "sync_blocked"
              && SyncPresentation.HintKey(blockedWithNetwork) == "sync_blocked_hint"
              && SyncPresentation.ErrorKey(blockedWithNetwork) == "sync_failure_network",
            "Blocked changes were replaced by the retryable error.");

        // A failure carried by the snapshot is not reported while the user has
        // paused: the pause is the reason nothing is moving.
        var pausedWithBridgeFailure = State(SyncSnapshot(paused: true, error: "network"),
            error: new BridgeException.Failure(ErrorCode.Network, "offline"));
        Check(SyncPresentation.Phase(pausedWithBridgeFailure) == SyncPhase.Paused
              && SyncPresentation.HintKey(pausedWithBridgeFailure) is null,
            "A paused binding reported a network failure.");

        // A request that was sent is not a finished round.
        var running = State(SyncSnapshot(lastSuccess: 1_700_000_000_000), running: true);
        Check(SyncPresentation.Phase(running) == SyncPhase.Running
              && SyncPresentation.StatusKey(running) == "sync_running",
            "A sent sync request did not report that it is running.");

        // A failed round stays visible, with the retry explanation as its hint.
        var failed = State(SyncSnapshot(pending: 2, error: "network"));
        Check(SyncPresentation.Phase(failed) == SyncPhase.Failed
              && SyncPresentation.StatusKey(failed) == "sync_failed"
              && SyncPresentation.HintKey(failed) == "sync_failure_network",
            "A failed round did not report the failure.");

        // A library that was never connected stays out of the status bar.
        var unconfigured = State(SyncSnapshot(origin: null, connected: false), hasCredential: false);
        Check(SyncPresentation.Phase(unconfigured) == SyncPhase.Authorization
              && SyncPresentation.StatusKey(unconfigured) == "sync_authorize"
              && !SyncPresentation.IsVisible(unconfigured),
            "An unconfigured library claimed a sync status.");

        // A stored token that the server rejected needs its own instruction.
        var expired = State(SyncSnapshot(error: "unauthorized"));
        Check(SyncPresentation.Phase(expired) == SyncPhase.Authorization
              && SyncPresentation.StatusKey(expired) == "sync_reauthorize"
              && SyncPresentation.HintKey(expired) == "sync_failure_unauthorized"
              && SyncPresentation.IsVisible(expired),
            "An expired token was not distinguished from a disconnected library.");

        // A credential store failure also asks for the token again.
        var credential = State(SyncSnapshot(), error: new SyncCredentialException("keychain"));
        Check(SyncPresentation.Phase(credential) == SyncPhase.Authorization
              && SyncPresentation.StatusKey(credential) == "sync_reauthorize"
              && SyncPresentation.ErrorKey(credential) == "sync_failure_credential",
            "A credential lookup failure did not ask for a new token.");

        // Recovery: the same binding, once the failure clears and a round lands.
        var recovered = State(SyncSnapshot(lastSuccess: 1_700_000_200_000));
        Check(SyncPresentation.Phase(recovered) == SyncPhase.UpToDate
              && SyncPresentation.ErrorKey(recovered) is null,
            "Clearing the failure did not return to the ready state.");

        // Before the native status is read nothing is claimed.
        var unavailable = State(SyncSnapshot(), loaded: false);
        Check(SyncPresentation.Phase(unavailable) == SyncPhase.Unavailable
              && !SyncPresentation.IsVisible(unavailable),
            "Sync status was shown before the native status was read.");

        // A status error this build does not know has no detail text but still
        // shows the failure state, and a terminal one stops automatic retries.
        var unknown = State(SyncSnapshot(error: "unsupported_schema"));
        Check(SyncPresentation.Phase(unknown) == SyncPhase.Failed
              && SyncPresentation.StatusKey(unknown) == "sync_failed"
              && SyncPresentation.ErrorKey(unknown) is null
              && SyncPresentation.HintKey(unknown) == "sync_failed"
              && SyncPresentation.IsTerminal(unknown)
              && !SyncPresentation.CanSynchronize(unknown),
            "An unknown terminal status error failed silently.");

        // Every key the table can return must exist in both resource files: the
        // renderer looks these up by table, so the literal-usage scan in
        // ResourceParity cannot see them.
        var root = FindRepository();
        var english = ReadResw(Path.Combine(root, "apps/windows/app/Strings/en-US/Resources.resw"));
        var chinese = ReadResw(Path.Combine(root, "apps/windows/app/Strings/zh-CN/Resources.resw"));
        string?[] keys =
        [
            SyncPresentation.StatusKey(ready), SyncPresentation.StatusKey(unsynced),
            SyncPresentation.StatusKey(pending), SyncPresentation.StatusKey(blocked),
            SyncPresentation.StatusKey(paused), SyncPresentation.StatusKey(running),
            SyncPresentation.StatusKey(failed), SyncPresentation.StatusKey(unconfigured),
            SyncPresentation.StatusKey(expired), SyncPresentation.StatusKey(credential),
            SyncPresentation.StatusKey(recovered), SyncPresentation.StatusKey(unavailable),
            SyncPresentation.StatusKey(unknown), SyncPresentation.HintKey(blocked),
            SyncPresentation.HintKey(expired), SyncPresentation.HintKey(failed),
            SyncPresentation.HintKey(pausedWithExpiredToken), SyncPresentation.HintKey(blockedWithNetwork),
            SyncPresentation.StatusKey(pausedWithBlocked), SyncPresentation.StatusKey(pendingBeforeFirstRound),
            SyncPresentation.StatusKey(runningWithCredentialFailure), SyncPresentation.ErrorKey(credential),
            SyncPresentation.ErrorKey(blockedWithNetwork), SyncPresentation.ErrorKey(pausedWithBridgeFailure),
        ];
        foreach (var key in keys.Distinct())
            Check(key is not null && english.ContainsKey(key) && chinese.ContainsKey(key),
                $"Sync status text is missing from the resources: {key}");

        Console.WriteLine("PASS: sync status mapping (ready, pending, blocked, paused, running, failed, authorization, recovery, precedence combinations)");
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
              && SyncPresentation.NeedsAuthorization(sync.State)
              && !SyncPresentation.IsVisible(sync.State),
            "A fresh library did not report the disconnected sync state.");
        Check(adapter.Subscriptions.Count == 1, "Sync status was not observed exactly once.");

        adapter.Status = SyncSnapshot(connected: true, pending: 2);
        await sync.ConnectAsync("  https://sync.example.com/  ", "mdk_first");
        Check(adapter.ConnectCalls == 1 && adapter.LastOrigin == "https://sync.example.com/"
              && adapter.LastToken == "mdk_first", "Connecting did not trim or forward the server address.");
        Check(tokens.Saved.TryGetValue(library + "|https://sync.example.com/", out var stored) && stored == "mdk_first",
            "Connecting did not persist the device token.");
        // The adapter's connect snapshot has no completed round, so a connected
        // binding must not claim it is up to date yet.
        Check(sync.State.HasCredential && sync.State.Status.Connected
              && SyncPresentation.StatusKey(sync.State) == "sync_never",
            "A successful connection claimed a status no round has proven yet.");

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
        adapter.Status = SyncSnapshot(connected: true, lastSuccess: 1_699_999_000_000);
        await sync.SetPausedAsync(false);
        Check(!sync.State.Status.Paused && SyncPresentation.StatusKey(sync.State) == "sync_ready",
            "Resuming was not reflected in the sync state.");

        adapter.Status = SyncSnapshot(connected: true, pending: 5);
        adapter.Push(adapter.Status);
        await WaitForAsync(() => sync.State.Status.Pending == 5, "native status updates");
        Check(sync.State.Status.Pending == 5
              && SyncPresentation.StatusKey(sync.State) == "sync_pending",
            "A pushed native status did not reach the state.");

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
        Check(sync.State.Status.LastSuccessMs == 1_700_000_100_000 && SyncPresentation.ErrorKey(sync.State) is null
              && SyncPresentation.StatusKey(sync.State) == "sync_ready",
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
        // The stored token is still there, so the entry asks to replace it
        // rather than to connect from scratch.
        Check(SyncPresentation.NeedsAuthorization(unauthorized)
              && SyncPresentation.IsTerminal(unauthorized)
              && !SyncPresentation.CanSynchronize(unauthorized)
              && SyncPresentation.StatusKey(unauthorized) == "sync_reauthorize"
              && SyncPresentation.ErrorKey(unauthorized) == "sync_failure_unauthorized",
            "A bridge authorization failure was not presented as a re-authorization request.");

        // Every coordinator path that hits a credential-store failure drops
        // HasCredential first, so the entry falls back to the connect prompt.
        var credentialFailure = new SyncState(
            SyncSnapshot(connected: true), Loaded: true, Running: false, HasCredential: false, Changing: false,
            Error: new SyncCredentialException("credential store unavailable"));
        Check(SyncPresentation.ErrorKey(credentialFailure) == "sync_failure_credential"
              && SyncPresentation.StatusKey(credentialFailure) == "sync_authorize"
              && !SyncPresentation.CanSynchronize(credentialFailure),
            "A credential store failure was not presented as a connect request.");

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
        await SyncWakeSignalAsync();
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

    // The wake signal is conflated: one Set releases exactly one wait, and a wake
    // raised while a sync runs must still cause one more sync afterwards. A ten
    // second retry floor proves every sync here came from a wake, and a 20 ms
    // debounce lets the test wait on counters instead of guessing at sleeps.
    private static async Task SyncWakeSignalAsync()
    {
        const string library = "wake-library";
        const string origin = "https://sync.example.com/";
        var tokens = new FakeSyncTokens();
        tokens.Saved[library + "|" + origin] = "mdk_wake";
        var adapter = new FakeSyncAdapter { Status = SyncSnapshot(connected: true) };
        var sync = new SyncCoordinator(adapter, tokens, library, TimeSpan.FromSeconds(10),
            TimeSpan.FromMilliseconds(20));
        await sync.StartAsync();
        try
        {
            await WaitForAsync(() => adapter.SynchronizeCalls == 1, "the first automatic sync");

            // A wake that arrives while the loop is waiting runs one sync, not two.
            sync.NotifyContentChanged();
            await WaitForAsync(() => adapter.SynchronizeCalls == 2, "one sync for one local change");
            await Task.Delay(250);
            Check(adapter.SynchronizeCalls == 2, "One local change started more than one sync.");

            // A burst inside one debounce window coalesces into a single wake.
            for (var i = 0; i < 5; i++) sync.NotifyContentChanged();
            await WaitForAsync(() => adapter.SynchronizeCalls == 3, "a coalesced burst of local changes");
            await Task.Delay(250);
            Check(adapter.SynchronizeCalls == 3, "A burst of local changes started more than one sync.");

            // A wake raised during a sync is kept and applied as soon as it ends:
            // the ten second floor leaves no other way to reach the next sync.
            adapter.SynchronizeGate = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
            sync.NotifyContentChanged();
            await WaitForAsync(() => adapter.SynchronizeCalls == 4, "the gated sync to start");
            sync.NotifyContentChanged();
            adapter.SynchronizeGate.SetResult();
            await WaitForAsync(() => adapter.SynchronizeCalls == 5, "a sync for the wake raised during a sync");
            await Task.Delay(250);
            Check(adapter.SynchronizeCalls == 5, "A wake raised during a sync was delivered twice.");

            // The status observer is the other wake producer: a push that leaves the
            // binding unusable must not sync, and the push that makes it usable again
            // must wake the loop exactly once.
            adapter.Push(SyncSnapshot(connected: true, paused: true));
            await WaitForAsync(() => sync.State.Status.Paused, "the paused status to reach the state");
            await Task.Delay(250);
            Check(adapter.SynchronizeCalls == 5, "A paused status push started a sync.");
            adapter.Push(SyncSnapshot(connected: true));
            await WaitForAsync(() => adapter.SynchronizeCalls == 6, "a sync after the binding became usable");
            await Task.Delay(250);
            Check(adapter.SynchronizeCalls == 6, "A push that made the binding usable started more than one sync.");
        }
        finally
        {
            adapter.SynchronizeGate?.TrySetResult();
            await sync.DisposeAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(5));
        }
        var settled = adapter.SynchronizeCalls;
        sync.NotifyContentChanged();
        await Task.Delay(250);
        Check(adapter.SynchronizeCalls == settled, "A disposed coordinator still started a sync.");
        Console.WriteLine("PASS: conflated wake signal, debounced bursts and shutdown");
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
            // The native call only flips the pause flag and returns the rest of
            // the binding unchanged, so the fake keeps its cursor, queue and
            // last successful round instead of rebuilding a fresh snapshot.
            Status = Status with { Paused = paused };
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

    private sealed class FakeTrashCatalog : ITrashCatalog
    {
        public Func<CancellationToken, Task<CollectionMetadata[]>> Collections { get; set; } =
            _ => Task.FromResult(Array.Empty<CollectionMetadata>());
        public Func<CancellationToken, Task<TagMetadata[]>> Tags { get; set; } =
            _ => Task.FromResult(Array.Empty<TagMetadata>());
        public Func<EntityReference, Task<CollectionMetadata>> RestoreCollection { get; set; } =
            _ => Task.FromException<CollectionMetadata>(new InvalidOperationException("unexpected collection restore"));
        public Func<EntityReference, Task<TagMetadata>> RestoreTag { get; set; } =
            _ => Task.FromException<TagMetadata>(new InvalidOperationException("unexpected tag restore"));
        public Func<EntityReference, Task> PurgeCollection { get; set; } =
            _ => Task.FromException(new InvalidOperationException("unexpected collection purge"));
        public Func<EntityReference, Task> PurgeTag { get; set; } =
            _ => Task.FromException(new InvalidOperationException("unexpected tag purge"));

        public Task<CollectionMetadata[]> CollectionsAsync(CancellationToken cancellation, bool deleted = false)
            => Collections(cancellation);

        public Task<TagMetadata[]> TagsAsync(CancellationToken cancellation, bool deleted = false)
            => Tags(cancellation);

        public Task<CollectionMetadata> RestoreCollectionAsync(
            EntityReference target, long deletedRevision, CancellationToken cancellation)
            => RestoreCollection(target);

        public Task<TagMetadata> RestoreTagAsync(
            EntityReference target, long deletedRevision, CancellationToken cancellation)
            => RestoreTag(target);

        public Task PurgeCollectionAsync(
            EntityReference target, long deletedRevision, CancellationToken cancellation)
            => PurgeCollection(target);

        public Task PurgeTagAsync(
            EntityReference target, long deletedRevision, CancellationToken cancellation)
            => PurgeTag(target);
    }

    private sealed class FakeCatalogStrings : ICatalogStrings
    {
        public string this[string key] => key;
        public string Format(string key, params object[] arguments) => key;
        public string ErrorKey(Exception error) => error.Message;
    }
}
