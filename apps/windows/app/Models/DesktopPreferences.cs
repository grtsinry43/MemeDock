namespace MemeDock.Windows.Models;

public sealed record GlobalShortcut(uint Modifiers, uint Key)
{
    public bool IsValid => Modifiers is > 0 and <= 15 && IsBindableKey(Key);
    public static bool IsBindableKey(uint key) => key is 0x08 or 0x09 or 0x0D or 0x13 or 0x14 or 0x1B
        or >= 0x20 and <= 0x28 or >= 0x2C and <= 0x2E
        or >= 0x30 and <= 0x39 or >= 0x41 and <= 0x5A
        or >= 0x60 and <= 0x6F or >= 0x70 and <= 0x7A or >= 0x7C and <= 0x87
        or 0x90 or 0x91 or >= 0xBA and <= 0xC0 or >= 0xDB and <= 0xDF or 0xE2;
    [System.Text.Json.Serialization.JsonIgnore]
    public uint[] Keys => new[]
    {
        (Modifiers & 8) != 0 ? 0x5Bu : 0, (Modifiers & 2) != 0 ? 0x11u : 0,
        (Modifiers & 1) != 0 ? 0x12u : 0, (Modifiers & 4) != 0 ? 0x10u : 0, Key
    }.Where(key => key != 0).ToArray();
    public override string ToString() => string.Join("+", new[]
    {
        (Modifiers & 8) != 0 ? "Win" : null, (Modifiers & 2) != 0 ? "Ctrl" : null, (Modifiers & 1) != 0 ? "Alt" : null,
        (Modifiers & 4) != 0 ? "Shift" : null,
        KeyName(Key)
    }.Where(value => value is not null));
    public static string KeyName(uint key) => key switch
    {
        0x08 => "Backspace", 0x09 => "Tab", 0x0D => "Enter", 0x10 => "Shift", 0x11 => "Ctrl", 0x12 => "Alt",
        0x13 => "Pause", 0x14 => "Caps Lock", 0x1B => "Esc", 0x20 => "Space", 0x21 => "Page Up", 0x22 => "Page Down",
        0x23 => "End", 0x24 => "Home", 0x25 => "←", 0x26 => "↑", 0x27 => "→", 0x28 => "↓",
        0x2C => "Print Screen", 0x2D => "Insert", 0x2E => "Delete", 0x5B or 0x5C => "Win",
        >= 0x30 and <= 0x39 or >= 0x41 and <= 0x5A => ((char)key).ToString(),
        >= 0x60 and <= 0x69 => $"Num {key - 0x60}", 0x6A => "Num *", 0x6B => "Num +", 0x6C => "Num Separator",
        0x6D => "Num -", 0x6E => "Num .", 0x6F => "Num /", >= 0x70 and <= 0x87 => $"F{key - 0x6F}",
        0x90 => "Num Lock", 0x91 => "Scroll Lock", 0xBA => ";", 0xBB => "=", 0xBC => ",", 0xBD => "-",
        0xBE => ".", 0xBF => "/", 0xC0 => "`", 0xDB => "[", 0xDC => "\\", 0xDD => "]", 0xDE => "'",
        _ => $"0x{key:X2}"
    };
}

public sealed record DesktopPreferences(bool ShowTaskbarIcon = true, bool AnchorToInput = true,
    GlobalShortcut? Shortcut = null, bool ShortcutHintDismissed = false)
{
    public DesktopPreferences Sanitized() => this with { Shortcut = Shortcut?.IsValid == true ? Shortcut : null };
}

public static class ResidentLifetime
{
    public static bool ShouldHide(bool quitRequested, bool trayAvailable) => !quitRequested && trayAvailable;
}
