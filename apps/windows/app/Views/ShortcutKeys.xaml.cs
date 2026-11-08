using System.Runtime.InteropServices;
using System.Text;
using MemeDock.Windows.Models;
using Microsoft.UI.Xaml.Controls;

namespace MemeDock.Windows.Views;

public sealed partial class ShortcutKeys : UserControl
{
    public ShortcutKeys() => InitializeComponent();
    internal void SetShortcut(GlobalShortcut? shortcut) => SetKeys(shortcut?.Keys ?? []);
    internal void SetKeys(IEnumerable<uint> keys)
    {
        var labels = keys.Select(KeyLabel).ToArray();
        Keys.ItemsSource = labels;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(this, string.Join(" + ", labels));
    }
    private static string KeyLabel(uint key)
    {
        if (key is 0x10 or 0x11 or 0x12 or 0x5B or 0x5C or >= 0x25 and <= 0x28 or >= 0x60 and <= 0x6F)
            return GlobalShortcut.KeyName(key);
        var scan = MapVirtualKey(key, 4);
        var parameter = (int)((scan & 0xFF) << 16) | (1 << 25);
        if ((scan & 0xFF00) == 0xE000) parameter |= 1 << 24;
        var text = new StringBuilder(64);
        return scan != 0 && GetKeyNameText(parameter, text, text.Capacity) > 0 ? text.ToString() : GlobalShortcut.KeyName(key);
    }
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern uint MapVirtualKey(uint key, uint map);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetKeyNameText(int parameter, StringBuilder text, int size);
}
