using MemeDock.Windows.Localization;
using MemeDock.Windows.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace MemeDock.Windows.Views;

public sealed partial class BrandModeSwitcher : UserControl
{
    private LocalizationService? _strings;
    private bool _isCreator;

    public event EventHandler<bool>? ModeSelected;

    public BrandModeSwitcher() => InitializeComponent();

    public bool IsCreator
    {
        get => _isCreator;
        set
        {
            if (_isCreator == value) return;
            var goingToCreator = value;
            _isCreator = value;
            UpdateSelectionVisuals(animate: true, goingToCreator: goingToCreator);
        }
    }

    public void Configure(LocalizationService strings)
    {
        _strings = strings;
        Relocalize();
        UpdateSelectionVisuals(animate: false, goingToCreator: _isCreator);
    }

    public void Relocalize()
    {
        if (_strings is null) return;
        DockSubtitle.Text = _strings["mode_dock_subtitle"];
        CreatorSubtitle.Text = _strings["mode_creator_subtitle"];
        ToolTipService.SetToolTip(TriggerButton, _strings["creator_switch_mode"]);
        AutomationProperties.SetName(TriggerButton, _isCreator ? "Meme Creator" : "MemeDock");
    }

    public void SetModeEnabled(bool enabled) => TriggerButton.IsEnabled = enabled;

    private void UpdateSelectionVisuals(bool animate, bool goingToCreator)
    {
        ModeSuffix.Text = _isCreator ? " Creator" : "Dock";
        if (animate) Motion.AnimateContentSwap(ModeSuffix, goingToCreator ? 8f : -8f);

        DockCheck.Visibility = MainWindow.ToVisibility(!_isCreator);
        CreatorCheck.Visibility = MainWindow.ToVisibility(_isCreator);

        var accentBg = Application.Current.Resources.TryGetValue("AccentFillColorDefaultBrush", out var a) ? a as Brush : null;
        var accentFg = Application.Current.Resources.TryGetValue("TextOnAccentFillColorPrimaryBrush", out var f) ? f as Brush : null;
        var subtleBg = Application.Current.Resources.TryGetValue("ControlFillColorSecondaryBrush", out var s) ? s as Brush : null;
        var normalFg = Application.Current.Resources.TryGetValue("TextFillColorPrimaryBrush", out var n) ? n as Brush : null;

        if (accentBg is not null && accentFg is not null && subtleBg is not null && normalFg is not null)
        {
            DockIconBadge.Background = _isCreator ? subtleBg : accentBg;
            DockIconGlyph.Foreground = _isCreator ? normalFg : accentFg;
            CreatorIconBadge.Background = _isCreator ? accentBg : subtleBg;
            CreatorIconGlyph.Foreground = _isCreator ? accentFg : normalFg;
        }

        AutomationProperties.SetName(TriggerButton, _isCreator ? "Meme Creator" : "MemeDock");
    }

    private void OnFlyoutOpening(object? sender, object args) => Motion.AnimateRotation(Chevron, 180f);
    private void OnFlyoutClosed(object? sender, object args) => Motion.AnimateRotation(Chevron, 0f);

    private void OnSelectDock(object sender, RoutedEventArgs args)
    {
        ModeFlyout.Hide();
        if (_isCreator) ModeSelected?.Invoke(this, false);
    }

    private void OnSelectCreator(object sender, RoutedEventArgs args)
    {
        ModeFlyout.Hide();
        if (!_isCreator) ModeSelected?.Invoke(this, true);
    }
}
