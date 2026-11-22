using System.Diagnostics;
using MemeDock.Windows.Services;
using Microsoft.UI.Dispatching;
using Windows.Media.Core;
using Windows.Media.Playback;

namespace MemeDock.Windows.Views.Creator;

/// <summary>
/// Wraps WinUI's <see cref="MediaPlayer"/> with ~30 Hz playhead tracking,
/// seamless looping within <c>[Lower, Upper]</c>, serialized seeks, and deterministic
/// release of file handles before the core discards the staged video.
/// </summary>
internal sealed class CreatorMediaPlayer : IDisposable
{
    private readonly DispatcherQueue _dispatcher;
    private readonly DispatcherQueueTimer _timer;
    private MediaSource? _source;
    private double _lower;
    private double _upper;
    private double _totalDuration;
    private double? _desiredSeek;
    private bool _seeking;
    private bool _resumeAfterSeek;
    private int _generation;
    private bool _disposed;

    public MediaPlayer Player { get; }
    public double Position { get; private set; }
    public bool IsPlaying { get; private set; }
    public bool IsAvailable { get; private set; } = true;

    public event EventHandler? PositionChanged;
    public event EventHandler? StateChanged;

    public CreatorMediaPlayer(DispatcherQueue dispatcher)
    {
        _dispatcher = dispatcher;
        Player = new MediaPlayer
        {
            AutoPlay = false,
            IsLoopingEnabled = false,
            RealTimePlayback = false
        };
        Player.MediaOpened += OnMediaOpened;
        Player.MediaFailed += OnMediaFailed;
        Player.MediaEnded += OnMediaEnded;
        Player.PlaybackSession.SeekCompleted += OnSeekCompleted;

        _timer = _dispatcher.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(33);
        _timer.IsRepeating = true;
        _timer.Tick += OnTick;
    }

    public void Load(string path, double duration, double end)
    {
        Clear();
        _totalDuration = Math.Max(0, duration);
        _lower = 0;
        _upper = Math.Max(0, end);
        IsAvailable = true;
        Position = 0;

        try
        {
            var normalized = StickerOutputService.NormalizePath(path);
            _source = MediaSource.CreateFromUri(new Uri(normalized));
            Player.Source = _source;
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Creator media load failed: {error}");
            IsAvailable = false;
        }

        StateChanged?.Invoke(this, EventArgs.Empty);
        PositionChanged?.Invoke(this, EventArgs.Empty);
    }

    public void SetRange(double start, double end)
    {
        _lower = Math.Max(0, start);
        _upper = Math.Max(_lower, end);
        if (Position < _lower || Position > _upper)
            Seek(_lower);
    }

    public void Seek(double seconds)
    {
        var clamped = Math.Max(0, _totalDuration > 0 ? Math.Min(_totalDuration, seconds) : seconds);
        Position = clamped;
        PositionChanged?.Invoke(this, EventArgs.Empty);
        if (!IsAvailable || _source is null) return;
        _desiredSeek = clamped;
        ChaseSeek(resumeAfter: false);
    }

    public void Step(double delta)
    {
        Pause();
        Seek(Position + delta);
    }

    public void Toggle()
    {
        if (!IsAvailable || _source is null) return;
        if (IsPlaying)
        {
            Pause();
            return;
        }

        if (Position >= _upper - 0.025 || Position < _lower)
        {
            Position = _lower;
            _desiredSeek = _lower;
            IsPlaying = true;
            _timer.Start();
            ChaseSeek(resumeAfter: true);
        }
        else
        {
            IsPlaying = true;
            _timer.Start();
            Player.Play();
        }
        StateChanged?.Invoke(this, EventArgs.Empty);
    }

    public void Pause()
    {
        _timer.Stop();
        _resumeAfterSeek = false;
        if (_source is not null)
        {
            try { Player.Pause(); }
            catch (Exception error) { Trace.WriteLine($"Creator media pause: {error.GetType().Name}"); }
        }
        if (!IsPlaying) return;
        IsPlaying = false;
        StateChanged?.Invoke(this, EventArgs.Empty);
    }

    public void Clear()
    {
        _generation++;
        Pause();
        _desiredSeek = null;
        _seeking = false;
        _resumeAfterSeek = false;
        Player.Source = null;
        _source?.Dispose();
        _source = null;
        Position = 0;
        _lower = 0;
        _upper = 0;
        _totalDuration = 0;
        IsAvailable = true;
        StateChanged?.Invoke(this, EventArgs.Empty);
        PositionChanged?.Invoke(this, EventArgs.Empty);
    }

    private void LoopToStart()
    {
        if (!IsAvailable || !IsPlaying) return;
        Position = _lower;
        _desiredSeek = _lower;
        PositionChanged?.Invoke(this, EventArgs.Empty);
        ChaseSeek(resumeAfter: true);
    }

    private void ChaseSeek(bool resumeAfter)
    {
        if (_seeking || _desiredSeek is not { } seconds || _source is null) return;
        if (!Player.PlaybackSession.CanSeek)
        {
            _desiredSeek = null;
            if (resumeAfter && IsPlaying) Player.Play();
            return;
        }
        _desiredSeek = null;
        _seeking = true;
        _resumeAfterSeek = resumeAfter;
        try
        {
            Player.PlaybackSession.Position = TimeSpan.FromSeconds(seconds);
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Creator seek failed: {error.GetType().Name}");
            _seeking = false;
            _resumeAfterSeek = false;
        }
    }

    private void OnTick(DispatcherQueueTimer sender, object args)
    {
        if (!IsAvailable || _source is null || _seeking) return;
        var seconds = Player.PlaybackSession.Position.TotalSeconds;
        if (!double.IsFinite(seconds)) return;
        Position = seconds;
        PositionChanged?.Invoke(this, EventArgs.Empty);
        if (IsPlaying && seconds >= _upper - 0.025)
            LoopToStart();
    }

    private void OnMediaOpened(MediaPlayer sender, object args)
    {
        var token = _generation;
        _dispatcher.TryEnqueue(() =>
        {
            if (_disposed || token != _generation) return;
            IsAvailable = true;
            if (_desiredSeek is not null) ChaseSeek(_resumeAfterSeek);
            StateChanged?.Invoke(this, EventArgs.Empty);
        });
    }

    private void OnMediaFailed(MediaPlayer sender, MediaPlayerFailedEventArgs args)
    {
        var token = _generation;
        Trace.WriteLine($"Creator media playback unavailable ({args.Error}): {args.ErrorMessage}");
        _dispatcher.TryEnqueue(() =>
        {
            if (_disposed || token != _generation) return;
            Pause();
            IsAvailable = false;
            StateChanged?.Invoke(this, EventArgs.Empty);
        });
    }

    private void OnMediaEnded(MediaPlayer sender, object args)
    {
        var token = _generation;
        _dispatcher.TryEnqueue(() =>
        {
            if (_disposed || token != _generation || !IsPlaying) return;
            LoopToStart();
        });
    }

    private void OnSeekCompleted(MediaPlaybackSession sender, object args)
    {
        var token = _generation;
        _dispatcher.TryEnqueue(() =>
        {
            if (_disposed || token != _generation) return;
            _seeking = false;
            var resume = _resumeAfterSeek;
            _resumeAfterSeek = false;
            if (_desiredSeek is not null)
            {
                ChaseSeek(resume);
                return;
            }
            if (resume && IsPlaying && Player.PlaybackSession.PlaybackState != MediaPlaybackState.Playing)
                Player.Play();
        });
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _timer.Stop();
        _timer.Tick -= OnTick;
        Player.MediaOpened -= OnMediaOpened;
        Player.MediaFailed -= OnMediaFailed;
        Player.MediaEnded -= OnMediaEnded;
        Player.PlaybackSession.SeekCompleted -= OnSeekCompleted;
        Clear();
        Player.Dispose();
    }
}
