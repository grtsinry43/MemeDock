using MemeDock.Windows.Services;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.ApplicationModel.DataTransfer;

namespace MemeDock.Windows.Views;

public sealed partial class TrayPanelWindow : Window
{
    private MainWindow? _main;
    private DesktopIntegration? _integration;
    internal StickerPickerView Surface => Picker;
    internal event Action? ContentSizeChanged;
    internal double MeasureHeight(double width)
    {
        Root.Measure(new global::Windows.Foundation.Size(width, double.PositiveInfinity));
        return Math.Ceiling(Root.DesiredSize.Height);
    }
    public TrayPanelWindow()
    {
        InitializeComponent();
        ExtendsContentIntoTitleBar = true;
        Picker.ContentSizeChanged += () => ContentSizeChanged?.Invoke();
        SystemBackdrop = new MicaBackdrop();
        AppWindow.IsShownInSwitchers = false;
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.SetBorderAndTitleBar(false, false); presenter.IsResizable = false;
            presenter.IsMinimizable = false; presenter.IsMaximizable = false; presenter.IsAlwaysOnTop = true;
        }
    }
    internal bool MenuOpen => More.Flyout?.IsOpen == true || Import.Flyout?.IsOpen == true;
    internal void Configure(MainWindow main, DesktopIntegration integration) { _main = main; _integration = integration; Relocalize(); }
    internal void Relocalize()
    {
        if (_main is null) return;
        var s = _main.Strings;
        ChooseLabel.Text = s["picker_choose"]; ImportLabel.Text = s["library_add"]; LibraryLabel.Text = s["picker_library"]; SettingsLabel.Text = s["desktop_settings"];
        Files.Text = s["desktop_import_files"]; FromClipboard.Text = s["desktop_import_clipboard"]; Telegram.Text = s["tg_title"];
        Quit.Text = s["desktop_quit"]; SyncNow.Text = s["sync_now"];
        ToolTipService.SetToolTip(CancelActivity, s["tg_cancel"]);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(CancelActivity, s["tg_cancel"]);
        RecentsHeading.Text = s["picker_recent"];
        ViewAll.Content = s["picker_view_all"];
        foreach (var (button, label) in new[] { (Choose, ChooseLabel), (Import, ImportLabel), (Library, LibraryLabel), (Settings, SettingsLabel) })
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, label.Text);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(More, s["picker_more_actions"]);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(DismissHint, s["picker_close"]);
        Update(); Picker.Relocalize();
    }
    internal void Update()
    {
        if (_main is null || _integration is null) return;
        var vm = _main.ViewModel;
        Import.IsEnabled = vm.IsReady && !vm.IsRestoring && !vm.IsImporting && vm.Telegram?.Busy != true;
        var key = !vm.IsReady ? "picker_loading_library" : vm.IsRestoring ? "picker_restoring" : null;
        if (key is not null) Status.Content = _main.Strings[key];
        else if (vm.IsImporting) Status.Content = vm.ImportText;
        else if (vm.Telegram is { Busy: true } tg) Status.Content = tg.Phase == ViewModels.TelegramPhase.Importing
            ? _main.Strings.Format("tg_progress", tg.Completed, tg.Progress?.Items.Length ?? 0) : _main.Strings["tg_loading"];
        else if (vm.Sync is { } sync && SyncPresentation.IsVisible(sync.State))
            Status.Content = _main.Strings[SyncPresentation.StatusKey(sync.State)];
        else Status.Content = "";
        Status.Visibility = string.IsNullOrEmpty(Status.Content as string) ? Visibility.Collapsed : Visibility.Visible;
        ActivityRegion.Visibility = Status.Visibility;
        CancelActivity.Visibility = vm.IsImporting || vm.Telegram?.Busy == true ? Visibility.Visible : Visibility.Collapsed;
        ImportProgress.Visibility = vm.IsImporting || vm.Telegram?.Phase == ViewModels.TelegramPhase.Importing ? Visibility.Visible : Visibility.Collapsed;
        ImportProgress.Maximum = vm.IsImporting ? Math.Max(1, vm.ImportTotal) : Math.Max(1, vm.Telegram?.Progress?.Items.Length ?? 1);
        ImportProgress.Value = vm.IsImporting ? Math.Max(0, vm.ImportCompleted - 1) : vm.Telegram?.Completed ?? 0;
        ImportProgress.IsIndeterminate = vm.IsImporting ? vm.ImportTotal == 0 : vm.Telegram?.Progress is null;
        SyncNow.Visibility = vm.Sync?.State.Status.Connected == true ? Visibility.Visible : Visibility.Collapsed;
        SyncNow.IsEnabled = vm.Sync is { } coordinator && SyncPresentation.CanSynchronize(coordinator.State) && SyncPresentation.Phase(coordinator.State) != SyncPhase.Running;
        var desktop = _integration.Preferences.Desktop;
        ShortcutHint.Content = _main.Strings[desktop.Shortcut is null ? "picker_set_shortcut" : "picker_title"];
        ShortcutValue.SetShortcut(desktop.Shortcut);
        ShortcutValue.Visibility = desktop.Shortcut is null ? Visibility.Collapsed : Visibility.Visible;
        ShortcutFooter.Visibility = desktop.Shortcut is null && desktop.ShortcutHintDismissed ? Visibility.Collapsed : Visibility.Visible;
        DismissHint.Visibility = desktop.Shortcut is null && !desktop.ShortcutHintDismissed ? Visibility.Visible : Visibility.Collapsed;
        ContentSizeChanged?.Invoke();
    }
    private void OnContentSizeChanged(object sender, SizeChangedEventArgs args) => ContentSizeChanged?.Invoke();
    private void OnChoose(object sender, RoutedEventArgs args) => _integration?.ShowPickerFromTray();
    private void OnLibrary(object sender, RoutedEventArgs args) => _integration?.OpenLibrary(null);
    private void OnSettings(object sender, RoutedEventArgs args) => _integration?.OpenSettings();
    private void OnStatus(object sender, RoutedEventArgs args) => _integration?.OpenSettings();
    private void OnCancelActivity(object sender, RoutedEventArgs args)
    {
        if (_main?.ViewModel.IsImporting == true) _main.ViewModel.CancelImport();
        else _main?.ViewModel.Telegram?.Cancel();
    }
    private void OnQuit(object sender, RoutedEventArgs args) => _integration?.Quit();
    private async void OnFiles(object sender, RoutedEventArgs args) { if (_integration is not null) await _integration.ImportFilesAsync(); }
    private async void OnClipboard(object sender, RoutedEventArgs args) { if (_integration is not null) await _integration.ImportClipboardAsync(); }
    private void OnTelegram(object sender, RoutedEventArgs args) => _integration?.OpenTelegram();
    private async void OnSync(object sender, RoutedEventArgs args) { if (_main?.ViewModel.Sync is { } sync) await sync.SynchronizeNowAsync(); }
    private async void OnDismissHint(object sender, RoutedEventArgs args)
    {
        if (_integration is null) return;
        try { await _integration.SavePreferencesAsync(_integration.Preferences.Desktop with { ShortcutHintDismissed = true }); Update(); }
        catch (Exception) { Picker.ShowMessage("desktop_preferences_failed"); }
    }
    private void OnDragOver(object sender, DragEventArgs args)
    {
        args.AcceptedOperation = Import.IsEnabled && !args.DataView.Contains(StickerImportService.InternalDragFormat)
            && args.DataView.Contains(StandardDataFormats.StorageItems) ? DataPackageOperation.Copy : DataPackageOperation.None;
    }
    private async void OnDrop(object sender, DragEventArgs args)
    {
        if (_integration is null || !Import.IsEnabled || args.DataView.Contains(StickerImportService.InternalDragFormat)) return;
        var deferral = args.GetDeferral();
        try { await _integration.ImportDropAsync(args.DataView); }
        finally { deferral.Complete(); }
    }
}
