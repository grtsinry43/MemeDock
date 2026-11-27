using System.Globalization;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Services;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace MemeDock.Windows.Views.Creator;

public sealed partial class CreatorResultView : UserControl
{
    private CreatorViewModel? _model;
    private LocalizationService? _strings;
    private WebpPlayback? _playback;
    private string? _loadedPath;
    private bool _active = true;
    private bool _actionBusy;

    public event EventHandler? RegenerateRequested;
    public event EventHandler? AddToDockRequested;
    public event EventHandler? CopyRequested;
    public event EventHandler? SaveRequested;
    public event EventHandler? ShareRequested;

    public CreatorResultView() => InitializeComponent();

    public void Configure(CreatorViewModel? model, LocalizationService strings)
    {
        _model = model;
        _strings = strings;
        Relocalize();
        Refresh();
    }

    public void Relocalize()
    {
        if (_strings is null) return;
        StaleBar.Message = _strings["creator_stale"];
        RegenerateButton.Content = _strings["creator_regenerate"];
        BrokenLabel.Text = _strings["preview_unavailable"];
        AddLabel.Text = _strings["creator_add_to_dock"];
        CopyLabel.Text = _strings["copy_image"];
        SaveLabel.Text = _strings["picker_save"];
        ShareLabel.Text = _strings["creator_share"];
        ResultHint.Text = _strings["creator_result_hint"];
        Refresh();
    }

    public void SetActive(bool active)
    {
        _active = active;
        _playback?.SetRunning(active);
    }

    public void SetActionBusy(bool busy)
    {
        _actionBusy = busy;
        Refresh();
    }

    public void Refresh()
    {
        if (_model is null || _strings is null)
        {
            StopPlayback();
            return;
        }

        var result = _model.Result;
        StaleBar.IsOpen = _model.IsStale && result is not null;
        RegenerateButton.IsEnabled = _model.CanGenerate;

        if (result is not null)
        {
            DimensionsText.Text = $"{result.Width} × {result.Height}";
            DurationText.Text = string.Format(CultureInfo.InvariantCulture, "{0:0.00} s", result.DurationMs / 1000.0);
            FpsText.Text = $"{(int)Math.Round(result.FrameRate)} FPS";
            SizeText.Text = ByteSize.Format((long)Math.Min(result.Artifact.ByteSize, long.MaxValue));
            MetadataPill.Visibility = Visibility.Visible;
            EnsurePlayback(result.Artifact.Path);
        }
        else
        {
            MetadataPill.Visibility = Visibility.Collapsed;
            StopPlayback();
        }

        var canExport = _model.HasCurrentResult && !_model.IsSaving && !_model.IsBusy && !_actionBusy;
        AddButton.IsEnabled = canExport;
        CopyButton.IsEnabled = canExport;
        SaveButton.IsEnabled = canExport;
        ShareButton.IsEnabled = canExport;
        SavingRing.IsActive = _model.IsSaving;
        SavingRing.Visibility = MainWindow.ToVisibility(_model.IsSaving);
        AddIcon.Visibility = MainWindow.ToVisibility(!_model.IsSaving);
    }

    private void EnsurePlayback(string path)
    {
        if (_loadedPath == path && _playback is not null)
        {
            _playback.SetRunning(_active);
            return;
        }

        StopPlayback();
        _loadedPath = path;
        BrokenState.Visibility = Visibility.Collapsed;
        var playback = new WebpPlayback(StickerOutputService.NormalizePath(path), DispatcherQueue);
        playback.Failed += _ =>
        {
            if (ReferenceEquals(_playback, playback))
                BrokenState.Visibility = Visibility.Visible;
        };
        _playback = playback;
        PreviewImage.Source = playback.Source;
        playback.SetRunning(_active);
    }

    public void StopPlayback()
    {
        _loadedPath = null;
        PreviewImage.Source = null;
        BrokenState.Visibility = Visibility.Collapsed;
        _playback?.Dispose();
        _playback = null;
    }

    private void OnRegenerate(object sender, RoutedEventArgs args) => RegenerateRequested?.Invoke(this, EventArgs.Empty);
    private void OnAddToDock(object sender, RoutedEventArgs args) => AddToDockRequested?.Invoke(this, EventArgs.Empty);
    private void OnCopy(object sender, RoutedEventArgs args) => CopyRequested?.Invoke(this, EventArgs.Empty);
    private void OnSave(object sender, RoutedEventArgs args) => SaveRequested?.Invoke(this, EventArgs.Empty);
    private void OnShare(object sender, RoutedEventArgs args) => ShareRequested?.Invoke(this, EventArgs.Empty);
    private void OnUnloaded(object sender, RoutedEventArgs args) => StopPlayback();
}
