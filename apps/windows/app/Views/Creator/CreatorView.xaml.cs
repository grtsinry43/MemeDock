using System.ComponentModel;
using System.Diagnostics;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using MemeDock.Windows.ViewModels;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.Windows.Storage.Pickers;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.System;

namespace MemeDock.Windows.Views.Creator;

public sealed partial class CreatorView : UserControl
{
    private static readonly string[] VideoExtensions =
        [".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi", ".ts", ".m2ts"];

    private CreatorViewModel? _model;
    private StickerOutputViewModel? _output;
    private LocalizationService? _strings;
    private WindowsShareService? _share;
    private WindowId _windowId;
    private Func<string?>? _collectionProvider;
    private CreatorMediaPlayer? _player;
    private string? _fallbackImagePath;
    private bool _updatingSelector;
    private bool _windowActive = true;
    private bool _visible;
    private bool _narrowLayout;
    private bool _shareBusy;

    public event EventHandler<(string Message, InfoBarSeverity Severity)>? NoticeRequested;

    public CreatorView()
    {
        InitializeComponent();
        Motion.UseCrossFade(EditStage);
        Motion.UseCrossFade(ResultView);

        CropOverlay.CropChanged += (_, crop) => _model?.SetCrop(crop);
        CropOverlay.AspectUnlocked += (_, _) => { if (_model is not null) _model.AspectLock = null; };
        CropOverlay.TappedForPlayback += (_, _) => { if (_player?.IsAvailable == true) _player.Toggle(); };
        CropOverlay.ResetRequested += (_, _) => _model?.ResetCrop();

        Timeline.PlayPauseRequested += (_, _) => _player?.Toggle();
        Timeline.InteractionStarted += (_, _) => _player?.Pause();
        Timeline.ScrubRequested += (_, seconds) => _player?.Seek(seconds);
        Timeline.StartChanged += (_, seconds) => _model?.SetStart(seconds);
        Timeline.EndChanged += (_, seconds) => _model?.SetEnd(seconds);
        Timeline.RangeMoved += (_, newStart) => _model?.MoveRange(newStart);
        Timeline.SetStartFromPlayheadRequested += (_, _) => { if (_player is not null) _model?.SetStartFromPlayhead(_player.Position); };
        Timeline.SetEndFromPlayheadRequested += (_, _) => { if (_player is not null) _model?.SetEndFromPlayhead(_player.Position); };

        OutputSettings.GenerateRequested += async (_, _) => await GenerateAsync();
        OutputSettings.CancelRequested += (_, _) => _model?.CancelWork();

        ResultView.RegenerateRequested += async (_, _) => await GenerateAsync();
        ResultView.AddToDockRequested += async (_, _) => await AddToDockAsync();
        ResultView.CopyRequested += async (_, _) => await CopyResultAsync();
        ResultView.SaveRequested += async (_, _) => await SaveResultAsync();
        ResultView.ShareRequested += async (_, _) => await ShareResultAsync();
    }

    internal void Configure(
        CreatorViewModel? model,
        StickerOutputViewModel? output,
        LocalizationService strings,
        WindowsShareService share,
        WindowId windowId,
        Func<string?> collectionProvider)
    {
        DetachModel();
        _model = model;
        _output = output;
        _strings = strings;
        _share = share;
        _windowId = windowId;
        _collectionProvider = collectionProvider;

        if (_player is null)
        {
            _player = new CreatorMediaPlayer(DispatcherQueue);
            _player.PositionChanged += OnPlayerPositionChanged;
            _player.StateChanged += OnPlayerStateChanged;
            VideoPlayerElement.SetMediaPlayer(_player.Player);
        }

        if (_model is not null)
        {
            _model.PropertyChanged += OnModelChanged;
            _model.VideoReleasing += OnVideoReleasing;
            _model.VideoOpened += OnVideoOpened;
            _model.RangeAdjusted += OnRangeAdjusted;
            _model.AddedToDock += OnAddedToDock;
        }

        OutputSettings.Configure(_model, strings);
        ResultView.Configure(_model, strings);
        Relocalize();
        Refresh();
    }

    public void Relocalize()
    {
        if (_strings is null) return;
        EmptyTitle.Text = _strings["creator_title"];
        EmptyHint.Text = _strings["creator_empty_hint"];
        ChooseVideoLabel.Text = _strings["creator_choose"];
        EmptyDropHint.Text = _strings["creator_drop_hint"];
        EmptyCancelButton.Content = _strings["cancel"];
        SdrHint.Text = _strings["creator_sdr_hint"];
        ChangeVideoButton.Content = _strings["creator_change"];
        EditModeTab.Text = _strings["creator_edit"];
        ResultModeTab.Text = _strings["creator_result"];
        FramePreviewLabel.Text = _strings["creator_frame_preview"];
        CropHintLabel.Text = _strings["creator_crop_hint"];
        DropOverlayLabel.Text = _strings["creator_drop_hint"];

        CropOverlay.Relocalize(_strings);
        Timeline.Relocalize(_strings);
        OutputSettings.Relocalize();
        ResultView.Relocalize();
        Refresh();
    }

    public void SetActive(bool windowActive, bool visible)
    {
        _windowActive = windowActive;
        _visible = visible;
        var active = windowActive && visible;
        if (!active) _player?.Pause();
        ResultView.SetActive(active && _model?.ShowingResult == true);
    }

    public void Shutdown()
    {
        DetachModel();
        ResultView.StopPlayback();
        if (_player is not null)
        {
            _player.PositionChanged -= OnPlayerPositionChanged;
            _player.StateChanged -= OnPlayerStateChanged;
            VideoPlayerElement.SetMediaPlayer(null);
            _player.Dispose();
            _player = null;
        }
    }

    private void DetachModel()
    {
        if (_model is null) return;
        _model.PropertyChanged -= OnModelChanged;
        _model.VideoReleasing -= OnVideoReleasing;
        _model.VideoOpened -= OnVideoOpened;
        _model.RangeAdjusted -= OnRangeAdjusted;
        _model.AddedToDock -= OnAddedToDock;
        _model = null;
    }

    private void OnModelChanged(object? sender, PropertyChangedEventArgs args)
    {
        DispatcherQueue?.TryEnqueue(Refresh);
    }

    private void OnVideoReleasing(object? sender, EventArgs args)
    {
        _player?.Clear();
        _fallbackImagePath = null;
        FallbackPreviewImage.Source = null;
    }

    private void OnVideoOpened(object? sender, ICreatorVideo video)
    {
        DispatcherQueue?.TryEnqueue(() =>
        {
            if (_model is null || !ReferenceEquals(_model.Source, video)) return;
            _player?.Load(video.Path, _model.Duration, _model.End);
            Refresh();
        });
    }

    private void OnRangeAdjusted(object? sender, (double Start, double End, double? SeekTo) args)
    {
        if (_player is null) return;
        _player.SetRange(args.Start, args.End);
        if (args.SeekTo is { } seek) _player.Seek(seek);
    }

    private void OnAddedToDock(object? sender, bool restoreRequired)
    {
        if (_strings is null) return;
        DispatcherQueue?.TryEnqueue(() =>
        {
            if (restoreRequired)
                NoticeRequested?.Invoke(this, (_strings["import_restore_required"], InfoBarSeverity.Warning));
            else
                NoticeRequested?.Invoke(this, (_strings["creator_added"], InfoBarSeverity.Success));
        });
    }

    private void OnPlayerPositionChanged(object? sender, EventArgs args)
    {
        if (_player is null || _model is null) return;
        Timeline.UpdatePlayhead(_player.Position, _player.IsPlaying, _player.IsAvailable);
        if (!_player.IsAvailable)
            _model.RequestFallbackPreview(_player.Position);
    }

    private void OnPlayerStateChanged(object? sender, EventArgs args)
    {
        if (_player is null || _model is null) return;
        UpdatePreviewSurface();
        Timeline.UpdatePlayhead(_player.Position, _player.IsPlaying, _player.IsAvailable);
        if (!_player.IsAvailable && _model.FallbackFramePath is null)
            _model.RequestFallbackPreview(_player.Position, debounceMs: 0);
    }

    private void Refresh()
    {
        if (_model is null || _strings is null) return;
        var hasVideo = _model.Source is not null;
        EmptyStateHost.Visibility = MainWindow.ToVisibility(!hasVideo);
        EditorHost.Visibility = MainWindow.ToVisibility(hasVideo);

        if (!hasVideo)
        {
            EmptyLoadingPanel.Visibility = MainWindow.ToVisibility(_model.IsBusy);
            EmptyActionPanel.Visibility = MainWindow.ToVisibility(!_model.IsBusy);
            EmptyProgress.IsActive = _model.IsBusy;
            EmptyLoadingLabel.Text = _model.StageText;
            EmptyErrorBar.IsOpen = _model.FailureKey is not null;
            EmptyErrorBar.Message = _model.FailureText;
            ResultView.StopPlayback();
            return;
        }

        FileNameLabel.Text = _model.FileName;
        ChangeVideoButton.IsEnabled = !_model.IsBusy && !_model.IsSaving;

        var hasResult = _model.Result is not null;
        ViewModeBar.Visibility = MainWindow.ToVisibility(hasResult);
        ResultModeTab.Icon = _model.IsStale
            ? new FontIcon
            {
                Glyph = "\uE7BA",
                FontSize = 12,
                Foreground = Application.Current.Resources.TryGetValue("SystemFillColorCautionBrush", out var b) ? b as Brush : null
            }
            : null;

        var showResult = _model.ShowingResult && hasResult;
        _updatingSelector = true;
        try
        {
            ViewModeBar.SelectedItem = showResult ? ResultModeTab : EditModeTab;
        }
        finally
        {
            _updatingSelector = false;
        }

        EditStage.Visibility = MainWindow.ToVisibility(!showResult);
        ResultView.Visibility = MainWindow.ToVisibility(showResult);
        if (showResult) _player?.Pause();
        ResultView.SetActive(_windowActive && _visible && showResult);

        StageErrorBar.IsOpen = _model.FailureKey is not null;
        StageErrorBar.Message = _model.FailureText;

        UpdatePreviewSurface();
        LayoutFittedVideo();

        var info = _model.Info;
        CropOverlay.Update(
            _model.Crop,
            _model.AspectLock,
            info?.Width ?? 1u,
            info?.Height ?? 1u,
            interactive: !_model.IsBusy && !_model.IsSaving);

        Timeline.UpdateState(
            _model.Duration,
            _model.Start,
            _model.End,
            _player?.Position ?? 0,
            _player?.IsPlaying == true,
            _player?.IsAvailable ?? true,
            interactive: !_model.IsBusy && !_model.IsSaving,
            _model.Thumbnails);

        OutputSettings.Refresh();
        ResultView.Refresh();
    }

    private void UpdatePreviewSurface()
    {
        var available = _player?.IsAvailable ?? true;
        VideoPlayerElement.Visibility = MainWindow.ToVisibility(available);
        FallbackPreviewImage.Visibility = MainWindow.ToVisibility(!available);
        FramePreviewBadge.Visibility = MainWindow.ToVisibility(!available);

        if (!available && _model?.FallbackFramePath is { } path && path != _fallbackImagePath)
        {
            _fallbackImagePath = path;
            FallbackPreviewImage.Source = new BitmapImage(new Uri(StickerOutputService.NormalizePath(path)));
        }
    }

    private void OnPreviewContainerSizeChanged(object sender, SizeChangedEventArgs args) => LayoutFittedVideo();

    private void LayoutFittedVideo()
    {
        var containerW = PreviewContainer.ActualWidth;
        var containerH = PreviewContainer.ActualHeight;
        if (containerW <= 1 || containerH <= 1) return;

        var videoW = Math.Max(1.0, _model?.Info?.Width ?? 1u);
        var videoH = Math.Max(1.0, _model?.Info?.Height ?? 1u);
        var aspect = videoW / videoH;
        var fitW = Math.Min(containerW, containerH * aspect);
        var fitH = fitW / aspect;
        FittedVideoHost.Width = Math.Max(1, fitW);
        FittedVideoHost.Height = Math.Max(1, fitH);
    }

    private void OnWorkspaceSizeChanged(object sender, SizeChangedEventArgs args)
    {
        var narrow = args.NewSize.Width < 760;
        if (narrow == _narrowLayout) return;
        _narrowLayout = narrow;

        if (narrow)
        {
            ContentScroll.VerticalScrollBarVisibility = ScrollBarVisibility.Auto;
            DividerColumn.Width = new GridLength(0);
            InspectorColumn.Width = new GridLength(0);
            VerticalDivider.Visibility = Visibility.Collapsed;
            HorizontalDivider.Visibility = Visibility.Visible;
            Grid.SetRow(OutputSettings, 2);
            Grid.SetColumn(OutputSettings, 0);
            Grid.SetColumnSpan(OutputSettings, 3);
            StageArea.MinHeight = 460;
        }
        else
        {
            ContentScroll.VerticalScrollBarVisibility = ScrollBarVisibility.Disabled;
            DividerColumn.Width = new GridLength(1);
            InspectorColumn.Width = new GridLength(284);
            VerticalDivider.Visibility = Visibility.Visible;
            HorizontalDivider.Visibility = Visibility.Collapsed;
            Grid.SetRow(OutputSettings, 0);
            Grid.SetColumn(OutputSettings, 2);
            Grid.SetColumnSpan(OutputSettings, 1);
            StageArea.MinHeight = 0;
        }
    }

    private async void OnChooseVideo(object sender, RoutedEventArgs args)
    {
        if (_model is null || _model.IsBusy || _model.IsSaving) return;
        try
        {
            var picker = new FileOpenPicker(_windowId);
            foreach (var ext in VideoExtensions) picker.FileTypeFilter.Add(ext);
            picker.FileTypeFilter.Add("*");
            var file = await picker.PickSingleFileAsync();
            if (file is null || string.IsNullOrWhiteSpace(file.Path)) return;
            await _model.OpenVideoAsync(file.Path);
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Choose creator video failed: {error}");
        }
    }

    private void OnCancel(object sender, RoutedEventArgs args) => _model?.CancelWork();

    private void OnViewModeChanged(SelectorBar sender, SelectorBarSelectionChangedEventArgs args)
    {
        if (_updatingSelector || _model is null) return;
        _model.ShowingResult = ReferenceEquals(sender.SelectedItem, ResultModeTab);
    }

    private async Task GenerateAsync()
    {
        if (_model is null) return;
        _player?.Pause();
        await _model.GenerateAsync();
    }

    private async Task AddToDockAsync()
    {
        if (_model is null) return;
        await _model.AddToDockAsync(_collectionProvider?.Invoke());
    }

    private async Task CopyResultAsync()
    {
        if (_model?.Result?.Artifact.Lease is not { } lease || _output is null || _model.IsStale || _strings is null) return;
        ResultView.SetActionBusy(true);
        try
        {
            var stem = Path.GetFileNameWithoutExtension(_model.FileName);
            await _output.CopyPreparedAsync(lease, stem);
        }
        finally
        {
            ResultView.SetActionBusy(false);
        }
    }

    private async Task SaveResultAsync()
    {
        if (_model?.Result?.Artifact.Lease is not { } lease || _output is null || _model.IsStale || _strings is null) return;
        ResultView.SetActionBusy(true);
        try
        {
            if (await _output.SavePreparedAsync(lease, _model.FileName, _windowId))
                NoticeRequested?.Invoke(this, (_strings["picker_saved"], InfoBarSeverity.Success));
        }
        finally
        {
            ResultView.SetActionBusy(false);
        }
    }

    private async Task ShareResultAsync()
    {
        if (_model is null || !_model.HasCurrentResult || _share is null || _strings is null || _shareBusy) return;
        _shareBusy = true;
        ResultView.SetActionBusy(true);
        try
        {
            var artifact = await _model.PrepareHandoffAsync(CancellationToken.None);
            var title = Path.GetFileNameWithoutExtension(_model.FileName);
            _share.ShareFile(artifact, title);
            NoticeRequested?.Invoke(this, (_strings["creator_share_launched"], InfoBarSeverity.Informational));
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Creator share failed: {error}");
            NoticeRequested?.Invoke(this, (_strings[CreatorMath.MapErrorKey(error, _strings)], InfoBarSeverity.Error));
        }
        finally
        {
            _shareBusy = false;
            ResultView.SetActionBusy(false);
        }
    }

    // ---- Drag and drop ----

    private void OnDragOver(object sender, DragEventArgs args)
    {
        var canDrop = _model is not null && !_model.IsBusy && !_model.IsSaving
            && args.DataView.Contains(StandardDataFormats.StorageItems)
            && !args.DataView.Contains(StickerImportService.InternalDragFormat)
            && (args.AllowedOperations & DataPackageOperation.Copy) != 0;

        args.AcceptedOperation = canDrop ? DataPackageOperation.Copy : DataPackageOperation.None;
        if (canDrop && _strings is not null)
            args.DragUIOverride.Caption = _strings["creator_drop_hint"];

        SetDropHighlight(canDrop);
        args.Handled = true;
    }

    private void OnDragLeave(object sender, DragEventArgs args) => SetDropHighlight(false);

    private async void OnDrop(object sender, DragEventArgs args)
    {
        SetDropHighlight(false);
        if (_model is null || _model.IsBusy || _model.IsSaving
            || !args.DataView.Contains(StandardDataFormats.StorageItems)
            || args.DataView.Contains(StickerImportService.InternalDragFormat)) return;

        var deferral = args.GetDeferral();
        try
        {
            var items = await args.DataView.GetStorageItemsAsync();
            var file = items.OfType<StorageFile>().FirstOrDefault();
            if (file is not null && !string.IsNullOrWhiteSpace(file.Path))
            {
                args.AcceptedOperation = DataPackageOperation.Copy;
                await _model.OpenVideoAsync(file.Path);
            }
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Creator drop failed: {error}");
        }
        finally
        {
            deferral.Complete();
        }
        args.Handled = true;
    }

    private void SetDropHighlight(bool active)
    {
        if (_model?.Source is not null)
        {
            DropOverlay.Visibility = MainWindow.ToVisibility(active);
            return;
        }
        DropOverlay.Visibility = Visibility.Collapsed;
        var accent = Application.Current.Resources.TryGetValue("AccentFillColorDefaultBrush", out var a) ? a as Brush : null;
        var normal = Application.Current.Resources.TryGetValue("CardStrokeColorDefaultBrush", out var n) ? n as Brush : null;
        if (accent is not null && normal is not null)
        {
            EmptyDashedBorder.Stroke = active ? accent : normal;
            EmptyDashedBorder.StrokeThickness = active ? 2.0 : 1.5;
        }
        EmptyIcon.Glyph = active ? "\uE896" : "\uE714";
    }

    // ---- Keyboard shortcuts ----

    private bool CanUseEditShortcuts =>
        _visible && _model is not null && _model.Source is not null
        && !_model.ShowingResult && !_model.IsBusy && !_model.IsSaving;

    private void OnCreatorKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (!CanUseEditShortcuts || _player is null) return;
        switch (args.Key)
        {
            case VirtualKey.Space:
                _player.Toggle();
                args.Handled = true;
                break;
            case VirtualKey.Left:
                _player.Step(-0.1);
                args.Handled = true;
                break;
            case VirtualKey.Right:
                _player.Step(0.1);
                args.Handled = true;
                break;
        }
    }

    private void OnCreatorCharacterReceived(UIElement sender, CharacterReceivedRoutedEventArgs args)
    {
        if (!CanUseEditShortcuts || _player is null || _model is null) return;
        if (args.Character == '[')
        {
            _model.SetStartFromPlayhead(_player.Position);
            args.Handled = true;
        }
        else if (args.Character == ']')
        {
            _model.SetEndFromPlayhead(_player.Position);
            args.Handled = true;
        }
    }

    private void OnUnloaded(object sender, RoutedEventArgs args) => _player?.Pause();
}
