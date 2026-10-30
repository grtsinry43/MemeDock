using MemeDock.Windows.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace MemeDock.Windows.Views;

public sealed partial class NoticeBar : UserControl
{
    private static readonly TimeSpan ShortLife = TimeSpan.FromSeconds(4);
    private static readonly TimeSpan ActionLife = TimeSpan.FromSeconds(6);
    private readonly DispatcherQueueTimer _timer;
    private Func<Task>? _action;

    public NoticeBar()
    {
        InitializeComponent();
        _timer = DispatcherQueue.GetForCurrentThread().CreateTimer();
        _timer.IsRepeating = false;
        _timer.Tick += (_, _) => Hide();
        Motion.UseEntrance(this);
    }

    /// <summary>
    /// Replaces whatever is showing. Errors stay until dismissed; everything else
    /// leaves on its own, later when it offers an action such as undo.
    /// </summary>
    public void Show(string message, InfoBarSeverity severity, string? actionLabel = null, Func<Task>? action = null)
    {
        if (message.Length == 0) return;
        _timer.Stop();
        _action = action;
        Bar.Severity = severity;
        Bar.Message = message;
        ActionButton.Content = actionLabel;
        ActionButton.Visibility = action is null ? Visibility.Collapsed : Visibility.Visible;
        ActionButton.IsEnabled = true;
        if (actionLabel is not null) AutomationProperties.SetName(ActionButton, actionLabel);
        Visibility = Visibility.Visible;
        // The bar stays open while only its text changes, so screen readers are told directly.
        FrameworkElementAutomationPeer.CreatePeerForElement(Bar)?.RaiseNotificationEvent(
            AutomationNotificationKind.ActionCompleted, AutomationNotificationProcessing.ImportantMostRecent,
            message, "MemeDock.Notice");
        if (severity != InfoBarSeverity.Error)
        {
            _timer.Interval = action is null ? ShortLife : ActionLife;
            _timer.Start();
        }
    }

    public void Hide()
    {
        _timer.Stop();
        _action = null;
        Visibility = Visibility.Collapsed;
    }

    private void OnClosing(InfoBar sender, InfoBarClosingEventArgs args)
    {
        // The bar itself never closes; the host collapses so the exit animation can run.
        args.Cancel = true;
        Hide();
    }

    private async void OnAction(object sender, RoutedEventArgs args)
    {
        if (_action is not { } action) return;
        ActionButton.IsEnabled = false;
        Hide();
        await action();
    }
}
