using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;

namespace MemeDock.Windows.Views;

public sealed partial class QuickPickerWindow : Window
{
    public QuickPickerWindow()
    {
        InitializeComponent();
        ExtendsContentIntoTitleBar = true;
        SystemBackdrop = PanelAcrylicBackdrop.CreateNative();
        AppWindow.IsShownInSwitchers = false;
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.SetBorderAndTitleBar(false, false); presenter.IsResizable = false;
            presenter.IsMinimizable = false; presenter.IsMaximizable = false; presenter.IsAlwaysOnTop = true;
        }
    }
    internal StickerPickerView Surface => Picker;
}
