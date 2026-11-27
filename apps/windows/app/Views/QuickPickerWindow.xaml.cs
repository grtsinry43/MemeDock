using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;

namespace MemeDock.Windows.Views;

public sealed partial class QuickPickerWindow : Window
{
    public QuickPickerWindow()
    {
        InitializeComponent();
        SystemBackdrop = PanelAcrylicBackdrop.CreateNative();
        AppWindow.IsShownInSwitchers = false;
        var presenter = OverlappedPresenter.CreateForContextMenu();
        presenter.IsAlwaysOnTop = true;
        AppWindow.SetPresenter(presenter);
    }
    internal StickerPickerView Surface => Picker;
}
