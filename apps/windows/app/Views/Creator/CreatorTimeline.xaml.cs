using System.Globalization;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;

namespace MemeDock.Windows.Views.Creator;

public sealed partial class CreatorTimeline : UserControl
{
    private const double HandleWidth = 14.0;

    private enum DragMode { None, Scrub, LeftHandle, RightHandle, SlideRange }

    private LocalizationService? _strings;
    private double _duration;
    private double _start;
    private double _end;
    private double _position;
    private bool _playing;
    private bool _playbackAvailable = true;
    private bool _interactive = true;
    private string _thumbSignature = "";

    private DragMode _dragMode;
    private double _dragOriginX;
    private double _dragOriginValue;

    public event EventHandler? PlayPauseRequested;
    public event EventHandler<double>? ScrubRequested;
    public event EventHandler<double>? StartChanged;
    public event EventHandler<double>? EndChanged;
    public event EventHandler<double>? RangeMoved;
    public event EventHandler? SetStartFromPlayheadRequested;
    public event EventHandler? SetEndFromPlayheadRequested;
    public event EventHandler? InteractionStarted;

    public CreatorTimeline() => InitializeComponent();

    public void Relocalize(LocalizationService strings)
    {
        _strings = strings;
        ToolTipService.SetToolTip(SetStartButton, strings["creator_set_start"]);
        AutomationProperties.SetName(SetStartButton, strings["creator_set_start"]);
        ToolTipService.SetToolTip(SetEndButton, strings["creator_set_end"]);
        AutomationProperties.SetName(SetEndButton, strings["creator_set_end"]);
        AutomationProperties.SetName(FilmstripSurface, strings["creator_playhead"]);
        AutomationProperties.SetName(LeftHandle, strings["creator_start"]);
        AutomationProperties.SetName(RightHandle, strings["creator_end"]);
        UpdateTransportLabels();
    }

    public void UpdateState(double duration, double start, double end, double position,
        bool playing, bool playbackAvailable, bool interactive, IReadOnlyList<CreatorThumbnailItem> thumbnails)
    {
        _duration = Math.Max(0, duration);
        _start = start;
        _end = end;
        _position = position;
        _playing = playing;
        _playbackAvailable = playbackAvailable;
        _interactive = interactive;

        IsEnabled = interactive;
        PlayPauseButton.IsEnabled = interactive && playbackAvailable;
        UpdateTransportLabels();
        SyncThumbnails(thumbnails);
        LayoutTrack();
    }

    public void UpdatePlayhead(double position, bool playing, bool playbackAvailable)
    {
        _position = position;
        _playing = playing;
        _playbackAvailable = playbackAvailable;
        PlayPauseButton.IsEnabled = _interactive && playbackAvailable;
        UpdateTransportLabels();
        LayoutPlayhead();
    }

    private void UpdateTransportLabels()
    {
        PlayPauseGlyph.Glyph = _playing ? "\uE769" : "\uE768";
        if (_strings is not null)
        {
            var playLabel = _strings[_playing ? "pause_animation" : "play_animation"];
            ToolTipService.SetToolTip(PlayPauseButton, playLabel);
            AutomationProperties.SetName(PlayPauseButton, playLabel);
        }
        PlayheadTime.Text = CreatorMath.FormatTime(_position);
        RangeText.Text = $"{CreatorMath.FormatTime(_start)} – {CreatorMath.FormatTime(_end)}";
        DurationText.Text = string.Format(CultureInfo.InvariantCulture, "{0:0.00}s / 30s", Math.Max(0, _end - _start));
    }

    private void SyncThumbnails(IReadOnlyList<CreatorThumbnailItem> thumbnails)
    {
        var signature = string.Join("|", thumbnails.Select(t => $"{t.TimeMs}:{t.Path}"));
        if (signature == _thumbSignature) return;
        _thumbSignature = signature;

        ThumbnailsGrid.Children.Clear();
        ThumbnailsGrid.ColumnDefinitions.Clear();
        for (var index = 0; index < thumbnails.Count; index++)
        {
            ThumbnailsGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            var normalized = StickerOutputService.NormalizePath(thumbnails[index].Path);
            var image = new Image
            {
                Stretch = Stretch.UniformToFill,
                Source = new BitmapImage(new Uri(normalized))
            };
            Grid.SetColumn(image, index);
            ThumbnailsGrid.Children.Add(image);
        }
    }

    private void OnTrackSizeChanged(object sender, SizeChangedEventArgs args) => LayoutTrack();

    private void LayoutTrack()
    {
        var width = TrackHost.ActualWidth;
        if (width <= 1) return;
        var duration = Math.Max(0.001, _duration);
        var left = Math.Clamp(_start / duration, 0, 1) * width;
        var right = Math.Clamp(_end / duration, 0, 1) * width;
        var selectionWidth = Math.Max(HandleWidth * 2, right - left);

        Canvas.SetLeft(LeftDim, 0);
        LeftDim.Width = Math.Max(0, left);

        Canvas.SetLeft(RightDim, right);
        RightDim.Width = Math.Max(0, width - right);

        Canvas.SetLeft(SelectionFrame, left);
        SelectionFrame.Width = Math.Max(1, right - left);

        var leftHandleX = Math.Clamp(left, 0, Math.Max(0, width - HandleWidth * 2));
        var rightHandleX = Math.Max(leftHandleX + HandleWidth, Math.Min(width - HandleWidth, right - HandleWidth));
        Canvas.SetLeft(LeftHandle, leftHandleX);
        Canvas.SetLeft(RightHandle, rightHandleX);

        var innerWidth = rightHandleX - (leftHandleX + HandleWidth);
        if (innerWidth > 24)
        {
            RangeSlideGrip.Visibility = Visibility.Visible;
            RangeSlideGrip.Width = innerWidth;
            SlideBarVisual.Width = Math.Min(36, innerWidth * 0.45);
            Canvas.SetLeft(RangeSlideGrip, leftHandleX + HandleWidth);
        }
        else
        {
            RangeSlideGrip.Visibility = Visibility.Collapsed;
        }

        LayoutPlayhead();
    }

    private void LayoutPlayhead()
    {
        var width = TrackHost.ActualWidth;
        if (width <= 1) return;
        var duration = Math.Max(0.001, _duration);
        var x = Math.Clamp(_position / duration * width, 1, Math.Max(1, width - 1));
        Canvas.SetLeft(PlayheadNeedle, x - 4);
    }

    private void OnPlayPause(object sender, RoutedEventArgs args) => PlayPauseRequested?.Invoke(this, EventArgs.Empty);
    private void OnSetStartFromPlayhead(object sender, RoutedEventArgs args) => SetStartFromPlayheadRequested?.Invoke(this, EventArgs.Empty);
    private void OnSetEndFromPlayhead(object sender, RoutedEventArgs args) => SetEndFromPlayheadRequested?.Invoke(this, EventArgs.Empty);

    private void OnScrubPressed(object sender, PointerRoutedEventArgs args)
    {
        if (!_interactive || !args.GetCurrentPoint(TrackHost).Properties.IsLeftButtonPressed) return;
        _dragMode = DragMode.Scrub;
        FilmstripSurface.CapturePointer(args.Pointer);
        InteractionStarted?.Invoke(this, EventArgs.Empty);
        SeekAt(args.GetCurrentPoint(TrackHost).Position.X);
        args.Handled = true;
    }

    private void OnScrubMoved(object sender, PointerRoutedEventArgs args)
    {
        if (_dragMode != DragMode.Scrub) return;
        SeekAt(args.GetCurrentPoint(TrackHost).Position.X);
        args.Handled = true;
    }

    private void OnScrubReleased(object sender, PointerRoutedEventArgs args)
    {
        if (_dragMode != DragMode.Scrub) return;
        _dragMode = DragMode.None;
        FilmstripSurface.ReleasePointerCapture(args.Pointer);
        args.Handled = true;
    }

    private void SeekAt(double x)
    {
        var width = Math.Max(1, TrackHost.ActualWidth);
        var fraction = Math.Clamp(x / width, 0, 1);
        ScrubRequested?.Invoke(this, fraction * Math.Max(0.001, _duration));
    }

    private void OnLeftHandlePressed(object sender, PointerRoutedEventArgs args)
        => BeginHandleDrag(LeftHandle, DragMode.LeftHandle, _start, args);

    private void OnRightHandlePressed(object sender, PointerRoutedEventArgs args)
        => BeginHandleDrag(RightHandle, DragMode.RightHandle, _end, args);

    private void OnSlidePressed(object sender, PointerRoutedEventArgs args)
        => BeginHandleDrag(RangeSlideGrip, DragMode.SlideRange, _start, args);

    private void BeginHandleDrag(UIElement element, DragMode mode, double originValue, PointerRoutedEventArgs args)
    {
        if (!_interactive || !args.GetCurrentPoint(TrackHost).Properties.IsLeftButtonPressed) return;
        _dragMode = mode;
        _dragOriginX = args.GetCurrentPoint(TrackHost).Position.X;
        _dragOriginValue = originValue;
        element.CapturePointer(args.Pointer);
        InteractionStarted?.Invoke(this, EventArgs.Empty);
        args.Handled = true;
    }

    private void OnHandleMoved(object sender, PointerRoutedEventArgs args)
    {
        if (_dragMode is not (DragMode.LeftHandle or DragMode.RightHandle)) return;
        var width = Math.Max(1, TrackHost.ActualWidth);
        var delta = (args.GetCurrentPoint(TrackHost).Position.X - _dragOriginX) / width * Math.Max(0.001, _duration);
        var seconds = _dragOriginValue + delta;
        if (_dragMode == DragMode.LeftHandle) StartChanged?.Invoke(this, seconds);
        else EndChanged?.Invoke(this, seconds);
        args.Handled = true;
    }

    private void OnSlideMoved(object sender, PointerRoutedEventArgs args)
    {
        if (_dragMode != DragMode.SlideRange) return;
        var width = Math.Max(1, TrackHost.ActualWidth);
        var delta = (args.GetCurrentPoint(TrackHost).Position.X - _dragOriginX) / width * Math.Max(0.001, _duration);
        RangeMoved?.Invoke(this, _dragOriginValue + delta);
        args.Handled = true;
    }

    private void OnHandleReleased(object sender, PointerRoutedEventArgs args)
    {
        if (_dragMode is not (DragMode.LeftHandle or DragMode.RightHandle) || sender is not UIElement element) return;
        _dragMode = DragMode.None;
        element.ReleasePointerCapture(args.Pointer);
        args.Handled = true;
    }

    private void OnSlideReleased(object sender, PointerRoutedEventArgs args)
    {
        if (_dragMode != DragMode.SlideRange) return;
        _dragMode = DragMode.None;
        RangeSlideGrip.ReleasePointerCapture(args.Pointer);
        args.Handled = true;
    }

    private void OnPointerCaptureLost(object sender, PointerRoutedEventArgs args) => _dragMode = DragMode.None;
}
