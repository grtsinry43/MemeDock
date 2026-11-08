namespace MemeDock.Windows.Services;

internal static class PanelFocus
{
    // Window.Activate alone does not bring a background application forward.
    // Verify the HWND instead of assuming that showing a topmost window focused it.
    internal static bool Activate(nint window, Action show, Func<nint, bool> foreground, Func<nint> current)
    {
        show();
        if (current() != window) foreground(window);
        return current() == window;
    }

    internal static bool ShouldDismiss(bool panelForeground, bool ownedForeground, bool menuOpen,
        bool dragging, bool modalOperation, bool pasting) =>
        !panelForeground && !dragging && !modalOperation && !pasting && !(ownedForeground && menuOpen);
}
