using MemeDock.Bridge;
using MemeDock.Windows.Models;
using Windows.ApplicationModel.DataTransfer;
using Windows.Graphics.Imaging;
using Windows.Storage;
using Windows.Storage.Streams;

namespace MemeDock.Windows.Services;

internal static class StickerImportService
{
    internal const string InternalDragFormat = "application/x-memedock-internal-drag";

    // Call from the UI thread: a data provider may render its data on demand.
    // File/image streams themselves are opened sequentially by the session.
    internal static async Task<IReadOnlyList<StickerImportSource>> ReadAsync(
        DataPackageView data, CancellationToken cancellation)
    {
        cancellation.ThrowIfCancellationRequested();
        if (data.Contains(StandardDataFormats.StorageItems))
        {
            var items = await data.GetStorageItemsAsync().AsTask(cancellation);
            if (items.Count > 200)
                throw new BridgeException.Failure(ErrorCode.ResourceLimit, "Too many import entries.");
            return items.OfType<StorageFile>().Select(file => new StickerImportSource(file.Name, async token =>
            {
                var stream = await file.OpenReadAsync().AsTask(token).ConfigureAwait(false);
                return stream.AsStreamForRead();
            })).ToArray();
        }
        if (data.Contains(StandardDataFormats.Bitmap))
        {
            var reference = await data.GetBitmapAsync().AsTask(cancellation);
            return [new StickerImportSource("Clipboard.png", token => OpenBitmapAsync(reference, token))];
        }
        return [];
    }

    private static async Task<Stream> OpenBitmapAsync(RandomAccessStreamReference reference,
        CancellationToken cancellation)
    {
        var limits = MemeDockNative.DefaultResourceConfiguration();
        using var input = await reference.OpenReadAsync().AsTask(cancellation).ConfigureAwait(false);
        if (input.Size > limits.MaxFileBytes) throw LimitExceeded();
        var decoder = await BitmapDecoder.CreateAsync(input).AsTask(cancellation).ConfigureAwait(false);
        var pixels = (ulong)decoder.PixelWidth * decoder.PixelHeight;
        // Match the core's conservative four-RGBA-canvas estimate before
        // allocating decoded pixels or encoder/output buffers. The input is
        // streamed, rather than copied into another managed image buffer.
        if (pixels == 0 || pixels > limits.MaxFramePixels
            || pixels > limits.MaxDecodeBytes / 16)
            throw LimitExceeded();

        using var bitmap = await decoder.GetSoftwareBitmapAsync(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Straight)
            .AsTask(cancellation).ConfigureAwait(false);
        var output = new InMemoryRandomAccessStream();
        try
        {
            var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, output)
                .AsTask(cancellation).ConfigureAwait(false);
            encoder.SetSoftwareBitmap(bitmap);
            encoder.IsThumbnailGenerated = false;
            await encoder.FlushAsync().AsTask(cancellation).ConfigureAwait(false);
            if (output.Size > limits.MaxFileBytes) throw LimitExceeded();
            output.Seek(0);
            // The .NET stream adapter owns the WinRT stream from here on.
            return output.AsStreamForRead();
        }
        catch
        {
            output.Dispose();
            throw;
        }
    }

    private static BridgeException.Failure LimitExceeded() =>
        new(ErrorCode.ResourceLimit, "Clipboard image exceeds import resource limits.");
}
