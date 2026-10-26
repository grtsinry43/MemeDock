using System.ComponentModel;
using System.Diagnostics;
using System.Globalization;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;

namespace MemeDock.Windows.Views;

public sealed partial class SettingsView : UserControl
{
    private LocalizationService? _strings;
    private AppPreferences? _preferences;
    private StorageViewModel? _storage;
    private BackupViewModel? _backup;
    private bool _updating;
    private bool _closed;
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
    /// The storage section appears once the library is open. Until then the page
    /// says so instead of showing zeroes.
    /// </summary>
    public void ConfigureStorage(StorageViewModel? storage)
    {
        _storage = storage;
        if (storage is null)
        {
            ShowStorage();
            return;
        }
        Relocalize();
        _ = RefreshStorageAsync();
    }

    /// <summary>
    /// Re-reads the figures. Called when the page becomes visible so the numbers
    /// reflect work done since the last visit (imports, exports, cleanup).
    /// </summary>
    public async Task RefreshStorageAsync()
    {
        if (_closed) return;
        if (_storage is not null) await _storage.RefreshAsync();
        if (!_closed) ShowStorage();
    }

    /// <summary>
    /// The backup section follows the view model for the whole life of the page: the
    /// stage text changes while a run is in flight, so the page listens instead of
    /// reading the model only after a click returns.
    /// </summary>
    public void ConfigureBackup(BackupViewModel? backup)
    {
        if (_backup is not null) _backup.PropertyChanged -= OnBackupChanged;
        _backup = backup;
        if (backup is null)
        {
            ShowBackup();
            return;
        }
        backup.PropertyChanged += OnBackupChanged;
        Relocalize();
        ShowBackup();
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

    /// <summary>
    /// Stops this page from touching its controls after the window started closing.
    /// The view model itself is released by <see cref="LibraryViewModel.DisposeAsync"/>.
    /// </summary>
    public void ShutdownStorage()
    {
        _closed = true;
        _storage = null;
    }

    /// <summary>
    /// Detaches the page from the backup view model. The view model keeps owning the
    /// running work; the window releases it only after the session is done with it.
    /// </summary>
    public void ShutdownBackup()
    {
        if (_backup is not null) _backup.PropertyChanged -= OnBackupChanged;
        _backup = null;
    }

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

            StorageSpaceLabel.Text = _strings["desktop_space"];
            AutomationProperties.SetName(StorageRefresh, _strings["desktop_refresh"]);
            OriginalCountLabel.Text = _strings["settings_original_count"];
            SavedOriginalsLabel.Text = _strings["settings_saved_originals"];
            CacheUsageLabel.Text = _strings["settings_cache_usage"];
            StorageHint.Text = _strings["settings_cache_footer"];
            StorageRetry.Content = _strings["retry"];
            StorageNotice.Message = _strings["desktop_storage_not_ready"];

            CleanupLabel.Text = _strings["desktop_cleanup"];
            CleanupHint.Text = _strings["desktop_cleanup_hint"];
            CleanButton.Content = _strings["desktop_clean"];
            AutomationProperties.SetName(CleanButton, _strings["desktop_clean"]);

            BackupLabel.Text = _strings["desktop_backup"];
            BackupHint.Text = _strings["desktop_backup_hint"];
            CreateBackupButton.Content = _strings["desktop_backup_create"];
            AutomationProperties.SetName(CreateBackupButton, _strings["desktop_backup_create"]);
            CheckpointLabel.Text = _strings["desktop_checkpoint"];
            CheckpointHint.Text = _strings["desktop_checkpoint_hint"];
            CreateCheckpointButton.Content = _strings["desktop_checkpoint_create"];
            AutomationProperties.SetName(CreateCheckpointButton, _strings["desktop_checkpoint_create"]);

            LocationsLabel.Text = _strings["desktop_locations"];
            DataDirectoryLabel.Text = _strings["desktop_data_directory"];
            CacheDirectoryLabel.Text = _strings["desktop_cache_directory"];
            ShareDirectoryLabel.Text = _strings["desktop_export_directory"];
            OpenDataDirectory.Content = _strings["desktop_open"];
            OpenCacheDirectory.Content = _strings["desktop_open"];
            OpenShareDirectory.Content = _strings["desktop_open"];
            AutomationProperties.SetName(OpenDataDirectory, _strings["desktop_open"] + " " + _strings["desktop_data_directory"]);
            AutomationProperties.SetName(OpenCacheDirectory, _strings["desktop_open"] + " " + _strings["desktop_cache_directory"]);
            AutomationProperties.SetName(OpenShareDirectory, _strings["desktop_open"] + " " + _strings["desktop_export_directory"]);

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
            ShowStorage();
            ShowBackup();
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

    /// <summary>
    /// Re-applies every selector from the persisted values after a write commits.
    /// A relocalization triggered while another write was pending can leave a
    /// selector showing the previous value even though the new value is stored.
    /// </summary>
    private void SyncSelectors()
    {
        if (_preferences is null) return;
        _updating = true;
        try
        {
            AppearanceSelector.SelectedIndex = (int)_preferences.Appearance;
            LanguageSelector.SelectedIndex = (int)_preferences.Language;
            SelectPreset(CopyStillSelector, _preferences.CopyOutput.Still);
            SelectPreset(CopyAnimatedSelector, _preferences.CopyOutput.Animated);
            SelectPreset(DragStillSelector, _preferences.DragOutput.Still);
            SelectPreset(DragAnimatedSelector, _preferences.DragOutput.Animated);
        }
        finally
        {
            _updating = false;
        }
        UpdateHints();
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
            SyncSelectors();
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Appearance could not be saved: {error}");
            AppearanceChanged?.Invoke(previous);
            _updating = true;
            try { AppearanceSelector.SelectedIndex = (int)previous; }
            finally { _updating = false; }
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
            // A language change can relocalize this page while the write is in flight,
            // which resets the selector to the value that was persisted back then.
            SyncSelectors();
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

    /// <summary>
    /// Renders the storage section from the view model. A missing snapshot is
    /// always shown as an em dash, never as "0 B": "could not read" and "nothing
    /// stored" must not look alike.
    /// </summary>
    private void ShowStorage()
    {
        if (_strings is null) return;
        var storage = _storage;
        var ready = storage is not null && storage.IsReady;
        StorageNotice.IsOpen = storage is null;
        StorageRefresh.IsEnabled = ready;
        StorageProgress.IsActive = storage?.IsLoading == true;

        var snapshot = storage?.Snapshot;
        OriginalCountValue.Text = snapshot is null
            ? AppVersion.Unknown
            : snapshot.OriginalCount.ToString("N0", CultureInfo.CurrentCulture);
        SavedOriginalsValue.Text = snapshot is null ? AppVersion.Unknown : ByteSize.Format(snapshot.OriginalBytes);
        CacheUsageValue.Text = snapshot is null ? AppVersion.Unknown : ByteSize.Format(snapshot.CacheBytes);

        var statisticsError = storage?.StatisticsErrorKey;
        StorageError.Message = statisticsError is null ? "" : _strings[statisticsError];
        StorageError.IsOpen = statisticsError is not null;

        CleanButton.IsEnabled = ready && storage?.IsCleaning != true;
        CleanProgress.IsActive = storage?.IsCleaning == true;
        if (storage?.CleanupErrorKey is { } cleanupError)
        {
            CleanupBar.Severity = InfoBarSeverity.Error;
            CleanupBar.Message = _strings[cleanupError];
            CleanupBar.IsOpen = true;
        }
        else if (storage?.CleanupResultKey is { } cleanupResult)
        {
            CleanupBar.Severity = InfoBarSeverity.Success;
            CleanupBar.Message = _strings.Format(
                cleanupResult,
                (storage.CleanedItems ?? 0).ToString("N0", CultureInfo.CurrentCulture));
            CleanupBar.IsOpen = true;
        }
        else
        {
            CleanupBar.IsOpen = false;
        }

        SetPath(DataDirectoryValue, OpenDataDirectory, storage?.DataDirectory, ready);
        SetPath(CacheDirectoryValue, OpenCacheDirectory, storage?.CacheDirectory, ready);
        SetPath(ShareDirectoryValue, OpenShareDirectory, storage?.ShareDirectory, ready);
        var directoryError = storage?.DirectoryErrorKey;
        DirectoryBar.Message = directoryError is null ? "" : _strings[directoryError];
        DirectoryBar.IsOpen = directoryError is not null;
    }

    /// <summary>
    /// Renders the backup card. Only work the core really reports is shown: there is
    /// no percentage, because the core exposes stages rather than a fraction. The
    /// saved location is shown only after the copy completed.
    /// </summary>
    private void ShowBackup()
    {
        if (_strings is null) return;
        var backup = _backup;
        var ready = backup is not null && backup.IsReady;
        CreateBackupButton.IsEnabled = ready && backup?.IsBusy != true;
        CreateCheckpointButton.IsEnabled = ready && backup?.IsBusy != true;
        BackupProgress.IsActive = backup?.IsCreatingBackup == true;
        CheckpointProgress.IsActive = backup?.IsCreatingCheckpoint == true;
        BackupStage.Text = backup?.StageKey is { } stage ? _strings[stage] : "";

        if (backup?.BackupErrorKey is { } backupError)
        {
            BackupBar.Severity = InfoBarSeverity.Error;
            BackupBar.Message = backup.BackupErrorPlaceholder is { } placeholder
                ? _strings.Format(backupError, placeholder)
                : _strings[backupError];
            BackupBar.IsOpen = true;
        }
        else if (backup?.BackupSavedPath is not null)
        {
            BackupBar.Severity = InfoBarSeverity.Success;
            BackupBar.Message = _strings["desktop_backup_saved"];
            BackupBar.IsOpen = true;
        }
        else
        {
            BackupBar.IsOpen = false;
        }

        var savedPath = backup?.BackupSavedPath;
        var savedBytes = backup?.BackupSavedBytes;
        BackupSavedPanel.Visibility = savedPath is null ? Visibility.Collapsed : Visibility.Visible;
        BackupSavedLabel.Text = _strings["desktop_backup_saved_to"];
        BackupSavedValue.Text = savedPath is null
            ? ""
            : savedBytes is { } size ? $"{savedPath} ({ByteSize.Format(size)})" : savedPath;
        ToolTipService.SetToolTip(BackupSavedValue, savedPath);

        if (backup?.CheckpointErrorKey is { } checkpointError)
        {
            CheckpointBar.Severity = InfoBarSeverity.Error;
            CheckpointBar.Message = _strings[checkpointError];
            CheckpointBar.IsOpen = true;
        }
        else if (backup?.CheckpointName is { } checkpoint)
        {
            CheckpointBar.Severity = InfoBarSeverity.Success;
            CheckpointBar.Message = _strings.Format("desktop_checkpoint_created", checkpoint);
            CheckpointBar.IsOpen = true;
        }
        else
        {
            CheckpointBar.IsOpen = false;
        }
    }

    private void OnBackupChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (_closed) return;
        ShowBackup();
    }

    private async void OnCreateBackup(object sender, RoutedEventArgs args)
    {
        if (_backup is null || _closed) return;
        await _backup.CreateBackupAsync();
        if (!_closed) ShowBackup();
    }

    private async void OnCreateCheckpoint(object sender, RoutedEventArgs args)
    {
        if (_backup is null || _closed) return;
        await _backup.CreateCheckpointAsync();
        if (!_closed) ShowBackup();
    }

    private static void SetPath(TextBlock text, Button open, string? path, bool ready)
    {
        text.Text = path ?? AppVersion.Unknown;
        ToolTipService.SetToolTip(text, path);
        open.IsEnabled = ready && path is not null;
    }

    private async void OnStorageRefresh(object sender, RoutedEventArgs args) => await RefreshStorageAsync();

    private async void OnCleanArtifacts(object sender, RoutedEventArgs args)
    {
        if (_storage is null || _strings is null || _closed) return;
        await _storage.CleanAsync();
        if (!_closed) ShowStorage();
    }

    private void OnOpenDataDirectory(object sender, RoutedEventArgs args) => OpenStorageDirectory(_storage?.DataDirectory);

    private void OnOpenCacheDirectory(object sender, RoutedEventArgs args) => OpenStorageDirectory(_storage?.CacheDirectory);

    private void OnOpenShareDirectory(object sender, RoutedEventArgs args) => OpenStorageDirectory(_storage?.ShareDirectory);

    private void OpenStorageDirectory(string? directory)
    {
        if (_storage is null || _strings is null || directory is null || _closed) return;
        _storage.OpenDirectory(directory);
        ShowStorage();
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
