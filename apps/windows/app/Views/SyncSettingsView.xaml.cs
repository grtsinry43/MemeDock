using System.Diagnostics;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace MemeDock.Windows.Views;

public sealed partial class SyncSettingsView : UserControl
{
    private LocalizationService? _strings;
    private SyncCoordinator? _sync;
    private bool _editing;
    private bool _closed;
    private string? _localErrorKey;

    public SyncSettingsView() => InitializeComponent();

    public void Configure(SyncCoordinator? sync, LocalizationService strings)
    {
        if (!ReferenceEquals(_sync, sync))
        {
            if (_sync is not null) _sync.StateChanged -= OnStateChanged;
            _sync = sync;
            if (_sync is not null) _sync.StateChanged += OnStateChanged;
        }
        _strings = strings;
        _closed = false;
        _editing = false;
        _localErrorKey = null;
        Relocalize();
    }

    public void Relocalize()
    {
        if (_strings is null) return;
        SyncLabel.Text = _strings["sync_title"];
        OriginBox.Header = _strings["sync_server"];
        TokenBox.Header = _strings["sync_token"];
        TokenHint.Text = _strings["sync_token_hint"];
        ConnectButton.Content = _strings["sync_connect"];
        CancelButton.Content = _strings["cancel"];
        InlineDisconnectButton.Content = _strings["sync_disconnect"];
        SyncNowButton.Content = _strings["sync_now"];
        TokenButton.Content = _strings["sync_reauthorize"];
        DisconnectButton.Content = _strings["sync_disconnect"];
        ServerLabel.Text = _strings["sync_server"];
        PendingLabel.Text = _strings["sync_pending"];
        BlockedLabel.Text = _strings["sync_blocked"];
        LastSyncLabel.Text = _strings["sync_last_success"];
        BlockedHint.Text = _strings["sync_blocked_hint"];
        MergeHint.Text = _strings["sync_merge_hint"];
        BackgroundHint.Text = _strings["sync_background_hint"];
        Render();
    }

    /// <summary>Stops touching the coordinator before the library is closed.</summary>
    public void Shutdown()
    {
        _closed = true;
        if (_sync is not null) _sync.StateChanged -= OnStateChanged;
        _sync = null;
    }

    private void OnStateChanged(object? sender, EventArgs e) => DispatcherQueue?.TryEnqueue(Render);

    private void Render()
    {
        if (_strings is null || _closed) return;
        var state = _sync?.State;
        if (state is null)
        {
            Visibility = Visibility.Collapsed;
            return;
        }
        Visibility = Visibility.Visible;

        var needsAuthorization = SyncPresentation.NeedsAuthorization(state);
        var showForm = needsAuthorization || _editing;
        var busy = state.Running || state.Changing;
        var connected = state.Status.Connected;

        BusyRing.IsActive = busy;
        BusyRing.Visibility = busy ? Visibility.Visible : Visibility.Collapsed;
        StatusText.Text = _strings[SyncPresentation.StatusKey(state)];

        ConnectForm.Visibility = showForm ? Visibility.Visible : Visibility.Collapsed;
        StatusPanel.Visibility = connected && !showForm ? Visibility.Visible : Visibility.Collapsed;
        CancelButton.Visibility = _editing ? Visibility.Visible : Visibility.Collapsed;
        InlineDisconnectButton.Visibility = connected && showForm ? Visibility.Visible : Visibility.Collapsed;
        ConnectButton.IsEnabled = !busy;
        OriginBox.IsEnabled = !busy;
        TokenBox.IsEnabled = !busy;

        if (showForm && OriginBox.Text.Length == 0 && state.Status.Origin is { Length: > 0 } origin)
            OriginBox.Text = origin;

        PendingValue.Text = state.Status.Pending.ToString(_strings.Culture);
        BlockedValue.Text = state.Status.Blocked.ToString(_strings.Culture);
        OriginValue.Text = state.Status.Origin ?? "";
        LastSyncValue.Text = state.Status.LastSuccessMs is long milliseconds && milliseconds > 0
            ? DateTimeOffset.FromUnixTimeMilliseconds(milliseconds).ToLocalTime().ToString("g", _strings.Culture)
            : _strings["sync_never"];
        BlockedHint.Visibility = state.Status.Blocked > 0 ? Visibility.Visible : Visibility.Collapsed;

        SyncNowButton.IsEnabled = connected && !busy && !state.Status.Paused && !needsAuthorization
                                  && !SyncPresentation.IsTerminal(state);
        PauseButton.IsEnabled = connected && !busy && !needsAuthorization;
        PauseButton.Content = _strings[state.Status.Paused ? "sync_resume" : "sync_pause"];
        TokenButton.IsEnabled = connected && !busy;
        DisconnectButton.IsEnabled = connected && !busy;
        BackgroundHint.Visibility = connected && !showForm ? Visibility.Visible : Visibility.Collapsed;

        var error = ErrorText(state);
        ErrorBar.Message = error ?? "";
        ErrorBar.IsOpen = error is not null;
    }

    private string? ErrorText(SyncState state)
    {
        if (_localErrorKey is not null) return _strings![_localErrorKey];
        var key = SyncPresentation.ErrorKey(state);
        if (key is not null) return _strings![key];
        return state.Error is null ? null : _strings![_strings!.ErrorKey(state.Error)];
    }

    private async void OnConnect(object sender, RoutedEventArgs e)
    {
        if (_strings is null || _sync is not { } sync) return;
        var origin = OriginBox.Text.Trim();
        var token = TokenBox.Password;
        if (origin.Length == 0 || token.Length == 0)
        {
            _localErrorKey = "failure_input";
            Render();
            return;
        }
        _localErrorKey = null;
        await sync.ConnectAsync(origin, token);
        if (!_closed && sync.State.Error is null && sync.State.HasCredential)
        {
            _editing = false;
            TokenBox.Password = "";
        }
        Render();
    }

    private void OnCancel(object sender, RoutedEventArgs e)
    {
        _editing = false;
        _localErrorKey = null;
        TokenBox.Password = "";
        Render();
    }

    private async void OnSynchronize(object sender, RoutedEventArgs e)
    {
        if (_sync is not { } sync) return;
        await sync.SynchronizeNowAsync();
        Render();
    }

    private async void OnTogglePause(object sender, RoutedEventArgs e)
    {
        if (_sync is not { } sync) return;
        var paused = sync.State.Status.Paused;
        await sync.SetPausedAsync(!paused);
        Render();
    }

    private void OnEditToken(object sender, RoutedEventArgs e)
    {
        if (_sync is null) return;
        _editing = true;
        _localErrorKey = null;
        TokenBox.Password = "";
        Render();
    }

    private async void OnDisconnect(object sender, RoutedEventArgs e)
    {
        if (_strings is null || _sync is not { } sync || XamlRoot is null) return;
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = _strings["sync_disconnect"],
            Content = _strings["sync_disconnect_hint"],
            PrimaryButtonText = _strings["sync_disconnect"],
            CloseButtonText = _strings["cancel"],
            DefaultButton = ContentDialogButton.Close
        };
        try
        {
            if (await dialog.ShowAsync() != ContentDialogResult.Primary) return;
        }
        catch (Exception error)
        {
            Trace.WriteLine(error);
            return;
        }
        if (_closed) return;
        await sync.DisconnectAsync();
        if (_closed) return;
        _editing = false;
        TokenBox.Password = "";
        Render();
    }
}
