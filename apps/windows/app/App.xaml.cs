using Microsoft.UI.Xaml;

namespace MemeDock.Windows;

public partial class App : Application
{
    private Window? _window;

    public App() => InitializeComponent();

    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        var instance = Microsoft.Windows.AppLifecycle.AppInstance.FindOrRegisterForKey("MemeDock.Library");
        if (!instance.IsCurrent)
        {
            await instance.RedirectActivationToAsync(Microsoft.Windows.AppLifecycle.AppInstance.GetCurrent().GetActivatedEventArgs());
            Exit();
            return;
        }
        instance.Activated += (_, _) =>
        {
            if (_window is MainWindow main) main.DispatcherQueue.TryEnqueue(main.ShowResidentWindow);
        };
        _window = new MainWindow();
        _window.Activate();
    }
}
