using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using Windows.ApplicationModel.DataTransfer;

namespace MemeDock.Windows.ViewModels;

public sealed class StickerOutputViewModel : ObservableObject, IAsyncDisposable
{
    private readonly LibrarySession _session;
    private readonly LocalizationService _strings;
    private readonly StickerOutputService _service;
    private readonly ClipboardGuard _clipboard;
    private readonly CancellationTokenSource _lifetime = new();
    private Task _work = Task.CompletedTask;
    private PreparedSticker? _drag;
    private StickerOutputTarget? _dragTarget;
    private bool _stopping;
    public bool IsBusy { get; private set; }
    public bool IsDragging => _drag is not null;
    public bool CanStart => !IsBusy && !_stopping;
    public string? MessageKey { get; private set; }
    public string? ErrorKey { get; private set; }
    public string StatusText => string.Join("\n", new[]
    {
        MessageKey is null ? "" : _strings[MessageKey], ErrorKey is null ? "" : _strings[ErrorKey]
    }.Where(text => text.Length > 0));

    internal StickerOutputViewModel(LibrarySession session, LocalizationService strings)
    {
        (_session, _strings) = (session, strings);
        _service = new StickerOutputService(session);
        _clipboard = new ClipboardGuard(session);
    }

    public void SetForeground(bool foreground) => _clipboard.SetForeground(foreground);
    public void Relocalize() => Notify();
    private void Notify() => OnPropertyChanged(string.Empty);
    private void Begin()
    {
        IsBusy = true;
        MessageKey = "share_preparing";
        ErrorKey = null;
        Notify();
    }

    public Task<bool> CopyAsync(StickerOutputTarget target, StickerOutputKind kind = StickerOutputKind.Image)
    {
        if (!CanStart) return Task.FromResult(false);
        var work = CopyCoreAsync(target, kind);
        _work = work;
        return work;
    }

    private async Task<bool> CopyCoreAsync(StickerOutputTarget target, StickerOutputKind kind)
    {
        Begin();
        PreparedSticker? prepared = null;
        try
        {
            var package = new DataPackage();
            prepared = await _service.PrepareAsync(target, kind, package, drag: false, _lifetime.Token);
            await _clipboard.InstallAsync(prepared, package, _lifetime.Token);
            prepared = null; // ClipboardGuard owns the published lease and any persistence retries.
            MessageKey = "image_copied";
            try { await _session.RecordUseAsync(target.Id, kind == StickerOutputKind.OriginalFile ? UsageAction.CopyFile : UsageAction.CopyImage); }
            catch (Exception error) { Trace.WriteLine($"Copy already published; record usage failed: {error}"); }
            return true;
        }
        catch (OperationCanceledException) { MessageKey = null; return false; }
        catch (Exception error) { MessageKey = null; Report(error); return false; }
        finally { prepared?.Dispose(); IsBusy = false; Notify(); }
    }

    public Task<bool> PrepareDragAsync(StickerOutputTarget target, DataPackage package)
    {
        if (!CanStart) return Task.FromResult(false);
        var work = PrepareDragCoreAsync(target, package);
        _work = work;
        return work;
    }

    private async Task<bool> PrepareDragCoreAsync(StickerOutputTarget target, DataPackage package)
    {
        Begin();
        PreparedSticker? prepared = null;
        try
        {
            prepared = await _service.PrepareAsync(target, StickerOutputKind.OriginalFile, package, drag: true, _lifetime.Token);
            _lifetime.Token.ThrowIfCancellationRequested();
            _drag = prepared;
            prepared = null;
            _dragTarget = target;
            MessageKey = null;
            return true;
        }
        catch (OperationCanceledException) { MessageKey = null; return false; }
        catch (Exception error) { MessageKey = null; Report(error); return false; }
        finally { prepared?.Dispose(); IsBusy = _drag is not null; Notify(); }
    }

    public Task CompleteDragAsync(bool copied)
    {
        if (_stopping || _drag is null) return Task.CompletedTask;
        return _work = CompleteDragCoreAsync(copied);
    }

    private async Task CompleteDragCoreAsync(bool copied)
    {
        var prepared = _drag;
        var target = _dragTarget;
        _drag = null;
        _dragTarget = null;
        try
        {
            if (copied && target is not null) await _session.RecordUseAsync(target.Id, UsageAction.CopyFile);
        }
        catch (Exception error) { Trace.WriteLine($"Drag already completed; record usage failed: {error}"); }
        finally
        {
            // PrepareHandoff has already persisted retention for a target reading asynchronously.
            prepared?.Dispose();
            IsBusy = false;
            Notify();
        }
    }

    private void Report(Exception error) { Trace.WriteLine(error); ErrorKey = _strings.ErrorKey(error); }
    public async ValueTask DisposeAsync()
    {
        _stopping = true;
        _lifetime.Cancel();
        await _work;
        _drag?.Dispose();
        _drag = null;
        await _clipboard.DisposeAsync();
        _lifetime.Dispose();
    }
}
