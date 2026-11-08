using Microsoft.UI.Composition;
using System.Diagnostics;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;

namespace MemeDock.Windows.Views;

// Use the more opaque Base Acrylic preset so content behind the picker does not
// compete with stickers and search text. Tint, blur, theme,
// high contrast and transparency-policy fallback all remain system controlled.
internal sealed class PanelAcrylicBackdrop : SystemBackdrop
{
    private DesktopAcrylicController? _controller;

    internal static SystemBackdrop CreateNative() => DesktopAcrylicController.IsSupported()
        ? new PanelAcrylicBackdrop() : new DesktopAcrylicBackdrop();

    protected override void OnTargetConnected(ICompositionSupportsSystemBackdrop target, XamlRoot root)
    {
        base.OnTargetConnected(target, root);
        if (_controller is not null) throw new InvalidOperationException("A panel backdrop cannot be shared between windows.");
        var controller = new DesktopAcrylicController { Kind = DesktopAcrylicKind.Base };
        try
        {
            controller.SetSystemBackdropConfiguration(GetDefaultSystemBackdropConfiguration(target, root));
            if (!controller.AddSystemBackdropTarget(target))
            {
                Trace.WriteLine("Panel Acrylic: Windows rejected the backdrop target.");
                controller.Dispose();
                return;
            }
            Trace.WriteLine($"Panel Acrylic connected: inputActive={GetDefaultSystemBackdropConfiguration(target, root).IsInputActive}");
            _controller = controller;
        }
        catch { controller.Dispose(); throw; }
    }
    protected override void OnTargetDisconnected(ICompositionSupportsSystemBackdrop target)
    {
        try { _controller?.RemoveSystemBackdropTarget(target); }
        finally
        {
            _controller?.Dispose();
            _controller = null;
            base.OnTargetDisconnected(target);
        }
    }
}
