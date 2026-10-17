using System.Diagnostics;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Services;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace MemeDock.Windows.Views;

public sealed partial class SettingsView : UserControl
{
    private LocalizationService? _strings;
    private AppPreferences? _preferences;
    private bool _updating;
    private Task _saving = Task.CompletedTask;
    public Task PendingSave => _saving;

    public SettingsView() => InitializeComponent();

    public void Configure(LocalizationService strings, AppPreferences preferences)
    {
        _strings = strings;
        _preferences = preferences;
        Relocalize();
    }

    public void Relocalize()
    {
        if (_strings is null || _preferences is null) return;
        _updating = true;
        try
        {
            Heading.Text = _strings["desktop_settings"];
            LanguageLabel.Text = _strings["settings_language"];
            AutomationProperties.SetName(LanguageSelector, _strings["settings_language"]);
            SystemLanguage.Content = _strings["language_system"];
            ChineseLanguage.Content = _strings["language_chinese"];
            EnglishLanguage.Content = _strings["language_english"];
            LanguageSelector.SelectedIndex = (int)_preferences.Language;
            SaveError.Message = _strings["settings_language_save_failed"];
        }
        finally { _updating = false; }
    }

    private async void OnLanguageChanged(object sender, SelectionChangedEventArgs args)
    {
        if (_updating || _strings is null || _preferences is null || LanguageSelector.SelectedItem is not ComboBoxItem item) return;
        _saving = SaveAsync(Enum.Parse<LanguageMode>((string)item.Tag));
        await _saving;
    }

    private async Task SaveAsync(LanguageMode mode)
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
}
