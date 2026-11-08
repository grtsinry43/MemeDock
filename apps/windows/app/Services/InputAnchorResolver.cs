using System.Collections.Concurrent;
using System.Runtime.InteropServices;

namespace MemeDock.Windows.Services;

internal sealed class InputAnchorResolver : IAsyncDisposable
{
    internal sealed record Snapshot(Guid Id, nint Window, nint Focus, int Process, ScreenRect? Bounds, bool Unsupported);
    private readonly BlockingCollection<Action> _queue = new(8);
    private readonly Task _worker;
    private IAutomation? _automation;
    private IElement? _field;
    private Guid _id;

    public InputAnchorResolver()
    {
        // A single long-lived MTA owns all COM references, including their release.
        _worker = Task.Factory.StartNew(() =>
        {
            try { foreach (var action in _queue.GetConsumingEnumerable()) action(); }
            finally { ReleaseField(); if (_automation is not null) Marshal.FinalReleaseComObject(_automation); }
        }, CancellationToken.None, TaskCreationOptions.LongRunning, TaskScheduler.Default);
    }
    private Task<T> Run<T>(Func<T> operation)
    {
        var completion = new TaskCompletionSource<T>(TaskCreationOptions.RunContinuationsAsynchronously);
        try
        {
            if (!_queue.TryAdd(() =>
            {
                try { completion.SetResult(operation()); }
                catch (Exception error) { completion.SetException(error); }
            })) completion.SetException(new InvalidOperationException("Input resolver is busy."));
        }
        catch (InvalidOperationException) { completion.TrySetException(new ObjectDisposedException(nameof(InputAnchorResolver))); }
        return completion.Task;
    }
    public Task<Snapshot> CaptureAsync(nint window) => Run(() =>
    {
        ReleaseField();
        _id = Guid.NewGuid();
        var process = DesktopNative.Process(window);
        var info = new DesktopNative.GuiThreadInfo { Size = (uint)Marshal.SizeOf<DesktopNative.GuiThreadInfo>() };
        var thread = DesktopNative.GetWindowThreadProcessId(window, out _);
        DesktopNative.GetGUIThreadInfo(thread, ref info);
        ScreenRect? bounds = null;
        var unsupported = process == Environment.ProcessId;
        if (!unsupported && process != 0)
        {
            try
            {
                if (_automation is null)
                {
                    _automation = (IAutomation)Activator.CreateInstance(Type.GetTypeFromCLSID(new("e22ad333-b25f-460c-83d0-0581107395c9"), true)!)!;
                    _automation.SetConnectionTimeout(100);
                    _automation.SetTransactionTimeout(100);
                }
                _automation.GetFocusedElement(out var focused);
                if (focused is not null)
                {
                    if (focused.Property(30002) is int focusedProcess && focusedProcess == process)
                    {
                        _field = focused;
                        unsupported = focused.Property(30019) is true || focused.Property(30010) is false;
                        // AutomationId is provider metadata, never the editor's value or name.
                        var automationId = focused.Property(30011) as string ?? "";
                        unsupported |= automationId.Contains("search", StringComparison.OrdinalIgnoreCase);
                        if (focused.Property(30003) is int control && control is 50004 or 50030 or 50003
                            && focused.Property(30001) is double[] rect && rect.Length == 4
                            && rect.All(double.IsFinite) && rect[2] > 0 && rect[3] > 0
                            && rect[0] >= int.MinValue && rect[1] >= int.MinValue
                            && rect[0] + rect[2] <= int.MaxValue && rect[1] + rect[3] <= int.MaxValue
                            && rect[2] <= int.MaxValue && rect[3] <= int.MaxValue)
                            bounds = new((int)rect[0], (int)rect[1], (int)rect[2], (int)rect[3]);
                    }
                    else Marshal.FinalReleaseComObject(focused);
                }
            }
            catch (COMException) { ReleaseField(); } // Custom editors can still accept ordinary Ctrl+V.
        }
        if (bounds is null && info.Caret != 0)
        {
            var point = new DesktopNative.Point { X = info.CaretRect.Left, Y = info.CaretRect.Top };
            if (DesktopNative.ClientToScreen(info.Caret, ref point))
                bounds = new(point.X, point.Y, 1, Math.Max(1, info.CaretRect.Bottom - info.CaretRect.Top));
        }
        var className = new System.Text.StringBuilder(256);
        DesktopNative.GetClassName(info.Focus, className, className.Capacity);
        unsupported |= className.ToString().Equals("Edit", StringComparison.OrdinalIgnoreCase);
        return new Snapshot(_id, window, info.Focus, process, bounds, unsupported);
    });
    public Task<bool> RestoreAsync(Snapshot snapshot) => Run(() =>
    {
        if (snapshot.Id != _id || DesktopNative.GetForegroundWindow() != snapshot.Window
            || DesktopNative.Process(snapshot.Window) != snapshot.Process) return false;
        try { _field?.SetFocus(); } catch (COMException) { return false; }
        return true;
    });
    public Task<bool> IsFocusedAsync(Snapshot snapshot) => Run(() =>
    {
        if (snapshot.Id != _id || DesktopNative.GetForegroundWindow() != snapshot.Window) return false;
        if (_field is not null && _automation is not null)
        {
            IElement? current = null;
            try { _automation.GetFocusedElement(out current); return current is not null && _automation.CompareElements(_field, current); }
            catch (COMException) { return false; }
            finally { if (current is not null) Marshal.FinalReleaseComObject(current); }
        }
        var info = new DesktopNative.GuiThreadInfo { Size = (uint)Marshal.SizeOf<DesktopNative.GuiThreadInfo>() };
        DesktopNative.GetGUIThreadInfo(DesktopNative.GetWindowThreadProcessId(snapshot.Window, out _), ref info);
        return snapshot.Focus == 0 || snapshot.Focus == info.Focus;
    });
    private void ReleaseField() { if (_field is not null) Marshal.FinalReleaseComObject(_field); _field = null; }
    public async ValueTask DisposeAsync() { _queue.CompleteAdding(); await _worker; _queue.Dispose(); }

    // Vtable gaps are CLR COM interop metadata, not callable methods. Slot order matches
    // UIAutomationClient.h in the pinned Windows 10.0.26100 SDK; only needed APIs are exposed.
    [ComImport, Guid("34723aff-0c9d-49d0-9896-7ab52df8cd8a"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IAutomation
    {
        [return: MarshalAs(UnmanagedType.Bool)] bool CompareElements(IElement first, IElement second);
        void _VtblGap1_4();
        void GetFocusedElement(out IElement? element);
        void _VtblGap2_52();
        void SetConnectionTimeout(uint milliseconds);
        void _VtblGap3_1();
        void SetTransactionTimeout(uint milliseconds);
    }
    [ComImport, Guid("d22108aa-8ac5-49a5-837b-37bbb3d7591e"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IElement
    {
        void SetFocus();
        void _VtblGap1_6();
        [return: MarshalAs(UnmanagedType.Struct)] object Property(int id);
    }
}
