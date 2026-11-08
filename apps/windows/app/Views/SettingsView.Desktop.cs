using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace MemeDock.Windows.Views;

public sealed partial class SettingsView
{
    private DesktopIntegration? _desktop;
    private Task _desktopSaving = Task.CompletedTask;
    private ContentDialog? _shortcutDialog;
    internal void ConfigureDesktop(DesktopIntegration desktop)
    {
        _desktop = desktop;
        desktop.SettingsChanged += RelocalizeDesktop;
        RelocalizeDesktop();
    }
    private void RelocalizeDesktop()
    {
        if (_strings is null || _desktop is null || _closed) return;
        _updating = true;
        try
        {
            var preferences = _desktop.Preferences.Desktop;
            DesktopSectionHeader.Text = _strings["desktop_integration"];
            ResidentGroupLabel.Text = _strings["desktop_resident_group"];
            PickerGroupLabel.Text = _strings["picker_title"];
            TaskbarLabel.Text = _strings["desktop_show_taskbar"]; StartupLabel.Text = _strings["desktop_launch_login"];
            AnchorLabel.Text = _strings["picker_anchor"]; ShortcutLabel.Text = _strings["picker_shortcut"];
            ShowTaskbar.IsOn = preferences.ShowTaskbarIcon; AnchorInput.IsOn = preferences.AnchorToInput;
            try { LaunchAtLogin.IsOn = LoginStartup.Enabled; }
            catch (Exception) { DesktopError.Message = _strings["desktop_startup_failed"]; DesktopError.IsOpen = true; }
            ResidentHint.Text = _strings["desktop_resident_hint"]; AnchorHint.Text = _strings["picker_anchor_hint"];
            PasteHint.Text = _strings["picker_paste_hint"]; ShortcutPreview.SetShortcut(preferences.Shortcut);
            ShortcutPlaceholder.Text = _strings["picker_record_shortcut"];
            ShortcutPlaceholder.Visibility = preferences.Shortcut is null ? Visibility.Visible : Visibility.Collapsed;
            ClearShortcut.Content = _strings["picker_clear_shortcut"];
            StartupSettings.Content = _strings["desktop_startup_settings"];
            foreach (var (control, key) in new (DependencyObject, string)[] { (ShowTaskbar, "desktop_show_taskbar"), (LaunchAtLogin, "desktop_launch_login"), (AnchorInput, "picker_anchor"), (ShortcutRecorderButton, "picker_shortcut") })
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(control, _strings[key]);
            if (_desktop.ShortcutError is { } error) { DesktopError.Message = _strings[error]; DesktopError.IsOpen = true; }
        }
        finally { _updating = false; }
    }
    private async void OnDesktopToggle(object sender, RoutedEventArgs args)
    {
        if (_updating || _desktop is null || _closed) return;
        _desktopSaving = SaveDesktopAsync(_desktop.Preferences.Desktop with { ShowTaskbarIcon = ShowTaskbar.IsOn, AnchorToInput = AnchorInput.IsOn });
        await _desktopSaving;
    }
    private async Task SaveDesktopAsync(DesktopPreferences preferences)
    {
        if (_desktop is null || _strings is null) return;
        SetDesktopEnabled(false); DesktopError.IsOpen = false;
        try { await _desktop.SavePreferencesAsync(preferences); }
        catch (Exception error)
        {
            DesktopError.Message = _strings[error is System.ComponentModel.Win32Exception ? "picker_shortcut_conflict" : "desktop_preferences_failed"];
            DesktopError.IsOpen = true;
        }
        finally { RelocalizeDesktop(); SetDesktopEnabled(true); }
    }
    private void OnStartupToggle(object sender, RoutedEventArgs args)
    {
        if (_updating || _closed || _strings is null) return;
        DesktopError.IsOpen = false;
        try { LoginStartup.SetEnabled(LaunchAtLogin.IsOn); }
        catch (Exception) { DesktopError.Message = _strings["desktop_startup_failed"]; DesktopError.IsOpen = true; }
        RelocalizeDesktop();
    }
    private async void OnEditShortcut(object sender, RoutedEventArgs args)
    {
        if (_desktop is null || _strings is null || _closed || _shortcutDialog is not null || _confirming) return;
        _desktopSaving = EditShortcutAsync();
        await _desktopSaving;
    }
    private async Task EditShortcutAsync()
    {
        if (_desktop is null || _strings is null) return;
        var keys = new ShortcutKeys();
        var placeholder = new TextBlock { Text = _strings["picker_record_shortcut"], VerticalAlignment = VerticalAlignment.Center };
        var captureContent = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        captureContent.Children.Add(keys); captureContent.Children.Add(placeholder);
        var capture = new Button { Content = captureContent, MinHeight = 56, HorizontalAlignment = HorizontalAlignment.Stretch };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(capture, _strings["picker_record_shortcut"]);
        var error = new InfoBar { IsOpen = false, IsClosable = false, Severity = InfoBarSeverity.Error };
        var content = new StackPanel { Spacing = 16 };
        content.Children.Add(new TextBlock { Text = _strings["picker_shortcut_record_hint"], TextWrapping = TextWrapping.Wrap });
        content.Children.Add(capture); content.Children.Add(error);
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot, Title = _strings["picker_shortcut"], Content = content,
            PrimaryButtonText = _strings["picker_shortcut_save"], CloseButtonText = _strings["cancel"],
            DefaultButton = ContentDialogButton.None, IsPrimaryButtonEnabled = false
        };
        _shortcutDialog = dialog;
        ShortcutRecorder? recorder = null;
        void Changed() => DispatcherQueue.TryEnqueue(() =>
        {
            if (recorder is null || _shortcutDialog != dialog) return;
            var candidate = recorder.Capture.Candidate;
            keys.SetKeys(candidate?.Keys ?? new Models.GlobalShortcut(recorder.Capture.Modifiers, 0).Keys);
            placeholder.Visibility = candidate is null && recorder.Capture.Modifiers == 0 ? Visibility.Visible : Visibility.Collapsed;
            dialog.IsPrimaryButtonEnabled = candidate is not null;
            error.IsOpen = false;
        });
        capture.GotFocus += (_, _) => { recorder?.Begin(); Changed(); };
        capture.Click += (_, _) => capture.Focus(FocusState.Programmatic);
        dialog.Opened += (_, _) =>
        {
            try
            {
                recorder = new ShortcutRecorder(() => _desktop.IsMainForeground && capture.FocusState != FocusState.Unfocused, Changed);
                recorder.Begin(); capture.Focus(FocusState.Programmatic);
            }
            catch (Exception)
            {
                error.Message = _strings["picker_shortcut_record_failed"]; error.IsOpen = true;
            }
        };
        dialog.Closed += (_, _) => recorder?.Dispose();
        dialog.PrimaryButtonClick += async (_, args) =>
        {
            if (recorder?.Capture.Candidate is not { } candidate) { args.Cancel = true; return; }
            var deferral = args.GetDeferral();
            dialog.IsPrimaryButtonEnabled = false;
            try { await _desktop.SavePreferencesAsync(_desktop.Preferences.Desktop with { Shortcut = candidate }); }
            catch (Exception failure)
            {
                args.Cancel = true;
                error.Message = _strings[failure is System.ComponentModel.Win32Exception ? "picker_shortcut_conflict" : "desktop_preferences_failed"];
                error.IsOpen = true;
            }
            finally { dialog.IsPrimaryButtonEnabled = true; deferral.Complete(); }
        };
        try { await dialog.ShowAsync(); }
        finally { recorder?.Dispose(); _shortcutDialog = null; RelocalizeDesktop(); }
    }
    private async void OnClearShortcut(object sender, RoutedEventArgs args)
    {
        if (_desktop is null) return;
        _desktopSaving = SaveDesktopAsync(_desktop.Preferences.Desktop with { Shortcut = null });
        await _desktopSaving;
    }
    private void SetDesktopEnabled(bool enabled)
    {
        foreach (var control in new Control[] { ShowTaskbar, LaunchAtLogin, AnchorInput, ShortcutRecorderButton, ClearShortcut }) control.IsEnabled = enabled;
    }
}
