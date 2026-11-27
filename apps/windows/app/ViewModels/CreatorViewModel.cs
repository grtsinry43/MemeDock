using System.Collections.ObjectModel;
using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;

namespace MemeDock.Windows.ViewModels;

/// <summary>
/// Platform-independent state machine for video-to-sticker creation.
/// Kept free of WinUI types so unit tests drive every transition deterministically.
/// </summary>
public sealed class CreatorViewModel : ObservableObject, IAsyncDisposable
{
    private readonly ICreatorWorkspace _workspace;
    private readonly ICatalogStrings _strings;
    private readonly Func<Task>? _onAdded;
    private readonly CancellationTokenSource _lifetime = new();
    private readonly List<ICreatorArtifact> _thumbnailArtifacts = [];

    private CancellationTokenSource? _workCancellation;
    private CancellationTokenSource? _renderCancellation;
    private CancellationTokenSource? _fallbackCancellation;
    private Task _work = Task.CompletedTask;
    private Task _fallbackWork = Task.CompletedTask;
    private ICreatorArtifact? _fallbackArtifact;
    private int _revision;
    private bool _stopping;
    private bool _disposed;

    private ICreatorVideo? _source;
    private string _fileName = "";
    private string? _fallbackFramePath;
    private CreatorGeneratedAnimation? _result;
    private bool _isBusy;
    private bool _isSaving;
    private string _stage = "";
    private string? _failureKey;
    private bool _isStale;
    private bool _showingResult;
    private bool _modeIsCreator;

    private double _start;
    private double _end = 1.0;
    private CreatorCrop _crop = CreatorCrop.Full;
    private double? _aspectLock;
    private int _maxEdge = 512;
    private int _fps = 15;
    private int _quality = 80;

    public CreatorViewModel(ICreatorWorkspace workspace, ICatalogStrings strings, Func<Task>? onAdded = null)
    {
        _workspace = workspace;
        _strings = strings;
        _onAdded = onAdded;
    }

    /// <summary>
    /// Raised synchronously before releasing the staged video handle so native players
    /// release file locks before the core deletes the temporary file.
    /// </summary>
    public event EventHandler? VideoReleasing;

    /// <summary>Raised after a newly opened video is ready for playback.</summary>
    public event EventHandler<ICreatorVideo>? VideoOpened;

    /// <summary>Raised when an edit requests the player to seek/update loop bounds.</summary>
    public event EventHandler<(double Start, double End, double? SeekTo)>? RangeAdjusted;

    /// <summary>Raised when a generated WebP is added to the library. Payload is true if restore is required.</summary>
    public event EventHandler<bool>? AddedToDock;

    public ObservableCollection<CreatorThumbnailItem> Thumbnails { get; } = [];

    public ICreatorVideo? Source => _source;
    public VideoInfo? Info => _source?.Info;
    public string FileName => _fileName;
    public string? FallbackFramePath => _fallbackFramePath;
    public CreatorGeneratedAnimation? Result => _result;
    public bool IsBusy => _isBusy;
    public bool IsSaving => _isSaving;
    public string Stage => _stage;
    public string StageText => _strings[CreatorMath.MapStageKey(_stage)];
    public string? FailureKey => _failureKey;
    public string FailureText => _failureKey is null ? "" : _strings[_failureKey];
    public bool IsStale => _isStale;

    public bool ShowingResult
    {
        get => _showingResult;
        set
        {
            if (_showingResult == value) return;
            _showingResult = value;
            NotifyAll();
        }
    }

    public bool ModeIsCreator
    {
        get => _modeIsCreator;
        set
        {
            if (_modeIsCreator == value) return;
            _modeIsCreator = value;
            OnPropertyChanged();
        }
    }

    public double Duration => Info is { } info ? info.DurationMs / 1000.0 : 0.0;
    public double Start => _start;
    public double End => _end;
    public CreatorCrop Crop => _crop;

    public double? AspectLock
    {
        get => _aspectLock;
        set
        {
            if (_aspectLock == value) return;
            _aspectLock = value;
            NotifyAll();
        }
    }

    public int MaxEdge
    {
        get => _maxEdge;
        set
        {
            var clamped = Math.Clamp(value, 32, 512);
            if (_maxEdge == clamped) return;
            _maxEdge = clamped;
            InvalidateEdit();
        }
    }

    public int Fps
    {
        get => _fps;
        set
        {
            var clamped = Math.Clamp(value, 1, 30);
            if (_fps == clamped) return;
            _fps = clamped;
            InvalidateEdit();
        }
    }

    public int Quality
    {
        get => _quality;
        set
        {
            var clamped = Math.Clamp(value, 1, 100);
            if (_quality == clamped) return;
            _quality = clamped;
            InvalidateEdit();
        }
    }

    public bool CanGenerate => _source is not null && !_isBusy && !_isSaving && _end > _start && _end - _start <= CreatorMath.MaxClipSeconds + 0.0001;
    public bool HasCurrentResult => _result is not null && !_isStale;
    public bool IsCropped => _crop.IsCropped;

    public CreatorDimensions? CroppedPixelSize => Info is { } info
        ? CreatorMath.CroppedPixels(info.Width, info.Height, _crop)
        : null;

    public CreatorDimensions? EstimatedOutputSize => Info is { } info
        ? CreatorMath.EstimatedOutputSize(info.Width, info.Height, _crop, _maxEdge)
        : null;

    public void Relocalize() => NotifyAll();

    public void SetCrop(CreatorCrop value)
    {
        if (_crop == value) return;
        _crop = value;
        InvalidateEdit();
    }

    public void ResetCrop()
    {
        _aspectLock = null;
        if (_crop != CreatorCrop.Full)
        {
            _crop = CreatorCrop.Full;
            InvalidateEdit();
        }
        else
        {
            NotifyAll();
        }
    }

    public void ApplyAspect(double? aspect)
    {
        if (Info is not { } info) return;
        _aspectLock = aspect;
        var next = CreatorMath.CenteredCrop(info.Width, info.Height, aspect);
        if (_crop != next)
        {
            _crop = next;
            InvalidateEdit();
        }
        else
        {
            NotifyAll();
        }
        if (_showingResult) ShowingResult = false;
    }

    public void SetStart(double value)
    {
        if (Duration <= 0 || !double.IsFinite(value)) return;
        var (start, end) = CreatorMath.ClampStart(value, _end, Duration);
        ApplyRange(start, end, seekTo: start);
    }

    public void SetEnd(double value)
    {
        if (Duration <= 0 || !double.IsFinite(value)) return;
        var (start, end) = CreatorMath.ClampEnd(value, _start, Duration);
        ApplyRange(start, end, seekTo: Math.Max(start, end - 0.04));
    }

    public void MoveRange(double newStart)
    {
        if (Duration <= 0 || !double.IsFinite(newStart)) return;
        var (start, end) = CreatorMath.MoveRange(newStart, _start, _end, Duration);
        ApplyRange(start, end, seekTo: start);
    }

    public void SetStartFromPlayhead(double position)
    {
        if (Duration <= 0 || !double.IsFinite(position)) return;
        var end = _end;
        if (position >= end - CreatorMath.MinClipSeconds)
            end = Math.Min(Duration, position + 0.5);
        var (start, clampedEnd) = CreatorMath.ClampStart(position, end, Duration);
        ApplyRange(start, clampedEnd, seekTo: start);
    }

    public void SetEndFromPlayhead(double position)
    {
        if (Duration <= 0 || !double.IsFinite(position)) return;
        var start = _start;
        if (position <= start + CreatorMath.MinClipSeconds)
            start = Math.Max(0, position - 0.5);
        var (clampedStart, end) = CreatorMath.ClampEnd(position, start, Duration);
        ApplyRange(clampedStart, end, seekTo: Math.Max(clampedStart, end - 0.04));
    }

    private void ApplyRange(double start, double end, double? seekTo)
    {
        var changed = Math.Abs(_start - start) > 1e-6 || Math.Abs(_end - end) > 1e-6;
        _start = start;
        _end = end;
        if (changed) InvalidateEdit();
        else NotifyAll();
        RangeAdjusted?.Invoke(this, (_start, _end, seekTo));
    }

    private void InvalidateEdit()
    {
        _revision++;
        _isStale = _result is not null;
        _failureKey = null;
        _renderCancellation?.Cancel();
        NotifyAll();
    }

    public Task OpenVideoAsync(string path)
    {
        if (_isBusy || _isSaving || _stopping || string.IsNullOrWhiteSpace(path)) return Task.CompletedTask;
        return _work = OpenVideoCoreAsync(path);
    }

    private async Task OpenVideoCoreAsync(string path)
    {
        CancelWork();
        var oldFallback = _fallbackWork;
        var previous = _source;
        VideoReleasing?.Invoke(this, EventArgs.Empty);

        _source = null;
        DisposeResult();
        ClearThumbnails();
        ClearFallbackArtifact();
        _isStale = false;
        _showingResult = false;
        _failureKey = null;
        _fileName = Path.GetFileName(path);
        _isBusy = true;
        _stage = "copying_video";
        NotifyAll();

        using var cancellation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
        _workCancellation = cancellation;
        var token = cancellation.Token;

        try
        {
            await WaitQuietlyAsync(oldFallback);
            if (previous is not null)
            {
                try { await previous.DisposeAsync(); }
                catch (Exception error) { Trace.WriteLine($"Discard previous creator video: {error}"); }
            }
            token.ThrowIfCancellationRequested();

            var progress = new Progress<string>(stage =>
            {
                if (!token.IsCancellationRequested && _stage != stage)
                {
                    _stage = stage;
                    NotifyAll();
                }
            });

            var video = await _workspace.OpenVideoAsync(path, progress, token);
            if (token.IsCancellationRequested)
            {
                try { await video.DisposeAsync(); } catch (Exception) { }
                return;
            }

            _source = video;
            _start = 0;
            _end = Math.Min(5.0, Duration);
            _aspectLock = null;
            _crop = CreatorCrop.Full;
            _revision++;
            _stage = "preparing_previews";
            NotifyAll();
            VideoOpened?.Invoke(this, video);

            foreach (var timeMs in CreatorMath.PreviewTimestampsMs(Duration, count: 8))
            {
                token.ThrowIfCancellationRequested();
                var frame = await _workspace.RenderPreviewFrameAsync(video, timeMs, token);
                if (token.IsCancellationRequested)
                {
                    frame.Dispose();
                    return;
                }
                _thumbnailArtifacts.Add(frame);
                Thumbnails.Add(new CreatorThumbnailItem(timeMs, frame.Path));
                if (_fallbackFramePath is null)
                {
                    _fallbackFramePath = frame.Path;
                    OnPropertyChanged(nameof(FallbackFramePath));
                }
            }
        }
        catch (Exception error)
        {
            if (!CreatorMath.IsCancelled(error) && !_stopping)
            {
                Trace.WriteLine($"Creator open failed: {error}");
                _failureKey = CreatorMath.MapErrorKey(error, _strings);
            }
        }
        finally
        {
            if (ReferenceEquals(_workCancellation, cancellation)) _workCancellation = null;
            _isBusy = false;
            NotifyAll();
        }
    }

    public Task GenerateAsync()
    {
        if (!CanGenerate || _source is not { } video || _stopping) return Task.CompletedTask;
        return _work = GenerateCoreAsync(video);
    }

    private async Task GenerateCoreAsync(ICreatorVideo video)
    {
        _isBusy = true;
        _failureKey = null;
        _stage = "encoding_webp";
        var tokenRevision = _revision;
        var edit = new VideoEditOptions(
            StartMs: (ulong)Math.Round(_start * 1000.0),
            EndMs: (ulong)Math.Round(_end * 1000.0),
            CropX: _crop.X,
            CropY: _crop.Y,
            CropWidth: _crop.Width,
            CropHeight: _crop.Height,
            MaxEdge: (uint)_maxEdge,
            Fps: (uint)_fps,
            Quality: (byte)_quality);
        NotifyAll();

        using var cancellation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
        _workCancellation = cancellation;
        _renderCancellation = cancellation;
        var token = cancellation.Token;

        try
        {
            var progress = new Progress<string>(stage =>
            {
                if (!token.IsCancellationRequested && _stage != stage)
                {
                    _stage = stage;
                    NotifyAll();
                }
            });

            var output = await _workspace.CreateWebpAsync(video, edit, progress, token);
            if (token.IsCancellationRequested || tokenRevision != _revision)
            {
                output.Dispose();
                return;
            }

            DisposeResult();
            _result = output;
            _isStale = false;
            _showingResult = true;
        }
        catch (Exception error)
        {
            if (!CreatorMath.IsCancelled(error) && !_stopping)
            {
                Trace.WriteLine($"Creator generate failed: {error}");
                _failureKey = CreatorMath.MapErrorKey(error, _strings);
            }
        }
        finally
        {
            if (ReferenceEquals(_workCancellation, cancellation)) _workCancellation = null;
            if (ReferenceEquals(_renderCancellation, cancellation)) _renderCancellation = null;
            _isBusy = false;
            NotifyAll();
        }
    }

    public Task<bool> AddToDockAsync(string? collectionId)
    {
        if (!HasCurrentResult || _result is not { } result || _isSaving || _stopping)
            return Task.FromResult(false);
        return AddToDockCoreAsync(result, collectionId);
    }

    private async Task<bool> AddToDockCoreAsync(CreatorGeneratedAnimation result, string? collectionId)
    {
        _isSaving = true;
        _failureKey = null;
        NotifyAll();
        try
        {
            var stem = Path.GetFileNameWithoutExtension(_fileName);
            if (string.IsNullOrWhiteSpace(stem)) stem = "sticker";
            var outcome = await _workspace.AddToDockAsync(
                result.Artifact,
                $"{stem}.webp",
                stem,
                collectionId,
                _lifetime.Token);
            if (_onAdded is not null) await _onAdded();
            AddedToDock?.Invoke(this, outcome.Status == ImportStatus.RestoreRequired);
            return true;
        }
        catch (Exception error)
        {
            if (!CreatorMath.IsCancelled(error) && !_stopping)
            {
                Trace.WriteLine($"Creator add to dock failed: {error}");
                _failureKey = CreatorMath.MapErrorKey(error, _strings);
            }
            return false;
        }
        finally
        {
            _isSaving = false;
            NotifyAll();
        }
    }

    public Task<ExportArtifact> PrepareHandoffAsync(CancellationToken cancellation)
    {
        if (_result is not { } result || _isStale)
            throw new InvalidOperationException("No current creator result is available.");
        return _workspace.PrepareHandoffAsync(result.Artifact, cancellation);
    }

    /// <summary>
    /// Debounced core-decoded preview frame for codecs the platform media player cannot play natively.
    /// </summary>
    public void RequestFallbackPreview(double positionSeconds, int debounceMs = 120)
    {
        if (_isBusy || _stopping || _source is not { } video || Duration <= 0) return;
        _fallbackCancellation?.Cancel();
        _fallbackCancellation?.Dispose();
        var cancellation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
        _fallbackCancellation = cancellation;
        var clamped = Math.Clamp(positionSeconds, 0, Math.Max(0, Duration - 0.001));
        var timeMs = (ulong)Math.Round(clamped * 1000.0);
        _fallbackWork = UpdateFallbackPreviewCoreAsync(video, timeMs, debounceMs, cancellation);
    }

    private async Task UpdateFallbackPreviewCoreAsync(
        ICreatorVideo video, ulong timeMs, int debounceMs, CancellationTokenSource cancellation)
    {
        var token = cancellation.Token;
        try
        {
            if (debounceMs > 0) await Task.Delay(debounceMs, token);
            token.ThrowIfCancellationRequested();
            if (!ReferenceEquals(_source, video)) return;

            var artifact = await _workspace.RenderPreviewFrameAsync(video, timeMs, token);
            if (token.IsCancellationRequested || !ReferenceEquals(_source, video))
            {
                artifact.Dispose();
                return;
            }

            ClearFallbackArtifact();
            _fallbackArtifact = artifact;
            _fallbackFramePath = artifact.Path;
            OnPropertyChanged(nameof(FallbackFramePath));
        }
        catch (Exception error)
        {
            if (!CreatorMath.IsCancelled(error) && !_stopping)
            {
                Trace.WriteLine($"Creator fallback preview failed: {error}");
                _failureKey = CreatorMath.MapErrorKey(error, _strings);
                NotifyAll();
            }
        }
    }

    public void CancelWork()
    {
        _workCancellation?.Cancel();
        _renderCancellation?.Cancel();
        _fallbackCancellation?.Cancel();
    }

    public void RequestStop()
    {
        _stopping = true;
        CancelWork();
        _lifetime.Cancel();
    }

    public async ValueTask DisposeAsync()
    {
        if (_disposed) return;
        _disposed = true;
        RequestStop();
        await WaitQuietlyAsync(_work);
        await WaitQuietlyAsync(_fallbackWork);

        VideoReleasing?.Invoke(this, EventArgs.Empty);
        var video = _source;
        _source = null;
        DisposeResult();
        ClearThumbnails();
        ClearFallbackArtifact();

        if (video is not null)
        {
            try { await video.DisposeAsync(); }
            catch (Exception error) { Trace.WriteLine($"Creator shutdown discard failed: {error}"); }
        }

        _fallbackCancellation?.Dispose();
        _fallbackCancellation = null;
        _lifetime.Dispose();
    }

    private void DisposeResult()
    {
        _result?.Dispose();
        _result = null;
    }

    private void ClearThumbnails()
    {
        Thumbnails.Clear();
        foreach (var artifact in _thumbnailArtifacts) artifact.Dispose();
        _thumbnailArtifacts.Clear();
    }

    private void ClearFallbackArtifact()
    {
        _fallbackArtifact?.Dispose();
        _fallbackArtifact = null;
        _fallbackFramePath = null;
    }

    private static async Task WaitQuietlyAsync(Task task)
    {
        try { await task; }
        catch (Exception) { }
    }

    private void NotifyAll() => OnPropertyChanged(string.Empty);
}
