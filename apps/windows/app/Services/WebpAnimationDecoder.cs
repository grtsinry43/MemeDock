using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.Win32.SafeHandles;

namespace MemeDock.Windows.Services;

// Used only by the playback worker. Native input and frame pointers never leave
// this class; the returned pixel array is reused on the next ReadFrame call.
internal sealed class WebpAnimationDecoder : IDisposable
{
    private const int DemuxAbiVersion = 0x0107;
    private const int PremultipliedBgra = 8; // MODE_bgrA in libwebp 1.6.0.
    private const int PreviewEdge = 1280;
    private readonly DecoderHandle _decoder;
    private readonly byte[] _canvas;
    private readonly byte[] _pixels;
    private readonly int _canvasWidth;
    private readonly int _canvasHeight;
    private int _timestamp;
    private uint _framesRead;

    internal int Width { get; }
    internal int Height { get; }
    internal uint FrameCount { get; }
    internal uint LoopCount { get; }

    private WebpAnimationDecoder(DecoderHandle decoder, AnimationInfo info)
    {
        _decoder = decoder;
        _canvasWidth = checked((int)info.Width);
        _canvasHeight = checked((int)info.Height);
        var scale = Math.Min(1d, (double)PreviewEdge / Math.Max(info.Width, info.Height));
        Width = Math.Max(1, (int)(info.Width * scale));
        Height = Math.Max(1, (int)(info.Height * scale));
        FrameCount = info.FrameCount;
        LoopCount = info.LoopCount;
        _canvas = new byte[checked(_canvasWidth * _canvasHeight * 4)];
        _pixels = Width == _canvasWidth && Height == _canvasHeight
            ? _canvas : new byte[checked(Width * Height * 4)];
    }

    internal static async Task<WebpAnimationDecoder> OpenAsync(string path, ulong maxFileBytes,
        ulong maxFramePixels, ulong maxDecodeBytes, ulong imageBudgetBytes, CancellationToken cancellation)
    {
        await using var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read,
            65536, FileOptions.Asynchronous | FileOptions.SequentialScan);
        var length = file.Length;
        if (length <= 0) throw new InvalidDataException("WebP file is empty.");
        if ((ulong)length > maxFileBytes || length > int.MaxValue) throw new WebpDecodeLimitException();
        var bytes = new byte[(int)length];
        await file.ReadExactlyAsync(bytes, cancellation).ConfigureAwait(false);
        cancellation.ThrowIfCancellationRequested();
        var pinned = GCHandle.Alloc(bytes, GCHandleType.Pinned);
        DecoderHandle? decoder = null;
        try
        {
            var data = new WebpData { Bytes = pinned.AddrOfPinnedObject(), Size = (nuint)bytes.Length };
            // Validate the header before WebPAnimDecoderNewInternal can allocate
            // its animation canvases. GetInfo alone would check too late.
            if (Native.WebPGetInfo(data.Bytes, data.Size, out var width, out var height) == 0
                || width <= 0 || height <= 0)
                throw new InvalidDataException("Invalid WebP header.");
            var pixels = (ulong)width * (ulong)height;
            var scale = Math.Min(1d, (double)PreviewEdge / Math.Max(width, height));
            var displayBytes = (ulong)Math.Max(1, (int)(width * scale))
                * (ulong)Math.Max(1, (int)(height * scale)) * 4;
            if (pixels > maxFramePixels || pixels > maxDecodeBytes / 16
                || (ulong)length + pixels * 16 + displayBytes * 3 > imageBudgetBytes)
                throw new WebpDecodeLimitException();

            var options = new DecoderOptions();
            if (Native.WebPAnimDecoderOptionsInitInternal(ref options, DemuxAbiVersion) == 0)
                throw new InvalidDataException("Unsupported WebP decoder ABI.");
            options.ColorMode = PremultipliedBgra;
            var handle = Native.WebPAnimDecoderNewInternal(ref data, ref options, DemuxAbiVersion);
            if (handle == IntPtr.Zero) throw new InvalidDataException("Cannot open WebP animation.");
            decoder = new DecoderHandle(handle, pinned);
            pinned = default; // The safe handle now owns the pinned input.
            if (Native.WebPAnimDecoderGetInfo(decoder, out var info) == 0
                || info.Width != (uint)width || info.Height != (uint)height || info.FrameCount < 2)
                throw new InvalidDataException("WebP animation metadata is invalid.");
            cancellation.ThrowIfCancellationRequested();
            var result = new WebpAnimationDecoder(decoder, info);
            decoder = null;
            return result;
        }
        finally
        {
            decoder?.Dispose();
            if (pinned.IsAllocated) pinned.Free();
        }
    }

    internal (byte[] Pixels, int DelayMilliseconds) ReadFrame(CancellationToken cancellation)
    {
        cancellation.ThrowIfCancellationRequested();
        if (_framesRead >= FrameCount || Native.WebPAnimDecoderHasMoreFrames(_decoder) == 0
            || Native.WebPAnimDecoderGetNext(_decoder, out var frame, out var timestamp) == 0
            || frame == IntPtr.Zero || timestamp < _timestamp)
            throw new InvalidDataException("Cannot decode WebP frame.");
        var delay = timestamp - _timestamp;
        _timestamp = timestamp;
        _framesRead++;
        Marshal.Copy(frame, _canvas, 0, _canvas.Length);
        if (!ReferenceEquals(_pixels, _canvas))
        {
            var source = MemoryMarshal.Cast<byte, uint>(_canvas.AsSpan());
            var destination = MemoryMarshal.Cast<byte, uint>(_pixels.AsSpan());
            for (var y = 0; y < Height; y++)
            {
                cancellation.ThrowIfCancellationRequested();
                var sy = Math.Clamp((int)((2L * y + 1) * _canvasHeight * 256 / (2 * Height) - 128),
                    0, (_canvasHeight - 1) * 256);
                var row0 = (sy >> 8) * _canvasWidth;
                var row1 = Math.Min((sy >> 8) + 1, _canvasHeight - 1) * _canvasWidth;
                for (var x = 0; x < Width; x++)
                {
                    var sx = Math.Clamp((int)((2L * x + 1) * _canvasWidth * 256 / (2 * Width) - 128),
                        0, (_canvasWidth - 1) * 256);
                    var x0 = sx >> 8;
                    var x1 = Math.Min(x0 + 1, _canvasWidth - 1);
                    // Interpolate premultiplied channels together so scaled
                    // transparent edges do not acquire dark/color fringes.
                    var top = Interpolate(source[row0 + x0], source[row0 + x1], (uint)(sx & 255));
                    var bottom = Interpolate(source[row1 + x0], source[row1 + x1], (uint)(sx & 255));
                    destination[y * Width + x] = Interpolate(top, bottom, (uint)(sy & 255));
                }
            }
        }
        // Zero-duration frames cannot monopolize the decoder/UI threads.
        return (_pixels, delay == 0 ? 100 : Math.Max(10, delay));
    }

    private static uint Interpolate(uint first, uint second, uint weight)
    {
        const uint channels = 0x00ff00ff;
        var inverse = 256 - weight;
        var blueRed = (((first & channels) * inverse + (second & channels) * weight) >> 8) & channels;
        var greenAlpha = ((((first >> 8) & channels) * inverse + ((second >> 8) & channels) * weight) >> 8) & channels;
        return blueRed | greenAlpha << 8;
    }

    internal void Reset()
    {
        Native.WebPAnimDecoderReset(_decoder);
        _timestamp = 0;
        _framesRead = 0;
    }

    public void Dispose() => _decoder.Dispose();

    [StructLayout(LayoutKind.Sequential)]
    private struct WebpData { internal IntPtr Bytes; internal nuint Size; }

    // Explicit scalar padding matches demux.h without managed array marshalling.
    [StructLayout(LayoutKind.Sequential)]
    private struct DecoderOptions
    {
        internal int ColorMode, UseThreads;
        private uint _pad0, _pad1, _pad2, _pad3, _pad4, _pad5, _pad6;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct AnimationInfo
    {
        internal uint Width, Height, LoopCount, BackgroundColor, FrameCount;
        private uint _pad0, _pad1, _pad2, _pad3;
    }

    private sealed class DecoderHandle : SafeHandleZeroOrMinusOneIsInvalid
    {
        private GCHandle _input;
        internal DecoderHandle(IntPtr handle, GCHandle input) : base(ownsHandle: true)
        {
            SetHandle(handle);
            _input = input;
        }

        protected override bool ReleaseHandle()
        {
            Native.WebPAnimDecoderDelete(handle);
            if (_input.IsAllocated) _input.Free();
            return true;
        }
    }

    private static class Native
    {
        private const DllImportSearchPath SearchPath = DllImportSearchPath.AssemblyDirectory | DllImportSearchPath.SafeDirectories;

        [DefaultDllImportSearchPaths(SearchPath)]
        [DllImport("libwebp.dll", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        internal static extern int WebPGetInfo(IntPtr data, nuint size, out int width, out int height);

        [DefaultDllImportSearchPaths(SearchPath)]
        [DllImport("libwebpdemux.dll", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        internal static extern int WebPAnimDecoderOptionsInitInternal(ref DecoderOptions options, int version);

        [DefaultDllImportSearchPaths(SearchPath)]
        [DllImport("libwebpdemux.dll", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        internal static extern IntPtr WebPAnimDecoderNewInternal(ref WebpData data, ref DecoderOptions options, int version);

        [DefaultDllImportSearchPaths(SearchPath)]
        [DllImport("libwebpdemux.dll", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        internal static extern int WebPAnimDecoderGetInfo(DecoderHandle decoder, out AnimationInfo info);

        [DefaultDllImportSearchPaths(SearchPath)]
        [DllImport("libwebpdemux.dll", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        internal static extern int WebPAnimDecoderGetNext(DecoderHandle decoder, out IntPtr pixels, out int timestamp);

        [DefaultDllImportSearchPaths(SearchPath)]
        [DllImport("libwebpdemux.dll", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        internal static extern int WebPAnimDecoderHasMoreFrames(DecoderHandle decoder);

        [DefaultDllImportSearchPaths(SearchPath)]
        [DllImport("libwebpdemux.dll", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        internal static extern void WebPAnimDecoderReset(DecoderHandle decoder);

        [DefaultDllImportSearchPaths(SearchPath)]
        [DllImport("libwebpdemux.dll", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        internal static extern void WebPAnimDecoderDelete(IntPtr decoder);
    }
}

internal sealed class WebpDecodeLimitException : Exception
{
    internal WebpDecodeLimitException() : base("WebP preview exceeds image resource limits.") { }
}
