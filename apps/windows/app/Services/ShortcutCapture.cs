using Shortcut = MemeDock.Windows.Models.GlobalShortcut;

namespace MemeDock.Windows.Services;

internal sealed class ShortcutCapture
{
    private readonly HashSet<uint> _modifiers = [];
    public Shortcut? Candidate { get; private set; }
    public uint Modifiers => _modifiers.Aggregate(0u, (mask, key) => mask | Modifier(key));
    public static uint Modifier(uint key) => key switch
    {
        0x10 or 0xA0 or 0xA1 => 4, 0x11 or 0xA2 or 0xA3 => 2,
        0x12 or 0xA4 or 0xA5 => 1, 0x5B or 0x5C => 8, _ => 0
    };
    public void Reset(IEnumerable<uint> held)
    {
        _modifiers.Clear();
        foreach (var key in held) if (Modifier(key) != 0) _modifiers.Add(key);
        Candidate = null;
    }
    public bool KeyDown(uint key)
    {
        if (Modifier(key) != 0) { if (_modifiers.Add(key)) Candidate = null; return false; }
        // Preserve dialog navigation and cancellation. Modified Tab/Esc remain recordable.
        if (key == 0x09 && (Modifiers & ~4u) == 0 || key is 0x1B or 0x0D && Modifiers == 0) return false;
        var candidate = new Shortcut(Modifiers, key);
        Candidate = candidate.IsValid ? candidate : null;
        return true;
    }
    public void KeyUp(uint key) => _modifiers.Remove(key);
    public void ReleaseModifiers() => _modifiers.Clear();
}
