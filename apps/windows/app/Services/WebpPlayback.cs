using System.Diagnostics;
using System.Runtime.InteropServices.WindowsRuntime;
using MemeDock.Bridge;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Graphics.Imaging;

namespace MemeDock.Windows.Services;

// UI-thread owner, with decoding confined to one cancellable background worker.
internal sealed class WebpPlayback(string path, DispatcherQueue dispatcher) : IDisposable
{
    // A cancelled predecessor must finish its current native call before another
    // preview allocates animation canvases, including during rapid switching.
    private static readonly SemaphoreSlim DecodeSlot = new(1, 1);
    private static readonly SemaphoreSlim PresentationSlot = new(1, 1);
    private readonly object _state = new();
    private CancellationTokenSource? _run;
    private bool _disposed;
    internal SoftwareBitmapSource Source { get; } = new();
    internal bool HasFailed { get; private set; }
    internal event Action? FramePresented;
    internal event Action<Exception>? Failed;
    internal event Action? Completed;

    internal void SetRunning(bool running)
    {
        CancellationTokenSource run;
        lock (_state)
        {
            if (_disposed || HasFailed) return;
            if (!running)
            {
                _run?.Cancel();
                _run = null;
                return;
            }
            if (_run is not null) return;
            run = new CancellationTokenSource();
            _run = run;
        }
        _ = Task.Run(async () =>
        {
            try { await PlayAsync(run.Token).ConfigureAwait(false); }
            catch (OperationCanceledException) when (run.IsCancellationRequested) { }
            catch (Exception error)
            {
                Trace.WriteLine($"WebP preview: {error}");
                try
                {
                    await DispatchAsync(() =>
                    {
                        lock (_state) _run = null;
                        HasFailed = true;
                        Failed?.Invoke(error);
                        return Task.CompletedTask;
                    }, run.Token).ConfigureAwait(false);
                }
                catch (OperationCanceledException) { }
                catch (Exception reportingError) { Trace.WriteLine($"Report WebP preview failure: {reportingError}"); }
            }
            finally
            {
                lock (_state)
                {
                    if (ReferenceEquals(_run, run)) _run = null;
                    run.Dispose();
                }
            }
        });
    }

    private async Task PlayAsync(CancellationToken cancellation)
    {
        await DecodeSlot.WaitAsync(cancellation).ConfigureAwait(false);
        try
        {
            var limits = MemeDockNative.DefaultResourceConfiguration();
            using var decoder = await WebpAnimationDecoder.OpenAsync(path, limits.MaxFileBytes,
                limits.MaxFramePixels, limits.MaxDecodeBytes, limits.ImageBudgetBytes, cancellation).ConfigureAwait(false);
            uint cycles = 0;
            do
            {
                var clock = Stopwatch.StartNew();
                var deadline = TimeSpan.Zero;
                for (uint index = 0; index < decoder.FrameCount; index++)
                {
                    var frame = decoder.ReadFrame(cancellation);
                    var width = decoder.Width;
                    var height = decoder.Height;
                    // Await presentation before reusing the frame buffer. Only
                    // one frame can be pending in the UI queue at a time.
                    await DispatchAsync(async () =>
                    {
                        await PresentationSlot.WaitAsync(cancellation);
                        try
                        {
                            cancellation.ThrowIfCancellationRequested();
                            using var bitmap = SoftwareBitmap.CreateCopyFromBuffer(frame.Pixels.AsBuffer(),
                                BitmapPixelFormat.Bgra8, width, height, BitmapAlphaMode.Premultiplied);
                            await Source.SetBitmapAsync(bitmap);
                            if (!cancellation.IsCancellationRequested) FramePresented?.Invoke();
                        }
                        finally { PresentationSlot.Release(); }
                    }, cancellation).ConfigureAwait(false);
                    deadline += TimeSpan.FromMilliseconds(frame.DelayMilliseconds);
                    var wait = deadline - clock.Elapsed;
                    if (wait > TimeSpan.Zero) await Task.Delay(wait, cancellation).ConfigureAwait(false);
                    else deadline = clock.Elapsed; // Avoid a burst after a slow frame.
                }
                cycles++;
                decoder.Reset();
            } while (decoder.LoopCount == 0 || cycles < decoder.LoopCount);
        }
        finally { DecodeSlot.Release(); }

        await DispatchAsync(() =>
        {
            lock (_state) _run = null;
            Completed?.Invoke();
            return Task.CompletedTask;
        }, cancellation).ConfigureAwait(false);
    }

    private async Task DispatchAsync(Func<Task> action, CancellationToken cancellation)
    {
        var completion = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        using var registration = cancellation.Register(() => completion.TrySetCanceled(cancellation));
        if (!dispatcher.TryEnqueue(async () =>
        {
            if (cancellation.IsCancellationRequested) return;
            try
            {
                await action();
                completion.TrySetResult();
            }
            catch (Exception error) { completion.TrySetException(error); }
        })) throw new OperationCanceledException("The preview dispatcher is shutting down.", cancellation);
        await completion.Task.ConfigureAwait(false);
    }

    public void Dispose()
    {
        lock (_state)
        {
            if (_disposed) return;
            _disposed = true;
            _run?.Cancel();
            _run = null;
        }
        FramePresented = null;
        Failed = null;
        Completed = null;
    }
}
