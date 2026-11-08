using System.Runtime.InteropServices;

namespace MemeDock.Windows.Services;

internal sealed class InputPasteSession : IAsyncDisposable
{
    private readonly InputAnchorResolver _resolver = new();
    private InputAnchorResolver.Snapshot? _snapshot;
    private long _version;
    public bool InProgress { get; private set; }
    public async Task<ScreenRect?> CaptureAsync(nint window)
    {
        var version = ++_version;
        _snapshot = null;
        var snapshot = await _resolver.CaptureAsync(window);
        if (version != _version) return null;
        _snapshot = snapshot;
        return snapshot.Bounds;
    }
    public void Reset() { ++_version; _snapshot = null; }
    public async Task<string?> PasteAsync(uint clipboardSequence, Action hide)
    {
        if (InProgress) return "failure_busy";
        var target = _snapshot;
        if (target is null) return "picker_paste_missing";
        var version = _version;
        string? Check()
        {
            if (version != _version) return "picker_paste_changed";
            var foreground = DesktopNative.GetForegroundWindow();
            return PastePolicy.Refusal(target.Process, Environment.ProcessId,
                DesktopNative.IsWindow(target.Window) && DesktopNative.Process(target.Window) == target.Process,
                target.Unsupported, foreground == target.Window || DesktopNative.Process(foreground) == Environment.ProcessId,
                clipboardSequence, DesktopNative.GetClipboardSequenceNumber());
        }
        if (Check() is { } refusal) return refusal;
        InProgress = true;
        try
        {
            hide();
            if (!DesktopNative.SetForegroundWindow(target.Window)) return "picker_paste_missing";
            var deadline = Environment.TickCount64 + 900;
            while (DesktopNative.GetForegroundWindow() != target.Window)
            {
                if (Check() is { } changed) return changed;
                if (Environment.TickCount64 >= deadline) return "picker_paste_missing";
                await Task.Delay(20);
            }
            await _resolver.RestoreAsync(target);
            while (!await _resolver.IsFocusedAsync(target) || !ModifiersReleased())
            {
                if (Check() is { } changed) return changed;
                if (Environment.TickCount64 >= deadline) return "picker_paste_missing";
                await Task.Delay(20);
            }
            if (Check() is { } last) return last;
            if (DesktopNative.GetForegroundWindow() != target.Window) return "picker_paste_changed";
            var events = new[] { Key(0x11, false), Key(0x56, false), Key(0x56, true), Key(0x11, true) };
            var sent = DesktopNative.SendInput(4, events, Marshal.SizeOf<DesktopNative.Input>());
            if (sent != 4)
            {
                // A partial insertion must not leave synthetic Ctrl held down. Never retry V.
                DesktopNative.SendInput(1, [Key(0x11, true)], Marshal.SizeOf<DesktopNative.Input>());
                return "picker_paste_blocked";
            }
            return null;
        }
        finally { InProgress = false; }
    }
    private static bool ModifiersReleased() => new[] { 0x10, 0x11, 0x12, 0x5B, 0x5C }.All(key => DesktopNative.GetAsyncKeyState(key) >= 0);
    private static DesktopNative.Input Key(ushort key, bool up) => new()
    { Type = 1, Data = new() { Keyboard = new() { Key = key, Flags = up ? 2u : 0u } } };
    public async ValueTask DisposeAsync() { Reset(); await _resolver.DisposeAsync(); }
}
