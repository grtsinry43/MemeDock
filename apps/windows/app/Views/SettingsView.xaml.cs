using System.Diagnostics;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;

namespace MemeDock.Windows.Views;

public sealed partial class SettingsView : UserControl
{
    private LocalizationService? _strings;
    private AppPreferences? _preferences;
    private bool _updating;
    private Task _saving = Task.CompletedTask;
    public Task PendingSave => _saving;

    /// <summary>
    /// Raised before the change is written so the window can repaint immediately;
    /// a failed write raises it again with the previous value.
    /// </summary>
    public event Action<AppearanceMode>? AppearanceChanged;

    public SettingsView() => InitializeComponent();

    public void Configure(LocalizationService strings, AppPreferences preferences)
    {
        _strings = strings;
        _preferences = preferences;
        Relocalize();
    }

    public void ConfigureSync(SyncCoordinator? sync, LocalizationService strings)
    {
        SyncPanel.Configure(sync, strings);
        _strings = strings;
        Relocalize();
    }

    /// <summary>
    /// The identity is read from the open library, so it arrives after the page was
    /// first localized. Until then the values stay empty and copying is disabled.
    /// </summary>
    public void ShowIdentity(LibraryIdentity? identity)
    {
        var library = identity?.LibraryId ?? "";
        var device = identity?.DeviceId ?? "";
        LibraryIdValue.Text = library.Length > 0 ? library : AppVersion.Unknown;
        DeviceIdValue.Text = device.Length > 0 ? device : AppVersion.Unknown;
        CopyLibraryId.IsEnabled = library.Length > 0;
        CopyDeviceId.IsEnabled = device.Length > 0;
    }

    public void ShutdownSync() => SyncPanel.Shutdown();

    public void Relocalize()
    {
        if (_strings is null || _preferences is null) return;
        _updating = true;
        try
        {
            Heading.Text = _strings["desktop_settings"];

            AppearanceLabel.Text = _strings["settings_appearance"];
            AutomationProperties.SetName(AppearanceSelector, _strings["settings_appearance"]);
            SystemAppearance.Content = _strings["theme_system"];
            LightAppearance.Content = _strings["theme_light"];
            DarkAppearance.Content = _strings["theme_dark"];
            AppearanceSelector.SelectedIndex = (int)_preferences.Appearance;
            AppearanceError.Message = _strings["settings_save_failed"];

            LanguageLabel.Text = _strings["settings_language"];
            AutomationProperties.SetName(LanguageSelector, _strings["settings_language"]);
            SystemLanguage.Content = _strings["language_system"];
            ChineseLanguage.Content = _strings["language_chinese"];
            EnglishLanguage.Content = _strings["language_english"];
            LanguageSelector.SelectedIndex = (int)_preferences.Language;
            SaveError.Message = _strings["settings_language_save_failed"];

            OutputLabel.Text = _strings["settings_output"];
            CopyGroupLabel.Text = _strings["output_copy_group"];
            DragGroupLabel.Text = _strings["output_drag_group"];
            CopyStillLabel.Text = _strings["output_still"];
            CopyAnimatedLabel.Text = _strings["output_animated"];
            DragStillLabel.Text = _strings["output_still"];
            DragAnimatedLabel.Text = _strings["output_animated"];
            AutomationProperties.SetName(CopyStillSelector, _strings["output_copy_group"] + " " + _strings["output_still"]);
            AutomationProperties.SetName(CopyAnimatedSelector, _strings["output_copy_group"] + " " + _strings["output_animated"]);
            AutomationProperties.SetName(DragStillSelector, _strings["output_drag_group"] + " " + _strings["output_still"]);
            AutomationProperties.SetName(DragAnimatedSelector, _strings["output_drag_group"] + " " + _strings["output_animated"]);
            FillPresets(CopyStillSelector, animated: false, _preferences.CopyOutput.Still);
            FillPresets(CopyAnimatedSelector, animated: true, _preferences.CopyOutput.Animated);
            FillPresets(DragStillSelector, animated: false, _preferences.DragOutput.Still);
            FillPresets(DragAnimatedSelector, animated: true, _preferences.DragOutput.Animated);
            OutputError.Message = _strings["settings_output_failed"];

            AboutLabel.Text = _strings["settings_about"];
            VersionLabel.Text = _strings["settings_version"];
            VersionValue.Text = AppVersion.FromAssembly(typeof(SettingsView).Assembly);
            LibraryIdLabel.Text = _strings["settings_library_id"];
            DeviceIdLabel.Text = _strings["settings_device_id"];
            LicensesLabel.Text = _strings["settings_licenses"];
            LicenseValue.Text = _strings["settings_license_text"];
            CopyLibraryId.Content = _strings["about_copy"];
            CopyDeviceId.Content = _strings["about_copy"];

            SyncPanel.Relocalize();
        }
        finally
        {
            _updating = false;
            UpdateHints();
        }
    }

    private void FillPresets(ComboBox box, bool animated, ExportPreset selected)
    {
        box.Items.Clear();
        var index = 0;
        var selectedIndex = 0;
        foreach (var preset in StickerOutputPresets.For(animated))
        {
            box.Items.Add(new ComboBoxItem { Content = _strings![StickerOutputPresets.LabelKey(preset)], Tag = preset });
            if (preset == selected) selectedIndex = index;
            index++;
        }
        box.SelectedIndex = selectedIndex;
    }

    /// <summary>
    /// Restores a selector after a failed write without re-entering the save path.
    /// </summary>
    private void SelectPreset(ComboBox box, ExportPreset preset)
    {
        for (var index = 0; index < box.Items.Count; index++)
        {
            if (box.Items[index] is ComboBoxItem { Tag: ExportPreset candidate } && candidate == preset)
            {
                box.SelectedIndex = index;
                return;
            }
        }
    }

    private void UpdateHints()
    {
        if (_strings is null) return;
        SetHint(CopyStillSelector, CopyStillHint, animated: false);
        SetHint(CopyAnimatedSelector, CopyAnimatedHint, animated: true);
        SetHint(DragStillSelector, DragStillHint, animated: false);
        SetHint(DragAnimatedSelector, DragAnimatedHint, animated: true);
    }

    private void SetHint(ComboBox box, TextBlock hint, bool animated)
    {
        if (box.SelectedItem is not ComboBoxItem { Tag: ExportPreset preset })
        {
            hint.Text = "";
            return;
        }
        hint.Text = _strings![StickerOutputPresets.HintKey(preset, animated)];
    }

    private async void OnAppearanceChanged(object sender, SelectionChangedEventArgs args)
    {
        if (_updating || _strings is null || _preferences is null
            || AppearanceSelector.SelectedItem is not ComboBoxItem { Tag: string tag }) return;
        _saving = SaveAppearanceAsync(Enum.Parse<AppearanceMode>(tag));
        await _saving;
    }

    private async Task SaveAppearanceAsync(AppearanceMode mode)
    {
        AppearanceSelector.IsEnabled = false;
        AppearanceError.IsOpen = false;
        var previous = _preferences!.Appearance;
        try
        {
            // Repaint first so the choice feels immediate; the persisted value is
            // still the one that survives a restart, and a failure reverts here.
            AppearanceChanged?.Invoke(mode);
            await _preferences.SaveAppearanceAsync(mode);
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Appearance could not be saved: {error}");
            AppearanceChanged?.Invoke(previous);
            Relocalize();
            AppearanceError.IsOpen = true;
        }
        finally { AppearanceSelector.IsEnabled = true; }
    }

    private async void OnLanguageChanged(object sender, SelectionChangedEventArgs args)
    {
        if (_updating || _strings is null || _preferences is null
            || LanguageSelector.SelectedItem is not ComboBoxItem { Tag: string tag }) return;
        _saving = SaveLanguageAsync(Enum.Parse<LanguageMode>(tag));
        await _saving;
    }

    private async Task SaveLanguageAsync(LanguageMode mode)
    {
        LanguageSelector.IsEnabled = false;
        SaveError.IsOpen = false;
        try
        {
            await _preferences!.SaveLanguageAsync(mode);
            _strings!.SetLanguage(mode);
        }
        catch (Exception error)
        {
            Trace.WriteLine(error);
            Relocalize();
            SaveError.IsOpen = true;
        }
        finally { LanguageSelector.IsEnabled = true; }
    }

    private async void OnOutputPresetChanged(object sender, SelectionChangedEventArgs args)
    {
        if (_updating || _preferences is null || sender is not ComboBox box
            || box.SelectedItem is not ComboBoxItem { Tag: ExportPreset preset }) return;
        var (action, animated) = Describe(box);
        _saving = SaveOutputAsync(action, animated, preset, box);
        await _saving;
    }

    private (OutputAction Action, bool Animated) Describe(ComboBox box) => ReferenceEquals(box, CopyStillSelector)
        ? (OutputAction.Copy, false)
        : ReferenceEquals(box, CopyAnimatedSelector) ? (OutputAction.Copy, true)
        : ReferenceEquals(box, DragStillSelector) ? (OutputAction.Drag, false)
        : (OutputAction.Drag, true);

    private async Task SaveOutputAsync(OutputAction action, bool animated, ExportPreset preset, ComboBox box)
    {
        box.IsEnabled = false;
        OutputError.IsOpen = false;
        var previous = (action == OutputAction.Copy ? _preferences!.CopyOutput : _preferences!.DragOutput).For(animated);
        try
        {
            await _preferences!.SaveOutputPresetAsync(action, animated, preset);
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Output preset could not be saved: {error}");
            _updating = true;
            try { SelectPreset(box, previous); }
            finally { _updating = false; }
            OutputError.IsOpen = true;
        }
        finally
        {
            box.IsEnabled = true;
            UpdateHints();
        }
    }

    private void OnCopyLibraryId(object sender, RoutedEventArgs args) => CopyIdentity(LibraryIdValue.Text);

    private void OnCopyDeviceId(object sender, RoutedEventArgs args) => CopyIdentity(DeviceIdValue.Text);

    /// <summary>
    /// Library and device IDs are stable identity metadata. They go to the clipboard
    /// on request but must never be written to logs by this app.
    /// </summary>
    private void CopyIdentity(string value)
    {
        if (_strings is null || value.Length == 0 || value == AppVersion.Unknown) return;
        try
        {
            var package = new DataPackage();
            package.SetText(value);
            Clipboard.SetContent(package);
            // Flush keeps the value after this window closes; it is the documented
            // way for a desktop app to hand clipboard content to other processes.
            Clipboard.Flush();
            AboutFeedback.Severity = InfoBarSeverity.Success;
            AboutFeedback.Message = _strings["image_copied"];
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Identity could not be copied: {error.GetType().Name}");
            AboutFeedback.Severity = InfoBarSeverity.Error;
            AboutFeedback.Message = _strings["about_copy_failed"];
        }
        AboutFeedback.IsOpen = true;
    }
}
