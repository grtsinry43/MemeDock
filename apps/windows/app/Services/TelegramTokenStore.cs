namespace MemeDock.Windows.Services;

internal interface ITelegramTokenStore
{
    Task<string?> LoadAsync(CancellationToken cancellation);
    Task SaveAsync(string token, CancellationToken cancellation);
    Task ClearAsync(CancellationToken cancellation);
}
internal sealed class TelegramTokenStore : ITelegramTokenStore
{
    // A distinct scope prevents collisions with per-library sync credentials.
    private readonly WindowsCredentialTokenStore _store = new();
    private const string Scope = "com.grtsinry43.memedock.telegram-bot";
    public Task<string?> LoadAsync(CancellationToken cancellation) => _store.LoadAsync(Scope, "telegram", cancellation);
    public Task SaveAsync(string token, CancellationToken cancellation) => _store.SaveAsync(Scope, "telegram", token, cancellation);
    public Task ClearAsync(CancellationToken cancellation) => _store.ClearAsync(Scope, "telegram", cancellation);
}
