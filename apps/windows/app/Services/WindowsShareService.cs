using System.Diagnostics;
using System.Runtime.InteropServices;
using MemeDock.Bridge;
using Microsoft.UI.Xaml;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using WinRT.Interop;

namespace MemeDock.Windows.Services;

/// <summary>
/// Unpackaged WinUI 3 wrapper around <c>IDataTransferManagerInterop</c> to open the
/// native Windows Share sheet for a prepared sticker artifact.
/// </summary>
internal sealed class WindowsShareService : IDisposable
{
    // Verified from Windows SDK 10.0.26100.0 windows.applicationmodel.datatransfer.h
    private static readonly Guid IidDataTransferManager = new("a5caee9b-8708-49d1-8d36-67d25a8da00c");

    [ComImport]
    [Guid("3A3DCD6C-3EAB-43DC-BCDE-45671CE800C8")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IDataTransferManagerInterop
    {
        IntPtr GetForWindow([In] IntPtr appWindow, [In] ref Guid riid);
        void ShowShareUIForWindow([In] IntPtr appWindow);
    }

    private readonly IntPtr _hwnd;
    private DataTransferManager? _manager;
    private string? _pendingPath;
    private string? _pendingTitle;
    private bool _disposed;

    public WindowsShareService(Window window)
    {
        _hwnd = WindowNative.GetWindowHandle(window);
    }

    public void ShareFile(ExportArtifact artifact, string title)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        EnsureInitialized();
        _pendingPath = StickerOutputService.NormalizePath(artifact.Path);
        _pendingTitle = string.IsNullOrWhiteSpace(title) ? artifact.FileName : title;
        var interop = DataTransferManager.As<IDataTransferManagerInterop>();
        interop.ShowShareUIForWindow(_hwnd);
    }

    private void EnsureInitialized()
    {
        if (_manager is not null) return;
        var interop = DataTransferManager.As<IDataTransferManagerInterop>();
        var iid = IidDataTransferManager;
        var abi = interop.GetForWindow(_hwnd, ref iid);
        try
        {
            _manager = WinRT.MarshalInterface<DataTransferManager>.FromAbi(abi);
        }
        finally
        {
            if (abi != IntPtr.Zero) Marshal.Release(abi);
        }
        _manager.DataRequested += OnDataRequested;
    }

    private async void OnDataRequested(DataTransferManager sender, DataRequestedEventArgs args)
    {
        var path = _pendingPath;
        var title = _pendingTitle;
        if (string.IsNullOrWhiteSpace(path))
        {
            args.Request.FailWithDisplayText("No sticker file is ready to share.");
            return;
        }
        var deferral = args.Request.GetDeferral();
        try
        {
            var file = await StorageFile.GetFileFromPathAsync(path);
            args.Request.Data.Properties.Title = title ?? file.Name;
            args.Request.Data.SetStorageItems(new IStorageItem[] { file }, readOnly: true);
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Windows share failed: {error}");
            args.Request.FailWithDisplayText(error.Message);
        }
        finally
        {
            deferral.Complete();
        }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_manager is not null)
        {
            _manager.DataRequested -= OnDataRequested;
            _manager = null;
        }
        _pendingPath = null;
        _pendingTitle = null;
    }
}
