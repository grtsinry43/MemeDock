using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using MemeDock.Windows.Models;
using MemeDock.Windows.Views;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Windows.ApplicationModel.DataTransfer;

namespace MemeDock.Windows.Services;

internal sealed class DesktopIntegration : IAsyncDisposable
{
    private readonly MainWindow _main;
    private readonly TrayIcon _tray;
    private readonly GlobalShortcut _shortcut;
    private readonly InputPasteSession _input;
    private readonly DesktopNative.WinEventProc _foregroundCallback;
    private readonly nint _foregroundHook;
    private readonly SemaphoreSlim _saves = new(1);
    private SyncCoordinator? _sync;
    private QuickPickerWindow? _picker;
    private TrayPanelWindow? _hub;
    private Window? _active;
    private StickerPickerView? _surface;
    private nint _lastContext, _returnWindow, _nativeMenuContext;
    private long _version;
    private bool _stopping, _hiding, _refreshPending;
    private bool _hubResizeQueued;
    private ScreenRect _hubAnchor;
    private Task _presentation = Task.CompletedTask, _output = Task.CompletedTask;
    public AppPreferences Preferences { get; }
    public bool CanReside => _tray.Available;
    internal bool IsMainForeground => DesktopNative.GetForegroundWindow() == WinRT.Interop.WindowNative.GetWindowHandle(_main);
    public string? ShortcutError { get; private set; }
    public event Action? SettingsChanged;

    public DesktopIntegration(MainWindow main, AppPreferences preferences)
    {
        (_main, Preferences) = (main, preferences);
        var handle = WinRT.Interop.WindowNative.GetWindowHandle(main);
        _lastContext = DesktopNative.GetForegroundWindow();
        _tray = new TrayIcon(handle, main.Strings);
        _input = new();
        _shortcut = new GlobalShortcut(handle);
        try { _shortcut.Apply(preferences.Desktop.Shortcut); }
        catch (Win32Exception) { ShortcutError = "picker_shortcut_conflict"; }
        _tray.Selected += ToggleHub;
        _tray.Command += OnCommand;
        _tray.ContextOpening += () => _nativeMenuContext = MenuContext;
        _tray.Hotkey += id => { if (_shortcut.Handles(id)) TogglePicker(); };
        _tray.AvailabilityChanged += available => { if (!available) _main.ShowResidentWindow(); };
        _foregroundCallback = (_, _, window, _, _, _, _) =>
        {
            var name = new StringBuilder(256);
            DesktopNative.GetClassName(window, name, name.Capacity);
            if (window != 0 && name.ToString() is not ("Shell_TrayWnd" or "Shell_SecondaryTrayWnd" or "NotifyIconOverflowWindow"))
                _lastContext = window;
            _main.DispatcherQueue.TryEnqueue(CheckDismiss);
        };
        _foregroundHook = DesktopNative.SetWinEventHook(3, 3, 0, _foregroundCallback, 0, 0, 0);
        main.ViewModel.PropertyChanged += OnModelChanged;
        main.ViewModel.CatalogChanged += OnCatalogChanged;
        main.ViewModel.ContentChanged += OnContentChanged;
        main.ViewModel.ThumbnailInvalidated += OnThumbnailInvalidated;
        main.ViewModel.Rebuilt += OnRebuilt;
        main.Strings.PropertyChanged += OnLanguageChanged;
        ApplyAppearance();
    }
    private void OnCommand(int command)
    {
        switch (command)
        {
            case 1: _presentation = PresentAsync(false, _nativeMenuContext); break;
            case 2: OpenLibrary(null); break;
            case 3: _ = ImportFilesAsync(); break;
            case 4: OpenSettings(); break;
            case 5: Quit(); break;
        }
    }
    private nint MenuContext => _active is not null ? _returnWindow : _foregroundHook == 0 ? 0 : _lastContext;
    private void ToggleHub()
    {
        if (_stopping) return;
        if (_active == _hub && _active is not null) { Dismiss(true); return; }
        _presentation = PresentAsync(true, MenuContext);
    }
    private void TogglePicker()
    {
        if (_stopping) return;
        if (_active == _picker && _active is not null) { Dismiss(true); return; }
        _presentation = PresentAsync(false, DesktopNative.GetForegroundWindow());
    }
    public void ShowPickerFromTray() { if (!_stopping) _presentation = PresentAsync(false, MenuContext); }
    private void EnsureWindows()
    {
        if (_picker is not null) return;
        _picker = new(); _hub = new();
        _hub.ContentSizeChanged += ScheduleHubResize;
        _hub.Configure(_main, this);
        ConfigureWindow(_picker, _picker.Surface);
        ConfigureWindow(_hub, _hub.Surface);
        Rebind();
    }
    private void ConfigureWindow(Window window, StickerPickerView surface)
    {
        window.Activated += (_, args) =>
        {
            var foreground = DesktopNative.GetForegroundWindow();
            _main.ViewModel.Output?.SetForeground(foreground == WinRT.Interop.WindowNative.GetWindowHandle(window) || _main.IsResidentWindowActive);
            Trace.WriteLine($"Panel activation: state={args.WindowActivationState}, foreground={foreground == WinRT.Interop.WindowNative.GetWindowHandle(window)}");
            if (args.WindowActivationState == WindowActivationState.Deactivated) _main.DispatcherQueue.TryEnqueue(CheckDismiss);
        };
        window.AppWindow.Closing += (_, args) => { if (!_stopping) { args.Cancel = true; Dismiss(true); } };
        surface.Dismiss = Dismiss; surface.Open = OpenLibrary; surface.InteractionEnded = CheckDismiss;
        surface.Output = (card, paste, kind) => _output = OutputAsync(card, paste, kind, surface, window);
        surface.Save = async card =>
        {
            if (_main.ViewModel.IsRestoring || _main.ViewModel.Output is not { CanStart: true } output) return;
            await (_output = output.SaveAsync(new(card.Id, card.Generation), window.AppWindow.Id));
            if (output.ErrorKey is { } error) surface.ShowMessage(error);
            else if (output.MessageKey is { } message) surface.ShowMessage(message);
        };
    }
    private void Rebind()
    {
        if (!ReferenceEquals(_sync, _main.ViewModel.Sync))
        {
            if (_sync is not null) _sync.StateChanged -= OnSyncChanged;
            _sync = _main.ViewModel.Sync;
            if (_sync is not null) _sync.StateChanged += OnSyncChanged;
        }
        _picker?.Surface.Configure(_main, _main.ViewModel.Picker, false);
        _hub?.Surface.Configure(_main, _main.ViewModel.Recents, true);
        ApplyAppearance();
        if (_active == _picker && _active is not null && _main.ViewModel.Picker?.Visible == false)
            _presentation = _picker!.Surface.PresentAsync();
        else if (_active == _hub && _active is not null && _main.ViewModel.Recents?.Visible == false)
            _presentation = _hub!.Surface.PresentAsync();
    }
    private async Task PresentAsync(bool hub, nint target)
    {
        if (_stopping || _surface?.Interacting == true || _input.InProgress) return;
        Dismiss(false);
        var version = ++_version;
        _returnWindow = target;
        try
        {
            EnsureWindows();
            var foregroundBeforeCapture = DesktopNative.GetForegroundWindow();
            var bounds = await _input.CaptureAsync(target);
            if (_stopping || version != _version) return;
            // Resolving an input anchor may await another process. Do not steal
            // focus back if the user switched applications during that operation.
            var foregroundAfterCapture = DesktopNative.GetForegroundWindow();
            if (foregroundAfterCapture != foregroundBeforeCapture && foregroundAfterCapture != target) { _input.Reset(); return; }
            var window = hub ? (Window)_hub! : _picker!;
            var surface = hub ? _hub!.Surface : _picker!.Surface;
            _active = window; _surface = surface;
            DesktopNative.GetCursorPos(out var point);
            var anchor = hub ? _tray.Bounds() : Preferences.Desktop.AnchorToInput && bounds is { } input ? input : new(point.X, point.Y, 1, 1);
            if (hub) _hubAnchor = anchor;
            Position(window, anchor, hub ? 400 : 480, hub ? (int)_hub!.MeasureHeight(400) : 360);
            window.Title = _main.Strings[hub ? "picker_library" : "picker_title"];
            ApplyAppearance();
            if (!ActivatePanel(window)) { Dismiss(false, force: true); return; }
            surface.FocusInput();
            await surface.PresentAsync();
            if (_stopping || version != _version) return;
            CheckDismiss();
            _hub?.Update();
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Present panel: {error.GetType().Name}");
            if (!_stopping) _main.ShowDesktopFailure(_main.Strings.ErrorKey(error));
        }
    }
    private static bool ActivatePanel(Window window)
    {
        var handle = WinRT.Interop.WindowNative.GetWindowHandle(window);
        var activated = PanelFocus.Activate(handle, () =>
        {
            window.AppWindow.Show(true);
            window.Activate();
        }, DesktopNative.SetForegroundWindow, DesktopNative.GetForegroundWindow);
        Trace.WriteLine($"Panel foreground request: accepted={activated}");
        return activated;
    }
    private static void Position(Window window, ScreenRect anchor, int width, int height)
    {
        var monitor = DesktopNative.MonitorFromPoint(new() { X = anchor.X, Y = anchor.Y }, 2);
        var info = new DesktopNative.MonitorInfo { Size = (uint)Marshal.SizeOf<DesktopNative.MonitorInfo>() };
        if (!DesktopNative.GetMonitorInfo(monitor, ref info)) throw new Win32Exception(Marshal.GetLastWin32Error());
        var handle = WinRT.Interop.WindowNative.GetWindowHandle(window);
        if (DesktopNative.MonitorFromWindow(handle, 2) != monitor)
        {
            // Move the hidden window onto the target display first so Windows reports
            // that window's real per-monitor DPI, including PerMonitorV2 changes.
            var previousSize = window.AppWindow.Size;
            var seed = PanelPlacement.Place(info.Work.Bounds, anchor, previousSize.Width, previousSize.Height, 0);
            window.AppWindow.MoveAndResize(new(seed.X, seed.Y, seed.Width, seed.Height));
        }
        var dpi = DesktopNative.GetDpiForWindow(handle);
        if (dpi == 0) throw new InvalidOperationException("The panel has no valid window DPI.");
        var scale = dpi / 96d;
        var rect = PanelPlacement.Place(info.Work.Bounds, anchor, (int)(width * scale), (int)(height * scale), (int)(8 * scale));
        var position = window.AppWindow.Position;
        var size = window.AppWindow.Size;
        if (position.X != rect.X || position.Y != rect.Y || size.Width != rect.Width || size.Height != rect.Height)
            window.AppWindow.MoveAndResize(new(rect.X, rect.Y, rect.Width, rect.Height));
    }
    private void ScheduleHubResize()
    {
        if (_stopping || _hubResizeQueued || _hub is null || !ReferenceEquals(_active, _hub)) return;
        _hubResizeQueued = true;
        _main.DispatcherQueue.TryEnqueue(() =>
        {
            _hubResizeQueued = false;
            if (_stopping || _hub is null || !ReferenceEquals(_active, _hub)) return;
            Position(_hub, _hubAnchor, 400, (int)_hub.MeasureHeight(400));
        });
    }
    public void Dismiss(bool restoreFocus) => Dismiss(restoreFocus, force: false);
    private void Dismiss(bool restoreFocus, bool force)
    {
        if (_hiding || !force && !_stopping && (_surface?.Interacting == true || _input.InProgress)) return;
        _hiding = true;
        try
        {
            ++_version;
            var wasVisible = _active is not null;
            _active?.AppWindow.Hide(); _surface?.Suspend();
            _active = null; _surface = null; _input.Reset();
            _main.ViewModel.Output?.SetForeground(_main.IsResidentWindowActive);
            if (restoreFocus && wasVisible && DesktopNative.IsWindow(_returnWindow)) DesktopNative.SetForegroundWindow(_returnWindow);
        }
        finally { _hiding = false; }
    }
    private void CheckDismiss()
    {
        if (_stopping || _hiding || _active is null) return;
        var foreground = DesktopNative.GetForegroundWindow();
        if (foreground == 0) return;
        if (PanelFocus.ShouldDismiss(foreground == WinRT.Interop.WindowNative.GetWindowHandle(_active),
            DesktopNative.Process(foreground) == Environment.ProcessId,
            _surface?.MenuOpen == true || _hub?.MenuOpen == true, _surface?.Dragging == true,
            _surface?.ModalOperation == true, _input.InProgress)) Dismiss(false, force: true);
        else if (_refreshPending) { _refreshPending = false; OnContentChanged(null, null); }
    }
    private async Task OutputAsync(StickerCard card, bool paste, StickerOutputKind kind, StickerPickerView surface, Window window)
    {
        var vm = _main.ViewModel;
        if (_stopping || !vm.IsReady || vm.IsRestoring || vm.Output is not { CanStart: true } output) return;
        var version = _version;
        if (vm.Details is { } detail && detail.CurrentId == card.Id && !await detail.CommitAsync()) return;
        if (!await output.CopyAsync(new(card.Id, card.Generation), kind)) { surface.ShowMessage(output.ErrorKey ?? "failure_unknown"); return; }
        var sequence = output.ClipboardSequence;
        if (_stopping || version != _version) return;
        if (paste)
        {
            var error = await _input.PasteAsync(sequence, () => window.AppWindow.Hide());
            if (_stopping || version != _version) return;
            if (error is not null)
            {
                if (ActivatePanel(window)) { surface.FocusInput(); surface.ShowMessage(error); }
                else Dismiss(false, force: true);
                return;
            }
        }
        var ownedForeground = DesktopNative.Process(DesktopNative.GetForegroundWindow()) == Environment.ProcessId;
        window.AppWindow.Hide();
        if (!paste && ownedForeground && DesktopNative.IsWindow(_returnWindow)) DesktopNative.SetForegroundWindow(_returnWindow);
        _main.DispatcherQueue.TryEnqueue(() => Dismiss(false));
    }
    public async Task SavePreferencesAsync(DesktopPreferences desktop)
    {
        await _saves.WaitAsync();
        var previous = Preferences.Desktop;
        try
        {
            _shortcut.Apply(desktop.Shortcut);
            try { await Preferences.SaveDesktopAsync(desktop); }
            catch { _shortcut.Apply(previous.Shortcut); throw; }
            ShortcutError = null;
            _main.AppWindow.IsShownInSwitchers = desktop.ShowTaskbarIcon || !CanReside;
            _hub?.Update(); SettingsChanged?.Invoke();
        }
        finally { _saves.Release(); }
    }
    public void ApplyAppearance(AppearanceMode? appearance = null)
    {
        var mode = appearance ?? Preferences.Appearance;
        foreach (var window in new Window?[] { _picker, _hub })
        {
            if (window?.Content is not FrameworkElement root) continue;
            root.RequestedTheme = mode switch
            { AppearanceMode.Light => ElementTheme.Light, AppearanceMode.Dark => ElementTheme.Dark, _ => ElementTheme.Default };
            try
            {
                // The native window frame does not inherit the XAML content theme,
                // even when the panel hides its title bar.
                window.AppWindow.TitleBar.PreferredTheme = mode switch
                {
                    AppearanceMode.Light => TitleBarTheme.Light,
                    AppearanceMode.Dark => TitleBarTheme.Dark,
                    _ => TitleBarTheme.UseDefaultAppMode
                };
            }
            catch (Exception error)
            {
                // Match the main window's behavior on shells without this property.
                Trace.WriteLine($"The panel frame theme could not be applied: {error.GetType().Name}");
            }
        }
        _main.AppWindow.IsShownInSwitchers = Preferences.Desktop.ShowTaskbarIcon || !CanReside;
    }
    public void OpenLibrary(string? id) { Dismiss(false); _ = _main.OpenResidentLibraryAsync(id); }
    public void OpenSettings() { Dismiss(false); _main.ShowResidentSettings(); }
    public void OpenTelegram() { Dismiss(false); _ = _main.ShowTelegramAsync(); }
    public void Quit() => _main.RequestQuit();
    public async Task ImportFilesAsync() { Dismiss(false); await _main.ImportResidentFilesAsync(); }
    public async Task ImportClipboardAsync()
    {
        try { await _main.ImportResidentAsync(token => StickerImportService.ReadAsync(Clipboard.GetContent(), token)); }
        catch (Exception error) { _surface?.ShowMessage(_main.Strings.ErrorKey(error)); }
    }
    public async Task ImportDropAsync(DataPackageView data)
    {
        try { await _main.ImportResidentAsync(token => StickerImportService.ReadAsync(data, token)); }
        catch (Exception error) { _surface?.ShowMessage(_main.Strings.ErrorKey(error)); }
    }
    public void ShowImportResult()
    {
        var model = _main.ViewModel;
        if (model.ImportMessageKey is not null && _active == _hub)
            _surface?.ShowFeedback(model.ImportText, model.ImportMessageKey is not ("import_done" or "import_done_reused" or "import_done_existing" or "import_cancelled"));
    }
    private void OnModelChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (_main.ViewModel.IsRestoring) Dismiss(false);
        if (_main.ViewModel.IsReady && args.PropertyName == nameof(ViewModels.LibraryViewModel.IsReady)) Rebind();
        _surface?.RefreshState();
        _hub?.Update();
    }
    private void OnCatalogChanged(object? sender, EventArgs args) { _picker?.Surface.Relocalize(); _hub?.Surface.Relocalize(); }
    private void OnContentChanged(object? sender, string? id)
    {
        if (_main.ViewModel.IsRestoring || _stopping) return;
        if (_surface?.Interacting == true) { _refreshPending = true; return; }
        if (_main.ViewModel.Picker is { Visible: true } picker) _ = picker.ReloadAsync(debounce: true);
        if (_main.ViewModel.Recents is { Visible: true } recents) _ = recents.ReloadAsync(debounce: true);
    }
    private void OnRebuilt(object? sender, EventArgs args) => Rebind();
    private void OnThumbnailInvalidated(object? sender, string id)
    {
        foreach (var model in new[] { _main.ViewModel.Picker, _main.ViewModel.Recents })
            if (model?.Cards.FirstOrDefault(card => card.Id == id) is { } card)
            { card.Thumbnail = null; model.Thumbnails.Invalidate(card); }
    }
    private void OnSyncChanged(object? sender, EventArgs args) => _main.DispatcherQueue.TryEnqueue(() => { if (!_stopping) _hub?.Update(); });
    private void OnLanguageChanged(object? sender, PropertyChangedEventArgs args) { _picker?.Surface.Relocalize(); _hub?.Relocalize(); }
    public async ValueTask DisposeAsync()
    {
        _stopping = true; _shortcut.Dispose(); _tray.Dispose();
        if (_foregroundHook != 0) DesktopNative.UnhookWinEvent(_foregroundHook);
        Dismiss(false);
        await Task.WhenAll(_presentation, _output);
        await _input.DisposeAsync();
        _picker?.Close(); _hub?.Close();
        _main.ViewModel.PropertyChanged -= OnModelChanged; _main.ViewModel.CatalogChanged -= OnCatalogChanged;
        _main.ViewModel.ContentChanged -= OnContentChanged; _main.ViewModel.Rebuilt -= OnRebuilt;
        _main.ViewModel.ThumbnailInvalidated -= OnThumbnailInvalidated;
        _main.Strings.PropertyChanged -= OnLanguageChanged;
        if (_sync is not null) _sync.StateChanged -= OnSyncChanged;
        _saves.Dispose();
    }
}
