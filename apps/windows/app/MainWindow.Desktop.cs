using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using MemeDock.Windows.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;

namespace MemeDock.Windows;

public sealed partial class MainWindow
{
    private DesktopIntegration? _desktop;
    private bool _quitRequested;
    internal bool IsResidentWindowActive => _windowActive && AppWindow.IsVisible;
    private void InitializeDesktop()
    {
        try { _desktop = new DesktopIntegration(this, _preferences); Settings.ConfigureDesktop(_desktop); }
        catch (Exception error) { System.Diagnostics.Trace.WriteLine($"Desktop integration unavailable: {error.GetType().Name}"); }
        TelegramNavigation.Content = Strings["tg_title"];
    }
    internal void ShowResidentWindow()
    {
        if (_stopping) return;
        AppWindow.Show();
        if (AppWindow.Presenter is Microsoft.UI.Windowing.OverlappedPresenter presenter
            && presenter.State == Microsoft.UI.Windowing.OverlappedPresenterState.Minimized) presenter.Restore();
        Activate();
    }
    internal async Task OpenResidentLibraryAsync(string? id)
    {
        if (_stopping) return;
        ShowResidentWindow();
        try
        {
            if (ViewModel.IsRestoring || ViewModel.Batch?.IsRunning == true || ViewModel.Details is { } edited && !await edited.CommitAsync()) return;
            Navigation.SelectedItem = AllNavigation;
            await ViewModel.NavigateAsync(LibraryFilter.All);
            if (id is not null)
            {
                if (ViewModel.Cards.FirstOrDefault(card => card.Id == id) is { } card)
                { Gallery.SelectedItem = card; Gallery.ScrollIntoView(card); }
                else if (!_stopping && ViewModel.Details is { } detail)
                {
                    // Resolve the selected sticker directly instead of loading every page
                    // of a large library just to locate an old recent item.
                    Details.ShowPlaceholder(id, null);
                    Details.Visibility = Visibility.Visible; Batch.Visibility = Visibility.Collapsed;
                    Inspector.IsPaneOpen = true;
                    await detail.SelectAsync(id);
                    Details.SetActive(true);
                }
            }
        }
        catch (Exception error) { ShowDesktopFailure(Strings.ErrorKey(error)); }
    }
    internal void ShowResidentSettings() { ShowResidentWindow(); Navigation.SelectedItem = Navigation.SettingsItem; }
    internal void RequestQuit() { _quitRequested = true; Close(); }
    internal void ShowDesktopFailure(string key)
    {
        if (_stopping) return;
        ShowResidentWindow();
        Notice.Show(Strings[key], InfoBarSeverity.Error);
    }
    internal async Task ImportResidentFilesAsync()
    {
        if (_stopping) return;
        ShowResidentWindow();
        Navigation.SelectedItem = AllNavigation;
        await ViewModel.NavigateAsync(LibraryFilter.All);
        await StartImportAsync();
    }
    internal Task ImportResidentAsync(Func<CancellationToken, Task<IReadOnlyList<StickerImportSource>>> prepare)
    {
        if (_stopping || !_importing.IsCompleted || ViewModel.Telegram?.Busy == true) return Task.CompletedTask;
        return _importing = ImportResidentCoreAsync(prepare);
    }
    private async Task ImportResidentCoreAsync(Func<CancellationToken, Task<IReadOnlyList<StickerImportSource>>> prepare)
    {
        // The tray import is library-wide, independent of the hidden main window's filter.
        await ViewModel.ImportIntoLibraryAsync(prepare);
        if (!_stopping) _desktop?.ShowImportResult();
    }
    private async void OnTelegram(object sender, RoutedEventArgs args) => await ShowTelegramAsync();
    internal async Task RetryResidentAsync()
    {
        if (_stopping) return;
        if (ViewModel.IsReady) await ViewModel.RetryAsync();
        else { _initialization = InitializeAsync(); await _initialization; }
    }
    internal async Task ShowTelegramAsync()
    {
        if (_stopping || _dialog is not null || !ViewModel.IsReady || ViewModel.IsRestoring || ViewModel.IsImporting || ViewModel.Telegram is not { } telegram) return;
        ShowResidentWindow();
        var view = new TelegramImportView();
        view.Configure(telegram, Strings);
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot, Title = Strings["tg_title"], Content = view,
            CloseButtonText = Strings["picker_close"], RequestedTheme = Root.ActualTheme
        };
        dialog.Resources["ContentDialogMaxWidth"] = 800d;
        _dialog = dialog;
        try { await telegram.StartAsync(); if (!_stopping) await dialog.ShowAsync(); }
        catch (Exception error) { if (!_stopping) ShowDesktopFailure(Strings.ErrorKey(error)); }
        finally { telegram.Cancel(); view.Detach(); if (ReferenceEquals(_dialog, dialog)) _dialog = null; }
    }
}
