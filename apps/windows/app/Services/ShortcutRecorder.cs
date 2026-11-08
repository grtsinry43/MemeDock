using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;

namespace MemeDock.Windows.Services;

// Exists only for the recording dialog. No key events are inspected unless its
// capture button owns focus in the foreground window. Never records input text.
internal sealed class ShortcutRecorder : IDisposable
{
    private readonly Func<bool> _enabled;
    private readonly Action _changed;
    private readonly HookProc _callback;
    private readonly HashSet<uint> _suppressed = [];
    private nint _hook;
    internal ShortcutCapture Capture { get; } = new();
    internal ShortcutRecorder(Func<bool> enabled, Action changed)
    {
        (_enabled, _changed) = (enabled, changed);
        _callback = OnKey;
        _hook = SetWindowsHookEx(13, _callback, GetModuleHandle(null), 0);
        if (_hook == 0) throw new Win32Exception(Marshal.GetLastWin32Error());
    }
    internal void Begin() => Capture.Reset(new uint[] { 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0x5B, 0x5C }
        .Where(key => DesktopNative.GetAsyncKeyState((int)key) < 0));
    private nint OnKey(int code, nuint message, nint data)
    {
        try
        {
            if (code >= 0)
            {
                if (!_enabled())
                {
                    Capture.ReleaseModifiers();
                    return CallNextHookEx(_hook, code, message, data);
                }
                var key = (uint)Marshal.ReadInt32(data);
                var up = message is 0x101 or 0x105;
                if (up && _suppressed.Remove(key)) { Capture.KeyUp(key); return 1; }
                if ((Marshal.ReadInt32(data, 8) & 0x10) == 0)
                {
                    if (up) Capture.KeyUp(key);
                    else
                    {
                        var handled = Capture.KeyDown(key);
                        _changed();
                        if (handled || key is 0x5B or 0x5C) { _suppressed.Add(key); return 1; }
                    }
                }
            }
        }
        catch (Exception error) { Trace.WriteLine($"Shortcut capture: {error.GetType().Name}"); }
        return CallNextHookEx(_hook, code, message, data);
    }
    public void Dispose()
    {
        if (_hook == 0) return;
        UnhookWindowsHookEx(_hook); _hook = 0;
    }
    private delegate nint HookProc(int code, nuint message, nint data);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)] private static extern nint SetWindowsHookEx(int hook, HookProc callback, nint module, uint thread);
    [DllImport("user32.dll")] private static extern nint CallNextHookEx(nint hook, int code, nuint message, nint data);
    [DllImport("user32.dll")] private static extern bool UnhookWindowsHookEx(nint hook);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] private static extern nint GetModuleHandle(string? name);
}
