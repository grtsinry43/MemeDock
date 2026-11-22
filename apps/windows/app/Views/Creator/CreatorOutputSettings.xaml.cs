using System.Globalization;
using MemeDock.Windows.Localization;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace MemeDock.Windows.Views.Creator;

public sealed partial class CreatorOutputSettings : UserControl
{
    private CreatorViewModel? _model;
    private LocalizationService? _strings;
    private bool _updating;

    public event EventHandler? GenerateRequested;
    public event EventHandler? CancelRequested;

    public CreatorOutputSettings() => InitializeComponent();

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
        CropHeader.Text = _strings["creator_crop"];
        ResetCropButton.Content = _strings["creator_reset_crop"];
        AspectFull.Content = _strings["creator_full_frame"];
        OutputHeader.Text = _strings["creator_output"];
        LongestEdgeLabel.Text = _strings["creator_longest_edge"];
        FrameRateLabel.Text = _strings["creator_frame_rate"];
        QualityLabel.Text = _strings["creator_quality"];
        AutomationProperties.SetName(QualitySlider, _strings["creator_quality"]);
        WebpHint.Text = _strings["creator_webp_hint"];
        CancelButton.Content = _strings["cancel"];
        Refresh();
    }

    public void Refresh()
    {
        if (_model is null || _strings is null) return;
        _updating = true;
        try
        {
            var interactive = !_model.IsBusy && !_model.IsSaving;
            SettingsBody.IsHitTestVisible = interactive;
            SettingsBody.Opacity = interactive ? 1.0 : 0.6;

            ResetCropButton.Visibility = MainWindow.ToVisibility(_model.IsCropped);
            ResetCropButton.IsEnabled = interactive;

            AspectFull.IsChecked = !_model.IsCropped;
            Aspect1x1.IsChecked = IsAspectSelected(1.0);
            Aspect4x3.IsChecked = IsAspectSelected(4.0 / 3.0);
            Aspect16x9.IsChecked = IsAspectSelected(16.0 / 9.0);
            Aspect9x16.IsChecked = IsAspectSelected(9.0 / 16.0);

            if (_model.CroppedPixelSize is { } cropped)
            {
                var freeSuffix = _model.AspectLock is null && _model.IsCropped
                    ? $" · {_strings["creator_free_crop"]}"
                    : "";
                CroppedSizeText.Text = $"{cropped.Width} × {cropped.Height} px{freeSuffix}";
                CroppedSizeText.Visibility = Visibility.Visible;
            }
            else
            {
                CroppedSizeText.Visibility = Visibility.Collapsed;
            }

            Edge128.IsChecked = _model.MaxEdge == 128;
            Edge256.IsChecked = _model.MaxEdge == 256;
            Edge384.IsChecked = _model.MaxEdge == 384;
            Edge512.IsChecked = _model.MaxEdge == 512;

            Fps10.IsChecked = _model.Fps == 10;
            Fps15.IsChecked = _model.Fps == 15;
            Fps20.IsChecked = _model.Fps == 20;
            Fps24.IsChecked = _model.Fps == 24;
            Fps30.IsChecked = _model.Fps == 30;

            QualitySlider.Value = _model.Quality;
            QualityValue.Text = _model.Quality.ToString(CultureInfo.InvariantCulture);

            if (_model.EstimatedOutputSize is { } estimated)
            {
                EstimateText.Text = string.Format(
                    CultureInfo.InvariantCulture,
                    "{0} × {1} · {2} FPS · {3:0.00}s",
                    estimated.Width,
                    estimated.Height,
                    _model.Fps,
                    Math.Max(0, _model.End - _model.Start));
                EstimateCard.Visibility = Visibility.Visible;
            }
            else
            {
                EstimateCard.Visibility = Visibility.Collapsed;
            }

            GenerateButton.IsEnabled = _model.CanGenerate;
            BusyRing.IsActive = _model.IsBusy;
            BusyRing.Visibility = MainWindow.ToVisibility(_model.IsBusy);
            GenerateIcon.Visibility = MainWindow.ToVisibility(!_model.IsBusy);
            GenerateLabel.Text = _model.IsBusy
                ? _model.StageText
                : _strings[_model.IsStale ? "creator_regenerate" : "creator_generate"];
            CancelButton.Visibility = MainWindow.ToVisibility(_model.IsBusy);
        }
        finally
        {
            _updating = false;
        }
    }

    private bool IsAspectSelected(double target)
    {
        if (_model is null || !_model.IsCropped) return false;
        if (_model.AspectLock is { } locked) return Math.Abs(locked - target) < 0.001;
        if (_model.Info is not { } info || info.Height == 0 || _model.Crop.Height <= 0) return false;
        var ratio = (info.Width * _model.Crop.Width) / (info.Height * _model.Crop.Height);
        return Math.Abs(ratio - target) < 0.005;
    }

    private void OnResetCrop(object sender, RoutedEventArgs args) => _model?.ResetCrop();

    private void OnAspectClick(object sender, RoutedEventArgs args)
    {
        if (_updating || _model is null || sender is not ToggleButton { Tag: string tag }) return;
        double? aspect = tag switch
        {
            "1:1" => 1.0,
            "4:3" => 4.0 / 3.0,
            "16:9" => 16.0 / 9.0,
            "9:16" => 9.0 / 16.0,
            _ => null
        };
        _model.ApplyAspect(aspect);
        Refresh();
    }

    private void OnEdgeClick(object sender, RoutedEventArgs args)
    {
        if (_updating || _model is null || sender is not ToggleButton { Tag: string tag }
            || !int.TryParse(tag, out var edge)) return;
        _model.MaxEdge = edge;
        Refresh();
    }

    private void OnFpsClick(object sender, RoutedEventArgs args)
    {
        if (_updating || _model is null || sender is not ToggleButton { Tag: string tag }
            || !int.TryParse(tag, out var fps)) return;
        _model.Fps = fps;
        Refresh();
    }

    private void OnQualityChanged(object sender, RangeBaseValueChangedEventArgs args)
    {
        var quality = (int)Math.Round(args.NewValue);
        if (QualityValue is not null) QualityValue.Text = quality.ToString(CultureInfo.InvariantCulture);
        if (_updating || _model is null) return;
        _model.Quality = quality;
    }

    private void OnGenerate(object sender, RoutedEventArgs args) => GenerateRequested?.Invoke(this, EventArgs.Empty);
    private void OnCancel(object sender, RoutedEventArgs args) => CancelRequested?.Invoke(this, EventArgs.Empty);
}
