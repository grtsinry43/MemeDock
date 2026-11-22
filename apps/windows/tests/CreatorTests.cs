using MemeDock.Bridge;
using MemeDock.Windows;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using MemeDock.Windows.ViewModels;

internal static class CreatorTests
{
    public static async Task RunAsync(string repositoryRoot)
    {
        CropAndTimelineMath();
        ErrorAndStageMapping();
        GifSizeLimitAndPreferences();
        SyncBadgeHelpers();
        await ViewModelLifecycleAsync();
        await ViewModelStaleRenderDiscardedAsync();
        await RealCreatorRoundTripAsync(repositoryRoot);
        Console.WriteLine("PASS: creator math, view model lifecycle, real native round-trip, GIF size limit, and sync badge helpers");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }

    private static void CropAndTimelineMath()
    {
        var cropped = CreatorMath.CroppedPixels(1920, 1080, new CreatorCrop(0.25, 0, 0.5, 1));
        Check(cropped.Width == 960 && cropped.Height == 1080, "Cropped pixel dimensions were wrong.");

        var downscaled = CreatorMath.EstimatedOutputSize(1920, 1080, CreatorCrop.Full, 512);
        Check(downscaled.Width == 512 && downscaled.Height == 288, $"Unexpected downscaled size: {downscaled}.");

        var preserved = CreatorMath.EstimatedOutputSize(320, 240, CreatorCrop.Full, 512);
        Check(preserved.Width == 320 && preserved.Height == 240, "Smaller sources must keep their original size.");

        var square = CreatorMath.CenteredCrop(1920, 1080, 1.0);
        Check(Math.Abs(square.Width - (1080.0 / 1920.0)) < 1e-6 && Math.Abs(square.Height - 1.0) < 1e-6,
            "1:1 centered crop on 16:9 video had wrong proportions.");
        Check(Math.Abs(square.X - (1.0 - square.Width) / 2.0) < 1e-6, "Centered crop was not horizontally centered.");

        var portrait = CreatorMath.CenteredCrop(1080, 1920, 16.0 / 9.0);
        Check(Math.Abs(portrait.Width - 1.0) < 1e-6 && portrait.Height < 0.4,
            "16:9 centered crop on portrait video had wrong proportions.");

        var moved = CreatorMath.MoveCrop(new CreatorCrop(0.5, 0.5, 0.4, 0.4), 0.5, -1.0);
        Check(Math.Abs(moved.X - 0.6) < 1e-9 && Math.Abs(moved.Y) < 1e-9, "Moved crop escaped unit bounds.");

        var locked = CreatorMath.ResizeCrop(square, CreatorCropHandle.BottomRight, -0.1, -0.05, 1.0, 1920, 1080);
        var lockedPx = CreatorMath.CroppedPixels(1920, 1080, locked);
        Check(Math.Abs(lockedPx.Width - lockedPx.Height) <= 1, "Aspect-locked corner resize drifted from 1:1.");

        var edge = CreatorMath.ResizeCrop(CreatorCrop.Full, CreatorCropHandle.Right, -0.5, 0.0, null, 1920, 1080);
        Check(Math.Abs(edge.Width - 0.5) < 1e-9 && Math.Abs(edge.Height - 1.0) < 1e-9, "Right edge resize drifted.");

        // Timeline clamping: 30s ceiling and 0.05s minimum gap
        var (s1, e1) = CreatorMath.ClampEnd(45.0, 2.0, 60.0);
        Check(Math.Abs(e1 - 45.0) < 1e-9 && Math.Abs(s1 - 15.0) < 1e-9, "30s clip ceiling did not pull start forward.");

        var (s2, e2) = CreatorMath.ClampStart(5.0, 5.02, 10.0);
        Check(e2 - s2 >= CreatorMath.MinClipSeconds - 1e-6, "Minimum clip gap was violated.");

        var (ms, me) = CreatorMath.MoveRange(9.0, 1.0, 4.0, 10.0);
        Check(Math.Abs(ms - 7.0) < 1e-9 && Math.Abs(me - 10.0) < 1e-9, "Range slide escaped video duration.");

        var stamps = CreatorMath.PreviewTimestampsMs(10.0, 8);
        Check(stamps.Length == 8 && stamps[0] == 0 && stamps[^1] == 9999, "Preview timestamps did not span the clip.");

        Check(CreatorMath.FormatTime(4.25) == "4.25s", "Sub-minute time formatting drifted.");
        Check(CreatorMath.FormatTime(65.5) == "1:05.50", "Minute time formatting drifted.");
    }

    private static void ErrorAndStageMapping()
    {
        var strings = new FakeStrings();
        Check(CreatorMath.MapStageKey("copying_video") == "creator_reading", "Stage copying_video mapping drifted.");
        Check(CreatorMath.MapStageKey("preparing_previews") == "creator_preparing_previews", "Stage preview mapping drifted.");
        Check(CreatorMath.MapStageKey("validating_creation") == "creator_validating", "Stage validating mapping drifted.");
        Check(CreatorMath.MapStageKey("encoding_webp") == "creator_generating", "Stage encoding mapping drifted.");

        Check(CreatorMath.MapErrorKey(new BridgeException.Failure(ErrorCode.UnsupportedColorProfile, "hdr"), strings) == "creator_sdr_only",
            "HDR error did not map to creator_sdr_only.");
        Check(CreatorMath.MapErrorKey(new BridgeException.Failure(ErrorCode.ResourceLimit, "decoder budget exceeded"), strings) == "creator_input_resolution",
            "Decoder budget error did not map to creator_input_resolution.");
        Check(CreatorMath.MapErrorKey(new BridgeException.Failure(ErrorCode.ResourceLimit, "video exceeds 2 GiB"), strings) == "creator_input_size",
            "2 GiB error did not map to creator_input_size.");
        Check(CreatorMath.MapErrorKey(new BridgeException.Failure(ErrorCode.ResourceLimit, "other"), strings) == "creator_resource_limit",
            "Generic limit error did not map to creator_resource_limit.");
        Check(CreatorMath.MapErrorKey(new BridgeException.Failure(ErrorCode.InvalidImage, "bad"), strings) == "creator_invalid_video",
            "Invalid image did not map to creator_invalid_video.");
        Check(CreatorMath.MapErrorKey(new BridgeException.Failure(ErrorCode.UnsupportedFormat, "codec"), strings) == "creator_unsupported",
            "Unsupported format did not map to creator_unsupported.");
        Check(CreatorMath.MapErrorKey(new BridgeException.Failure(ErrorCode.InvalidInput, "range"), strings) == "creator_invalid_selection",
            "Invalid input did not map to creator_invalid_selection.");
    }

    private static void GifSizeLimitAndPreferences()
    {
        Check(GifSizeLimitPolicy.Clamp(0) == 1 && GifSizeLimitPolicy.Clamp(99) == 10, "GIF MiB clamp failed.");
        Check(GifSizeLimitPolicy.MaxBytes(false, 5) is null, "Disabled GIF limit must produce null.");
        Check(GifSizeLimitPolicy.MaxBytes(true, 5) == 5UL * 1024 * 1024, "Enabled GIF limit bytes drifted.");

        var limit = 4UL * 1024 * 1024;
        var gif = StickerOutputPresets.Options(ExportPreset.CompatibleGif, animated: true, limit);
        Check(gif.MaxBytes == limit, "CompatibleGif did not receive the GIF size limit.");
        var stillBest = StickerOutputPresets.Options(ExportPreset.BestCompatibility, animated: false, limit);
        Check(stillBest.MaxBytes is null, "Static BestCompatibility must not receive a GIF size limit.");
        var orig = StickerOutputPolicy.Options(animated: true, StickerOutputKind.OriginalFile, OutputDefaults.CopyDefault, limit);
        Check(orig.MaxBytes is null, "Original file export must not receive a GIF size limit.");
        var explicitGif = StickerOutputPolicy.Options(animated: true, StickerOutputKind.CompatibleGif, OutputDefaults.CopyDefault, limit);
        Check(explicitGif.Preset == ExportPreset.CompatibleGif && explicitGif.MaxBytes == limit,
            "Explicit CompatibleGif kind must emit a size-limited GIF even when Copy default is Original.");
        Check(GifLimitOverride.Unlimited.MaxBytes is null
              && GifLimitOverride.Limit(7).MaxBytes == 7UL * 1024 * 1024
              && GifLimitOverride.Limit(99).MiB == 10,
            "Per-session GIF limit override bounds drifted.");

        var directory = Path.Combine(Path.GetTempPath(), "memedock-gif-prefs-" + Guid.NewGuid().ToString("N"));
        try
        {
            Directory.CreateDirectory(directory);
            var prefs = new AppPreferences(directory);
            Check(prefs.GifSizeLimitEnabled && prefs.GifSizeLimitMiB == 5, "Default GIF limit drifted from 5 MiB.");
            prefs.SaveGifSizeLimitAsync(true, 8).GetAwaiter().GetResult();

            var reloaded = new AppPreferences(directory);
            reloaded.Load();
            Check(reloaded.GifSizeLimitEnabled && reloaded.GifSizeLimitMiB == 8
                && reloaded.EffectiveGifMaxBytes == 8UL * 1024 * 1024,
                "Saved GIF limit did not survive reload.");

            reloaded.SaveGifSizeLimitAsync(false, 3).GetAwaiter().GetResult();
            var reloadedDisabled = new AppPreferences(directory);
            reloadedDisabled.Load();
            Check(!reloadedDisabled.GifSizeLimitEnabled && reloadedDisabled.GifSizeLimitMiB == 3
                && reloadedDisabled.EffectiveGifMaxBytes is null,
                "Disabled GIF limit did not survive reload.");
        }
        finally
        {
            if (Directory.Exists(directory)) Directory.Delete(directory, recursive: true);
        }
    }

    private static void SyncBadgeHelpers()
    {
        var baseStatus = SyncState.Initial.Status with { Connected = true, LastSuccessMs = 1_700_000_000_000 };
        var downloading = SyncState.Initial with { Loaded = true, HasCredential = true, Status = baseStatus with { Downloading = 2 } };
        Check(SyncPresentation.IsActiveTransfer(downloading), "Background asset downloads must count as active transfer.");

        var cleanIdle = SyncState.Initial with { Loaded = true, HasCredential = true, Status = baseStatus };
        Check(SyncPresentation.ShouldCelebrateCompletion(wasActive: true, cleanIdle),
            "Clean idle after an active transfer should trigger settled celebration.");
        Check(!SyncPresentation.ShouldCelebrateCompletion(wasActive: false, cleanIdle),
            "Already-idle state must not trigger celebration.");

        var pending = cleanIdle with { Status = baseStatus with { Pending = 1 } };
        Check(!SyncPresentation.ShouldCelebrateCompletion(wasActive: true, pending),
            "Pending changes must not celebrate completion.");

        var now = DateTimeOffset.FromUnixTimeMilliseconds(1_700_000_000_000);
        Check(SyncPresentation.FormatLastSynced(now.AddSeconds(-20).ToUnixTimeMilliseconds(), now, false) == "Just now", "Relative seconds EN drifted.");
        Check(SyncPresentation.FormatLastSynced(now.AddMinutes(-5).ToUnixTimeMilliseconds(), now, true) == "5 分钟前", "Relative minutes ZH drifted.");
        Check(SyncPresentation.FormatLastSynced(now.AddHours(-3).ToUnixTimeMilliseconds(), now, false) == "3h ago", "Relative hours EN drifted.");
    }

    private static async Task ViewModelLifecycleAsync()
    {
        var workspace = new FakeCreatorWorkspace();
        var strings = new FakeStrings();
        var addedCallbackCount = 0;
        var releaseOrder = new List<string>();
        bool? addedRestoreFlag = null;

        await using var vm = new CreatorViewModel(workspace, strings, () =>
        {
            addedCallbackCount++;
            return Task.CompletedTask;
        });

        vm.VideoReleasing += (_, _) => releaseOrder.Add("ui-release");
        vm.AddedToDock += (_, restore) => addedRestoreFlag = restore;
        workspace.OnVideoDisposed = () => releaseOrder.Add("core-discard");

        await vm.OpenVideoAsync(@"C:\clips\funny-cat.mp4");
        Check(vm.Source is not null && vm.FileName == "funny-cat.mp4", "Video did not open.");
        Check(Math.Abs(vm.Start) < 1e-9 && Math.Abs(vm.End - 5.0) < 1e-9, "Default trim range was not [0, 5].");
        Check(vm.Thumbnails.Count == 8 && vm.FallbackFramePath is not null, "Preview thumbnails were not populated.");
        Check(vm.CanGenerate, "Opened video should be ready to generate.");

        await vm.GenerateAsync();
        Check(vm.Result is not null && vm.HasCurrentResult && vm.ShowingResult && !vm.IsStale,
            "Generate did not produce a current result.");

        // Editing marks the result stale
        vm.Fps = 24;
        Check(vm.IsStale && !vm.HasCurrentResult, "Editing FPS must mark the existing result stale.");

        await vm.GenerateAsync();
        Check(vm.HasCurrentResult && !vm.IsStale, "Regenerating must clear stale state.");

        var added = await vm.AddToDockAsync("col-1");
        Check(added && addedCallbackCount == 1 && addedRestoreFlag == false, "Add to Dock did not complete cleanly.");
        Check(workspace.LastImportName == "funny-cat.webp" && workspace.LastImportTitle == "funny-cat" && workspace.LastImportCollection == "col-1",
            "Add to Dock metadata drifted.");

        // Opening a second video must signal UI release BEFORE discarding the first native handle
        releaseOrder.Clear();
        await vm.OpenVideoAsync(@"C:\clips\second.mp4");
        Check(releaseOrder.Count == 2 && releaseOrder[0] == "ui-release" && releaseOrder[1] == "core-discard",
            $"Expected ui-release before core-discard, got: {string.Join(", ", releaseOrder)}");
    }

    private static async Task ViewModelStaleRenderDiscardedAsync()
    {
        var workspace = new FakeCreatorWorkspace();
        var gate = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        workspace.RenderGate = gate;

        await using var vm = new CreatorViewModel(workspace, new FakeStrings());
        await vm.OpenVideoAsync(@"C:\clips\clip.mp4");

        var generateTask = vm.GenerateAsync();
        // Change an edit parameter while generation is in flight -> cancels & invalidates token
        vm.Quality = 60;
        gate.TrySetResult();
        await generateTask;

        Check(vm.Result is null, "Outdated render result must not overwrite state after an edit.");
    }

    private static async Task RealCreatorRoundTripAsync(string repositoryRoot)
    {
        var fixtureHex = Path.Combine(repositoryRoot, "crates/core/tests/fixtures/creator-h264.mp4.hex");
        var bytes = Convert.FromHexString(string.Concat(File.ReadAllText(fixtureHex).Where(c => !char.IsWhiteSpace(c))));
        var root = Path.Combine(Path.GetTempPath(), "memedock-creator-e2e-" + Guid.NewGuid().ToString("N"));
        try
        {
            Directory.CreateDirectory(root);
            var videoPath = Path.Combine(root, "sample-clip.mp4");
            await File.WriteAllBytesAsync(videoPath, bytes);

            await using var session = await LibrarySession.OpenAsync(Path.Combine(root, "library"));
            var stagedPath = "";
            await using (var vm = new CreatorViewModel(session, new FakeStrings()))
            {
                await vm.OpenVideoAsync(videoPath);
                Check(vm.FailureKey is null, $"Real video open failed: {vm.FailureKey}");
                Check(vm.Source is not null && vm.Info is { Width: > 0, Height: > 0, DurationMs: > 0 },
                    "Real video metadata was not decoded.");
                stagedPath = vm.Source!.Path;
                Check(File.Exists(stagedPath), "Staged creator video copy does not exist on disk.");
                Check(vm.Thumbnails.Count > 0, "Real preview thumbnails were not rendered.");

                vm.ApplyAspect(1.0);
                vm.MaxEdge = 128;
                vm.Fps = 10;
                vm.Quality = 75;
                await vm.GenerateAsync();
                Check(vm.FailureKey is null && vm.Result is not null && vm.HasCurrentResult,
                    $"Real WebP creation failed: {vm.FailureKey}");
                Check(vm.Result!.Width <= 128 && vm.Result.Height <= 128 && vm.Result.Artifact.ByteSize > 0,
                    "Created WebP dimensions or byte size were invalid.");

                var handoff = await vm.PrepareHandoffAsync(CancellationToken.None);
                Check(File.Exists(handoff.Path) && handoff.Animated && handoff.Mime == "image/webp",
                    "Creator handoff did not produce an animated WebP file.");

                var added = await vm.AddToDockAsync(null);
                Check(added, "Adding real created WebP to Dock failed.");
            }

            // Creator disposal must discard the staged copy cleanly on Windows.
            Check(!File.Exists(stagedPath), "Staged creator video survived CreatorViewModel disposal.");

            var page = await session.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, CancellationToken.None);
            Check(page.Stickers.Length == 1 && page.Stickers[0].Title == "sample-clip"
                && page.Resources.Length == 1 && page.Resources[0].Asset.Animated && page.Resources[0].Asset.Mime == "image/webp",
                "Created animated WebP was not persisted in the library.");
        }
        finally
        {
            if (Directory.Exists(root)) Directory.Delete(root, recursive: true);
        }
    }

    private sealed class FakeStrings : ICatalogStrings
    {
        public string this[string key] => key;
        public string Format(string key, params object[] arguments) => key;
        public string ErrorKey(Exception error) => "failure_unknown";
    }

    private sealed class FakeCreatorWorkspace : ICreatorWorkspace
    {
        public Action? OnVideoDisposed { get; set; }
        public TaskCompletionSource? RenderGate { get; set; }
        public string? LastImportName { get; private set; }
        public string? LastImportTitle { get; private set; }
        public string? LastImportCollection { get; private set; }

        public Task<ICreatorVideo> OpenVideoAsync(string path, IProgress<string>? stages, CancellationToken cancellation)
        {
            stages?.Report("inspecting_video");
            return Task.FromResult<ICreatorVideo>(new FakeVideo(path, OnVideoDisposed));
        }

        public Task<ICreatorArtifact> RenderPreviewFrameAsync(ICreatorVideo video, ulong timeMs, CancellationToken cancellation)
            => Task.FromResult<ICreatorArtifact>(new FakeArtifact($"frame-{timeMs}.png", 1024));

        public async Task<CreatorGeneratedAnimation> CreateWebpAsync(ICreatorVideo video, VideoEditOptions edit,
            IProgress<string>? stages, CancellationToken cancellation)
        {
            stages?.Report("encoding_webp");
            if (RenderGate is { } gate)
            {
                await gate.Task.WaitAsync(cancellation);
            }
            cancellation.ThrowIfCancellationRequested();
            var artifact = new FakeArtifact("created.webp", 65536);
            return new CreatorGeneratedAnimation(artifact, edit.MaxEdge, edit.MaxEdge, edit.EndMs - edit.StartMs, edit.Fps);
        }

        public Task<ImportOutcome> AddToDockAsync(ICreatorArtifact artifact, string originalName, string title,
            string? collectionId, CancellationToken cancellation)
        {
            LastImportName = originalName;
            LastImportTitle = title;
            LastImportCollection = collectionId;
            var metadata = new StickerMetadata("sticker-1", title, originalName, "", false,
                new EntityLifecycle(1, 1, 0, 0, null, null));
            return Task.FromResult(new ImportOutcome(metadata, ImportStatus.Created));
        }

        public Task<ExportArtifact> PrepareHandoffAsync(ICreatorArtifact artifact, CancellationToken cancellation)
            => Task.FromResult(new ExportArtifact("exp-1", artifact.Path, artifact.Mime, artifact.FileName, artifact.ByteSize, true, 0));

        private sealed class FakeVideo(string path, Action? onDispose) : ICreatorVideo
        {
            public string Path { get; } = path;
            public VideoInfo Info { get; } = new("video/mp4", 1920, 1080, 12_000, 30.0);
            public ValueTask DisposeAsync()
            {
                onDispose?.Invoke();
                return ValueTask.CompletedTask;
            }
        }

        private sealed class FakeArtifact(string path, ulong bytes) : ICreatorArtifact
        {
            public string Path { get; } = path;
            public string FileName { get; } = System.IO.Path.GetFileName(path);
            public string Mime => "image/webp";
            public ulong ByteSize { get; } = bytes;
            public ArtifactLeaseHandle? Lease => null;
            public void Dispose() { }
        }
    }
}
