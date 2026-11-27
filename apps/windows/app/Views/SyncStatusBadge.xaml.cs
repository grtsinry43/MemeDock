using System.Diagnostics;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace MemeDock.Windows.Views;

public sealed partial class SyncStatusBadge : UserControl
{
    private SyncCoordinator? _sync;
    private LocalizationService? _strings;
    private CancellationTokenSource? _settleCancellation;
    private bool _wasActive;
    private bool _recentlySynced;
    private bool _spinning;
    private string _lastVisualKey = "";
    private bool _navigationEnabled = true;

    public event EventHandler? OpenSettingsRequested;

    public SyncStatusBadge()
    {
        InitializeComponent();
        Motion.UseCrossFade(StatusButton);
    }

    public void Configure(LocalizationService strings)
    {
        _strings = strings;
        Refresh();
    }

    public void Attach(SyncCoordinator? sync)
    {
        if (ReferenceEquals(_sync, sync)) return;
        if (_sync is not null) _sync.StateChanged -= OnSyncStateChanged;
        _sync = sync;
        _wasActive = false;
        _recentlySynced = false;
        CancelSettle();
        if (_sync is not null) _sync.StateChanged += OnSyncStateChanged;
        Refresh();
    }

    public void SetNavigationEnabled(bool enabled)
    {
        _navigationEnabled = enabled;
        StatusButton.IsEnabled = enabled;
        SettingsActionButton.IsEnabled = enabled;
    }

    public void Refresh()
    {
        if (_strings is null || _sync?.State is not { } state || !SyncPresentation.IsVisible(state))
        {
            StopSpin();
            StatusButton.Visibility = Visibility.Collapsed;
            _lastVisualKey = "";
            return;
        }

        var isActive = SyncPresentation.IsActiveTransfer(state);
        if (isActive)
        {
            CancelSettle();
            _recentlySynced = false;
        }
        else if (SyncPresentation.ShouldCelebrateCompletion(_wasActive, state))
        {
            StartSettle();
        }
        _wasActive = isActive;

        var phase = SyncPresentation.Phase(state);
        var effectiveRunning = phase == SyncPhase.Running || (isActive && phase is SyncPhase.UpToDate or SyncPhase.Pending or SyncPhase.Unsynced);

        string label;
        string visualKey;
        if (effectiveRunning)
        {
            label = state.Status.Downloading > 0
                ? $"{_strings["sync_running"]} · {state.Status.Downloading}"
                : _strings["sync_running"];
            visualKey = $"running:{state.Status.Downloading > 0}";
        }
        else if (_recentlySynced && phase == SyncPhase.UpToDate)
        {
            label = _strings["sync_ready"];
            visualKey = "settled";
        }
        else if (phase == SyncPhase.Pending && state.Status.Pending > 0)
        {
            label = $"{_strings["sync_pending"]} · {state.Status.Pending}";
            visualKey = $"pending:{state.Status.Pending}";
        }
        else
        {
            label = _strings[SyncPresentation.StatusKey(state)];
            visualKey = $"phase:{phase}";
        }

        var brush = ResolveBrush(phase, effectiveRunning, _recentlySynced);
        StatusLabel.Text = label;
        if (brush is not null)
        {
            StatusLabel.Foreground = brush;
            StatusIcon.Foreground = brush;
            StatusDot.Fill = brush;
        }

        if (effectiveRunning)
        {
            StatusIcon.Glyph = "\uE895";
            StatusIcon.Visibility = Visibility.Visible;
            StatusDot.Visibility = Visibility.Collapsed;
            StartSpin();
        }
        else
        {
            StopSpin();
            if (_recentlySynced && phase == SyncPhase.UpToDate)
            {
                StatusIcon.Glyph = "\uE73E";
                StatusIcon.Visibility = Visibility.Visible;
                StatusDot.Visibility = Visibility.Collapsed;
            }
            else if (phase is SyncPhase.UpToDate or SyncPhase.Pending)
            {
                StatusIcon.Visibility = Visibility.Collapsed;
                StatusDot.Visibility = Visibility.Visible;
            }
            else
            {
                StatusIcon.Glyph = phase switch
                {
                    SyncPhase.Authorization => "\uE72E",
                    SyncPhase.Paused => "\uE769",
                    SyncPhase.Blocked => "\uE7BA",
                    SyncPhase.Failed => "\uE783",
                    _ => "\uE917"
                };
                StatusIcon.Visibility = Visibility.Visible;
                StatusDot.Visibility = Visibility.Collapsed;
            }
        }

        if (_lastVisualKey.Length > 0 && _lastVisualKey != visualKey)
            Motion.AnimateContentSwap(StatusLabel, 5f);
        _lastVisualKey = visualKey;

        var hint = SyncPresentation.HintKey(state);
        ToolTipService.SetToolTip(StatusButton, hint is null ? label : _strings[hint]);
        AutomationProperties.SetName(StatusButton, label);
        StatusButton.IsEnabled = _navigationEnabled;
        StatusButton.Visibility = Visibility.Visible;

        if (StatusFlyout.IsOpen) PopulateFlyout(state, phase, effectiveRunning, brush);
    }

    private Brush? ResolveBrush(SyncPhase phase, bool effectiveRunning, bool settled)
    {
        if (effectiveRunning) return BadgeRoot.Resources["SyncIdleBrush"] as Brush;
        if (settled && phase == SyncPhase.UpToDate) return BadgeRoot.Resources["SyncSuccessBrush"] as Brush;
        return phase switch
        {
            SyncPhase.Failed or SyncPhase.Authorization => BadgeRoot.Resources["SyncFailureBrush"] as Brush,
            SyncPhase.Blocked or SyncPhase.Pending or SyncPhase.Unsynced => BadgeRoot.Resources["SyncAttentionBrush"] as Brush,
            SyncPhase.UpToDate => BadgeRoot.Resources["SyncSuccessBrush"] as Brush,
            _ => BadgeRoot.Resources["SyncIdleBrush"] as Brush
        };
    }

    private void PopulateFlyout(SyncState state, SyncPhase phase, bool effectiveRunning, Brush? brush)
    {
        if (_strings is null) return;
        FlyoutHeading.Text = effectiveRunning ? _strings["sync_running"] : _strings[SyncPresentation.StatusKey(state)];
        FlyoutIcon.Glyph = effectiveRunning ? "\uE895" : phase switch
        {
            SyncPhase.Authorization => "\uE72E",
            SyncPhase.Paused => "\uE769",
            SyncPhase.Blocked => "\uE7BA",
            SyncPhase.Failed => "\uE783",
            SyncPhase.UpToDate => "\uE73E",
            _ => "\uE917"
        };
        if (brush is not null) FlyoutIcon.Foreground = brush;

        var isChinese = _strings.Culture.TwoLetterISOLanguageName == "zh";
        LastSyncedLabel.Text = _strings["sync_last_success"];
        if (state.Status.LastSuccessMs is > 0 and var ms)
        {
            LastSyncedValue.Text = SyncPresentation.FormatLastSynced(ms, DateTimeOffset.UtcNow, isChinese);
            LastSyncedRow.Visibility = Visibility.Visible;
        }
        else
        {
            LastSyncedRow.Visibility = Visibility.Collapsed;
        }

        PendingLabel.Text = _strings["sync_pending"];
        PendingValue.Text = state.Status.Pending.ToString();
        PendingRow.Visibility = MainWindow.ToVisibility(state.Status.Pending > 0);

        DownloadingLabel.Text = _strings["sync_downloading"];
        DownloadingValue.Text = state.Status.Downloading.ToString();
        DownloadingRow.Visibility = MainWindow.ToVisibility(state.Status.Downloading > 0);

        BlockedLabel.Text = _strings["sync_blocked"];
        BlockedValue.Text = state.Status.Blocked.ToString();
        BlockedRow.Visibility = MainWindow.ToVisibility(state.Status.Blocked > 0);

        StatsPanel.Visibility = MainWindow.ToVisibility(
            LastSyncedRow.Visibility == Visibility.Visible
            || PendingRow.Visibility == Visibility.Visible
            || DownloadingRow.Visibility == Visibility.Visible
            || BlockedRow.Visibility == Visibility.Visible);

        var hintKey = SyncPresentation.HintKey(state);
        FlyoutHint.Text = hintKey is null ? "" : _strings[hintKey];
        FlyoutHint.Visibility = MainWindow.ToVisibility(hintKey is not null);

        var canAction = state.Loaded && state.Status.Connected && state.HasCredential && !SyncPresentation.IsTerminal(state);
        PrimaryActionButton.Visibility = MainWindow.ToVisibility(canAction);
        PrimaryActionButton.Content = _strings[state.Status.Paused ? "sync_resume" : "sync_now"];
        PrimaryActionButton.IsEnabled = canAction && !state.Running && !state.Changing;

        SettingsActionButton.Content = _strings["desktop_settings"];
        SettingsActionButton.IsEnabled = _navigationEnabled;
    }

    private void OnFlyoutOpening(object? sender, object args)
    {
        if (_sync?.State is not { } state) return;
        var phase = SyncPresentation.Phase(state);
        var effectiveRunning = SyncPresentation.IsActiveTransfer(state);
        PopulateFlyout(state, phase, effectiveRunning, ResolveBrush(phase, effectiveRunning, _recentlySynced));
    }

    private async void OnPrimaryAction(object sender, RoutedEventArgs args)
    {
        if (_sync is not { } sync) return;
        StatusFlyout.Hide();
        try
        {
            if (sync.State.Status.Paused) await sync.SetPausedAsync(false);
            else await sync.SynchronizeNowAsync();
        }
        catch (Exception error) { Trace.WriteLine($"Quick sync action failed: {error}"); }
    }

    private void OnOpenSettings(object sender, RoutedEventArgs args)
    {
        StatusFlyout.Hide();
        OpenSettingsRequested?.Invoke(this, EventArgs.Empty);
    }

    private void OnSyncStateChanged(object? sender, EventArgs args)
    {
        DispatcherQueue?.TryEnqueue(Refresh);
    }

    private void StartSpin()
    {
        if (_spinning) return;
        _spinning = true;
        Motion.StartContinuousSpin(StatusIcon);
    }

    private void StopSpin()
    {
        if (!_spinning) return;
        _spinning = false;
        Motion.StopSpin(StatusIcon);
    }

    private void StartSettle()
    {
        CancelSettle();
        _recentlySynced = true;
        var cancellation = new CancellationTokenSource();
        _settleCancellation = cancellation;
        _ = SettleAfterDelayAsync(cancellation);
    }

    private async Task SettleAfterDelayAsync(CancellationTokenSource cancellation)
    {
        try
        {
            await Task.Delay(2400, cancellation.Token);
            if (!cancellation.IsCancellationRequested)
            {
                _recentlySynced = false;
                Refresh();
            }
        }
        catch (OperationCanceledException) { }
    }

    private void CancelSettle()
    {
        _settleCancellation?.Cancel();
        _settleCancellation?.Dispose();
        _settleCancellation = null;
    }

    private void OnUnloaded(object sender, RoutedEventArgs args)
    {
        StopSpin();
        CancelSettle();
    }
}
