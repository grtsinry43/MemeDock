using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Services;
using MemeDock.Windows.ViewModels;

internal static class TelegramImportTests
{
    private static void Check(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
    private const string Token = "1234:abcdefghijklmnopqrstuv";
    public static async Task RunAsync()
    {
        Check(TelegramImportViewModel.IsTokenShaped(Token), "Valid bot token was rejected.");
        foreach (var invalid in new[] { "", "no-colon", "１２:abcdefghijklmnopqrstuv", "12:short", Token + "\n" })
            Check(!TelegramImportViewModel.IsTokenShaped(invalid), "Malformed token was accepted.");
        await SelectionAndResultsAsync(); await CancellationAsync(); await CredentialFailureAsync();
        Console.WriteLine("PASS: Telegram credentials, selection, per-item outcomes, retry and cancellation");
    }
    private static async Task SelectionAndResultsAsync()
    {
        var workspace = new Workspace(); var credentials = new Tokens(); var refreshed = 0;
        await using var model = new TelegramImportViewModel(workspace, credentials, new Strings(), () => { refreshed++; return Task.CompletedTask; });
        await model.StartAsync();
        Check(model.TokenSaved && model.Token == Token && model.Remember, "Saved token was not restored.");
        model.Query = "pack"; await model.LoadAsync();
        Check(model.Phase == TelegramPhase.Ready && credentials.Saves == 1 && workspace.Loads == 1, "Pack lookup did not save a confirmed token.");
        model.SelectNew();
        Check(model.Selected.SetEquals(["new", "missing"]), "Select-new included reused or trashed stickers or excluded missing originals.");
        model.Toggle("trash"); model.Toggle("unknown");
        Check(model.Selected.SetEquals(["new", "missing"]), "An unselectable item entered the import selection.");
        workspace.Pack.Report = new([
            new("new", "sticker", TelegramImportOutcome.Created, null),
            new("missing", null, TelegramImportOutcome.Failed, ErrorCode.Network)], false);
        await model.ImportAsync();
        Check(model.States["new"] == TelegramImportState.Imported && model.Selected.SetEquals(["missing"]), "Import lost a failed retry or kept a successful item selected.");
        Check(model.Outcomes["missing"] == TelegramImportOutcome.Failed && model.Errors["missing"] == ErrorCode.Network
            && model.Summary!.StartsWith("tg_import_partial:", StringComparison.Ordinal) && refreshed == 1, "Per-item results or refresh were lost.");
        workspace.Pack.Report = new([new("missing", "restored-original", TelegramImportOutcome.Reused, null)], false);
        await model.ImportAsync();
        Check(workspace.Pack.LastIds!.SequenceEqual(["missing"]) && model.Selected.Count == 0 && refreshed == 2, "Retry imported unselected or previously successful items.");
        model.Toggle("old");
        workspace.Pack.Report = new([new("old", null, TelegramImportOutcome.RestoreRequired, null)], false);
        await model.ImportAsync();
        Check(!model.CanSelect("old") && model.Selected.Count == 0 && model.Summary!.StartsWith("tg_import_restore:", StringComparison.Ordinal), "Import silently restored a trashed sticker.");
        await model.ForgetAsync();
        Check(!model.TokenSaved && model.Token.Length == 0 && credentials.Clears == 1, "Forgetting credentials left the token in memory.");
    }
    private static async Task CancellationAsync()
    {
        var workspace = new Workspace { DelayLoad = true };
        await using var model = new TelegramImportViewModel(workspace, new Tokens(), new Strings(), () => Task.CompletedTask) { Token = Token, Query = "pack" };
        var load = model.LoadAsync();
        Check(model.Busy && model.Phase == TelegramPhase.Loading, "Lookup was not marked busy before its first await.");
        await model.LoadAsync();
        Check(workspace.Loads == 1, "Concurrent lookups were accepted.");
        model.Cancel(); await load;
        Check(!model.Busy && model.Metadata is null, "Cancelled lookup left a loaded pack or busy state.");
        workspace.DelayLoad = false; await model.LoadAsync(); model.SelectNew();
        workspace.Pack.DelayImport = true;
        var import = model.ImportAsync();
        model.Cancel(); await import;
        Check(!model.Busy && model.Selected.SetEquals(["new", "missing"])
            && model.Summary!.StartsWith("tg_import_stopped:", StringComparison.Ordinal), "Cancelled import lost unfinished selection or its partial result.");
    }
    private static async Task CredentialFailureAsync()
    {
        var workspace = new Workspace(); var credentials = new Tokens { FailSave = true };
        await using var model = new TelegramImportViewModel(workspace, credentials, new Strings(), () => Task.CompletedTask)
        { Token = Token, Query = "pack", Remember = true };
        await model.LoadAsync();
        Check(model.Metadata is not null && model.Failure == "tg_token_failed" && !model.TokenSaved,
            "Credential write failure discarded the usable pack or claimed the token was saved.");
        model.Token = "invalid"; await model.LoadAsync();
        Check(workspace.Loads == 1 && model.Failure == "tg_token_invalid", "Malformed token reached the network.");
    }
    private sealed class Strings : ICatalogStrings
    {
        public string this[string key] => key;
        public string Format(string key, params object[] args) => key + ":" + string.Join(",", args);
        public string ErrorKey(Exception error) => "failure_network";
    }
    private sealed class Tokens : ITelegramTokenStore
    {
        public int Saves, Clears; public bool FailSave;
        public Task<string?> LoadAsync(CancellationToken cancellation) => Task.FromResult<string?>(Token);
        public Task SaveAsync(string token, CancellationToken cancellation)
        { if (FailSave) throw new IOException("Credential write failed."); Saves++; return Task.CompletedTask; }
        public Task ClearAsync(CancellationToken cancellation) { Clears++; return Task.CompletedTask; }
    }
    private sealed class Workspace : ITelegramWorkspace
    {
        public int Loads; public bool DelayLoad;
        public Pack Pack { get; } = new();
        public async Task<ITelegramPack> LoadAsync(string token, string query, CancellationToken cancellation)
        { Loads++; if (DelayLoad) await Task.Delay(Timeout.Infinite, cancellation); return Pack; }
    }
    private sealed class Pack : ITelegramPack
    {
        public TelegramPackMetadata Metadata { get; } = new("pack", "Pack", [
            new("new", "😃", 100, 100, TelegramFormat.Static, TelegramImportState.Available, true),
            new("missing", null, 100, 100, TelegramFormat.Tgs, TelegramImportState.OriginalMissing, true),
            new("old", null, 100, 100, TelegramFormat.Webm, TelegramImportState.Imported, true),
            new("trash", null, 100, 100, TelegramFormat.Static, TelegramImportState.RestoreRequired, false)]);
        public TelegramImportReport Report = new([], false);
        public string[]? LastIds; public bool DelayImport;
        public Task<string> PreviewAsync(string id, CancellationToken cancellation) => Task.FromResult("preview.png");
        public async Task<TelegramImportReport> ImportAsync(string[] ids, Action<TelegramImportReport> progress, CancellationToken cancellation)
        {
            LastIds = ids; progress(new(ids.Select(id => new TelegramImportItem(id, null, TelegramImportOutcome.Pending, null)).ToArray(), false));
            if (DelayImport)
            {
                try { await Task.Delay(Timeout.Infinite, cancellation); } catch (OperationCanceledException) { }
                return new(ids.Select(id => new TelegramImportItem(id, null, TelegramImportOutcome.Pending, null)).ToArray(), true);
            }
            return Report;
        }
        public void Dispose() { }
    }
}
