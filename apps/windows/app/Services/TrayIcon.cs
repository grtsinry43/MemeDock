using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using MemeDock.Windows.Localization;
using Microsoft.UI.Dispatching;

namespace MemeDock.Windows.Services;

internal sealed class TrayIcon : IDisposable
{
    private const uint CallbackMessage = 0x8000 + 73;
    private readonly nint _window;
    private readonly DesktopNative.SubclassProc _callback;
    private readonly uint _taskbarCreated = DesktopNative.RegisterWindowMessage("TaskbarCreated");
    private readonly nint _icon;
    private readonly TrayRegistration _registration;
    private readonly DispatcherQueueTimer _retry;
    private int _attempts;
    private bool _disposed;
    public event Action? Selected;
    public event Action<int>? Command;
    public event Action<int>? Hotkey;
    public event Action? ContextOpening;
    public event Action<bool>? AvailabilityChanged;
    private readonly LocalizationService _strings;

    public TrayIcon(nint window, LocalizationService strings)
    {
        (_window, _strings) = (window, strings);
        _icon = DesktopNative.LoadImage(0, Path.Combine(AppContext.BaseDirectory, "Assets", "MemeDock.ico"), 1, 32, 32, 0x10);
        if (_icon == 0) throw new Win32Exception(Marshal.GetLastWin32Error());
        _callback = WndProc;
        if (!DesktopNative.SetWindowSubclass(window, _callback, 73, 0))
        {
            DesktopNative.DestroyIcon(_icon);
            throw new InvalidOperationException("Could not install the desktop message handler.");
        }
        _registration = new TrayRegistration(Notify);
        _retry = DispatcherQueue.GetForCurrentThread().CreateTimer();
        _retry.Interval = TimeSpan.FromSeconds(2);
        _retry.Tick += (_, _) => Install();
        Install();
    }

    public bool Available => _registration.Available;
    private NotifyIconData Data(bool useGuid) => new()
    {
        Size = (uint)Marshal.SizeOf<NotifyIconData>(), Window = _window, Id = 1,
        Flags = 1 | 2 | 4 | 0x80 | (useGuid ? 0x20u : 0), Callback = CallbackMessage, Icon = _icon, Tip = "MemeDock",
        Guid = useGuid ? new Guid("CB8E99D2-A264-4396-915A-B8A5CA6BB44B") : Guid.Empty, Info = "", Title = ""
    };
    private bool Notify(TrayOperation operation, bool useGuid)
    {
        var data = Data(useGuid);
        if (operation == TrayOperation.SetVersion) data.Version = 4;
        var result = ShellNotifyIcon((uint)operation, ref data);
        Trace.WriteLine($"Tray {operation}: success={result}, identity={(useGuid ? "guid" : "window/id")}, size={data.Size}, version={data.Version}");
        return result;
    }
    private void Install()
    {
        if (_disposed) return;
        var available = _registration.Install();
        if (available || ++_attempts >= 5) _retry.Stop();
        else _retry.Start();
        AvailabilityChanged?.Invoke(available);
    }
    public ScreenRect Bounds()
    {
        var id = new NotifyIconIdentifier { Size = (uint)Marshal.SizeOf<NotifyIconIdentifier>(), Window = _window, Id = 1, Guid = Data(_registration.UsesGuid).Guid };
        if (ShellNotifyIconGetRect(ref id, out var rect) == 0) return rect.Bounds;
        DesktopNative.GetCursorPos(out var point);
        return new(point.X, point.Y, 1, 1);
    }
    private nint WndProc(nint window, uint message, nuint wParam, nint lParam, nuint id, nuint data)
    {
        // Exceptions must never escape a native callback.
        try
        {
            if (!_disposed && message == _taskbarCreated)
            {
                _registration.TaskbarRestarted();
                _attempts = 0;
                Install();
            }
            else if (!_disposed && message == 0x312) Hotkey?.Invoke((int)wParam);
            else if (!_disposed && message == CallbackMessage)
            {
                var notification = _registration.Version4 ? (uint)(lParam.ToInt64() & 0xFFFF) : (uint)lParam.ToInt64();
                if (_registration.Version4 && notification is 0x400 or 0x401
                    || !_registration.Version4 && notification == 0x202) Selected?.Invoke();
                else if (notification == 0x7B || !_registration.Version4 && notification == 0x205) ShowMenu();
            }
        }
        catch (Exception error) { Trace.WriteLine($"Desktop callback: {error.GetType().Name}"); }
        return DesktopNative.DefSubclassProc(window, message, wParam, lParam);
    }
    private void ShowMenu()
    {
        var menu = DesktopNative.CreatePopupMenu();
        if (menu == 0) return;
        try
        {
            foreach (var (id, key) in new[] { (1, "picker_title"), (2, "picker_open_library"), (3, "library_add"), (4, "desktop_settings") })
                DesktopNative.AppendMenu(menu, 0, (nuint)id, _strings[key]);
            DesktopNative.AppendMenu(menu, 0x800, 0, null);
            DesktopNative.AppendMenu(menu, 0, 5, _strings["desktop_quit"]);
            DesktopNative.GetCursorPos(out var point);
            ContextOpening?.Invoke();
            DesktopNative.SetForegroundWindow(_window);
            var result = DesktopNative.TrackPopupMenuEx(menu, 0x100 | 0x2, point.X, point.Y, _window, 0);
            DesktopNative.PostMessage(_window, 0, 0, 0);
            if (result != 0) Command?.Invoke((int)result);
        }
        finally { DesktopNative.DestroyMenu(menu); }
    }
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _retry.Stop();
        _registration.Dispose();
        DesktopNative.RemoveWindowSubclass(_window, _callback, 73);
        DesktopNative.DestroyIcon(_icon);
    }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] private struct NotifyIconData
    {
        public uint Size;
        public nint Window;
        public uint Id, Flags, Callback;
        public nint Icon;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string Tip;
        public uint State, StateMask;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string Info;
        public uint Version;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)] public string Title;
        public uint InfoFlags;
        public Guid Guid;
        public nint BalloonIcon;
    }
    [StructLayout(LayoutKind.Sequential)] private struct NotifyIconIdentifier
    { public uint Size; public nint Window; public uint Id; public Guid Guid; }
    [DllImport("shell32.dll", EntryPoint = "Shell_NotifyIconW", CharSet = CharSet.Unicode)] private static extern bool ShellNotifyIcon(uint message, ref NotifyIconData data);
    [DllImport("shell32.dll", EntryPoint = "Shell_NotifyIconGetRect")] private static extern int ShellNotifyIconGetRect(ref NotifyIconIdentifier id, out DesktopNative.Rect rect);
}
