using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Services;

namespace MemeDock.Windows.ViewModels;

internal enum TelegramPhase { Idle, Loading, Ready, Importing }

internal sealed class TelegramImportViewModel(ITelegramWorkspace workspace, ITelegramTokenStore tokens,
    ICatalogStrings strings, Func<Task> imported) : ObservableObject, IAsyncDisposable
{
    public string Token { get; set; } = "";
    public string Query { get; set; } = "";
    public bool Remember { get; set; }
    public bool TokenSaved { get; private set; }
    public TelegramPhase Phase { get; private set; }
    public bool Busy => Phase is TelegramPhase.Loading or TelegramPhase.Importing;
    public TelegramPackMetadata? Metadata => _pack?.Metadata;
    public HashSet<string> Selected { get; } = [];
    public Dictionary<string, TelegramImportState> States { get; } = [];
    public Dictionary<string, TelegramImportOutcome> Outcomes { get; } = [];
    public Dictionary<string, ErrorCode?> Errors { get; } = [];
    public TelegramImportReport? Progress { get; private set; }
    public string? Failure { get; private set; }
    private string? _failureKey;
    private string? _summaryKey;
    private object[] _summaryArguments = [];
    public string? Summary => _summaryKey is null ? null : strings.Format(_summaryKey, _summaryArguments);
    public int Completed => Progress?.Items.Count(item => item.Outcome != TelegramImportOutcome.Pending) ?? 0;
    private ITelegramPack? _pack;
    private CancellationTokenSource? _operation;
    private readonly CancellationTokenSource _lifetime = new();
    private readonly SemaphoreSlim _previews = new(4);
    private readonly HashSet<Task> _previewWork = [];
    private Task _work = Task.CompletedTask;
    private bool _started, _stopping;
    public void Relocalize() { if (_failureKey is not null) Failure = strings[_failureKey]; Notify(); }
    private void Notify() => OnPropertyChanged(string.Empty);
    private void Fail(string key) { _failureKey = key; Failure = strings[key]; }
    public async Task StartAsync()
    {
        if (_started || _stopping) return;
        _started = true;
        try
        {
            var token = await tokens.LoadAsync(_lifetime.Token);
            if (token is not null && IsTokenShaped(token)) { Token = token; Remember = TokenSaved = true; }
        }
        catch (OperationCanceledException) { }
        catch (Exception) { Fail("tg_token_failed"); }
        Notify();
    }
    public async Task ForgetAsync()
    {
        if (Busy || _stopping) return;
        try { await tokens.ClearAsync(_lifetime.Token); Token = ""; Remember = TokenSaved = false; }
        catch (Exception) { Fail("tg_token_failed"); }
        Notify();
    }
    public Task LoadAsync()
    {
        if (Busy || _stopping) return Task.CompletedTask;
        return _work = LoadCoreAsync();
    }
    private async Task LoadCoreAsync()
    {
        var token = Token.Trim();
        if (!IsTokenShaped(token)) { Fail("tg_token_invalid"); Notify(); return; }
        if (string.IsNullOrWhiteSpace(Query)) return;
        ResetOperation();
        Phase = TelegramPhase.Loading;
        Failure = _failureKey = _summaryKey = null;
        Notify();
        try
        {
            var pack = await workspace.LoadAsync(token, Query.Trim(), _operation!.Token);
            if (_operation.IsCancellationRequested || _stopping) { pack.Dispose(); return; }
            await DrainPreviewsAsync();
            _pack?.Dispose();
            _pack = pack;
            States.Clear(); Outcomes.Clear(); Errors.Clear(); Selected.Clear(); Progress = null;
            foreach (var item in pack.Metadata.Stickers) States[item.Id] = item.State;
            if (Remember)
            {
                try { await tokens.SaveAsync(token, _lifetime.Token); TokenSaved = true; }
                catch (OperationCanceledException) { }
                catch (Exception) { Fail("tg_token_failed"); }
            }
        }
        catch (OperationCanceledException) { }
        catch (Exception error) { Fail(error is BridgeException.Failure { code: ErrorCode.Unauthorized } ? "tg_token_invalid"
            : error is BridgeException.Failure { code: ErrorCode.NotFound } ? "tg_pack_missing" : strings.ErrorKey(error)); }
        finally { Phase = _pack is null ? TelegramPhase.Idle : TelegramPhase.Ready; Notify(); }
    }
    public bool CanSelect(string id) => States.TryGetValue(id, out var state) && state != TelegramImportState.RestoreRequired;
    public void Toggle(string id)
    {
        if (Busy || !CanSelect(id)) return;
        if (!Selected.Remove(id)) Selected.Add(id);
        Notify();
    }
    public void SelectNew()
    {
        if (Busy) return;
        Selected.Clear();
        foreach (var (id, state) in States) if (state is TelegramImportState.Available or TelegramImportState.OriginalMissing) Selected.Add(id);
        Notify();
    }
    public void ClearSelection() { if (!Busy) { Selected.Clear(); Notify(); } }
    public Task ImportAsync()
    {
        if (Busy || _stopping || _pack is null || Selected.Count == 0) return Task.CompletedTask;
        return _work = ImportCoreAsync();
    }
    private async Task ImportCoreAsync()
    {
        ResetOperation();
        Phase = TelegramPhase.Importing;
        Failure = _failureKey = _summaryKey = null;
        Outcomes.Clear(); Errors.Clear(); Progress = null;
        Notify();
        try
        {
            var report = await _pack!.ImportAsync(Selected.Order(StringComparer.Ordinal).ToArray(), progress => { Progress = progress; Notify(); }, _operation!.Token);
            Progress = report;
            foreach (var item in report.Items)
            {
                Outcomes[item.Id] = item.Outcome; Errors[item.Id] = item.Error;
                if (item.Outcome is TelegramImportOutcome.Created or TelegramImportOutcome.Reused)
                { States[item.Id] = TelegramImportState.Imported; Selected.Remove(item.Id); }
                else if (item.Outcome == TelegramImportOutcome.RestoreRequired)
                { States[item.Id] = TelegramImportState.RestoreRequired; Selected.Remove(item.Id); }
                // Failed and unprocessed items remain selected for a deliberate retry.
            }
            var added = report.Items.Count(item => item.Outcome == TelegramImportOutcome.Created);
            var reused = report.Items.Count(item => item.Outcome == TelegramImportOutcome.Reused);
            var failed = report.Items.Count(item => item.Outcome == TelegramImportOutcome.Failed);
            var restore = report.Items.Count(item => item.Outcome == TelegramImportOutcome.RestoreRequired);
            _summaryKey = report.Stopped ? "tg_import_stopped" : failed > 0 ? "tg_import_partial" : restore > 0 ? "tg_import_restore" : reused > 0 ? "tg_import_done" : "tg_import_added";
            _summaryArguments = report.Stopped ? [added + reused] : failed > 0 ? [added + reused, failed] : restore > 0 ? [restore] : reused > 0 ? [added, reused] : [added];
            if (report.Items.FirstOrDefault(item => item.Error is not null)?.Error is { } error)
                Fail(strings.ErrorKey(new BridgeException.Failure(error, "")));
            await imported();
        }
        catch (OperationCanceledException) { }
        catch (Exception error) { Fail(strings.ErrorKey(error)); }
        finally { Phase = TelegramPhase.Ready; Notify(); }
    }
    public Task<string?> PreviewAsync(string id)
    {
        if (_stopping || _pack is null) return Task.FromResult<string?>(null);
        var task = PreviewCoreAsync(_pack, id, _operation?.Token ?? _lifetime.Token);
        _previewWork.Add(task);
        _ = RemovePreviewAsync(task);
        return task;
    }
    private async Task RemovePreviewAsync(Task task) { try { await task; } finally { _previewWork.Remove(task); } }
    private async Task<string?> PreviewCoreAsync(ITelegramPack pack, string id, CancellationToken cancellation)
    {
        var acquired = false;
        try { await _previews.WaitAsync(cancellation); acquired = true; return await pack.PreviewAsync(id, cancellation); }
        catch (Exception) { return null; } // A missing preview never makes the sticker unimportable.
        finally { if (acquired) _previews.Release(); }
    }
    private Task DrainPreviewsAsync() => Task.WhenAll(_previewWork.ToArray());
    private void ResetOperation() { _operation?.Cancel(); _operation?.Dispose(); _operation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token); }
    public void Cancel() => _operation?.Cancel();
    public static bool IsTokenShaped(string token)
    {
        var colon = token.IndexOf(':');
        return colon > 0 && token.Length <= 256 && token[..colon].All(char.IsAsciiDigit)
            && token[(colon + 1)..] is { Length: >= 20 and <= 128 } secret
            && secret.All(c => char.IsAsciiLetterOrDigit(c) || c is '_' or '-');
    }
    public async ValueTask DisposeAsync()
    {
        _stopping = true;
        _lifetime.Cancel(); Cancel();
        await _work; await DrainPreviewsAsync();
        _pack?.Dispose(); _operation?.Dispose(); _lifetime.Dispose(); _previews.Dispose();
    }
}
