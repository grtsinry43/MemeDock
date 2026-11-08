using System.ComponentModel;
using Shortcut = MemeDock.Windows.Models.GlobalShortcut;

namespace MemeDock.Windows.Services;

internal sealed class GlobalShortcut : IDisposable
{
    private readonly Func<int, Shortcut, bool> _register;
    private readonly Action<int> _unregister;
    public GlobalShortcut(nint window, Func<int, Shortcut, bool>? register = null, Action<int>? unregister = null)
    {
        _register = register ?? ((id, shortcut) => DesktopNative.RegisterHotKey(window, id, shortcut.Modifiers | 0x4000, shortcut.Key));
        _unregister = unregister ?? (id => DesktopNative.UnregisterHotKey(window, id));
    }
    private int _id = 74;
    private Shortcut? _current;
    public bool Handles(int id) => _current is not null && id == _id;
    public void Apply(Shortcut? shortcut)
    {
        if (shortcut == _current) return;
        if (shortcut is not null && !shortcut.IsValid) throw new ArgumentException("Invalid shortcut.");
        var nextId = _id == 74 ? 75 : 74;
        // Register before releasing the old binding: a conflict never loses a working shortcut.
        if (shortcut is not null && !_register(nextId, shortcut))
            throw new Win32Exception(System.Runtime.InteropServices.Marshal.GetLastWin32Error());
        if (_current is not null) _unregister(_id);
        (_id, _current) = (nextId, shortcut);
    }
    public void Dispose() { if (_current is not null) _unregister(_id); _current = null; }
}
