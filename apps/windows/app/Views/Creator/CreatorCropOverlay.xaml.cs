using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.Foundation;

namespace MemeDock.Windows.Views.Creator;

public sealed partial class CreatorCropOverlay : UserControl
{
    private CreatorCrop _crop = CreatorCrop.Full;
    private double? _aspectLock;
    private uint _sourceWidth = 1;
    private uint _sourceHeight = 1;
    private bool _interactive = true;

    private CreatorCrop? _dragInitial;
    private Point _dragOrigin;
    private CreatorCropHandle? _activeHandle;
    private bool _isMoving;
    private bool _movedDuringDrag;

    public event EventHandler<CreatorCrop>? CropChanged;
    public event EventHandler? AspectUnlocked;
    public event EventHandler? TappedForPlayback;
    public event EventHandler? ResetRequested;

    public CreatorCropOverlay() => InitializeComponent();

    public void Relocalize(LocalizationService strings)
    {
        AutomationProperties.SetName(MoveSurface, strings["creator_crop"]);
        var resize = strings["creator_resize_crop"];
        foreach (var handle in new UIElement[]
        {
            TopHandle, BottomHandle, LeftHandle, RightHandle,
            TopLeftHandle, TopRightHandle, BottomLeftHandle, BottomRightHandle
        })
        {
            AutomationProperties.SetName(handle, resize);
        }
    }

    public void Update(CreatorCrop crop, double? aspectLock, uint sourceWidth, uint sourceHeight, bool interactive)
    {
        _crop = crop;
        _aspectLock = aspectLock;
        _sourceWidth = Math.Max(1u, sourceWidth);
        _sourceHeight = Math.Max(1u, sourceHeight);
        _interactive = interactive;
        IsHitTestVisible = interactive;
        LayoutOverlay();
    }

    private void OnOverlaySizeChanged(object sender, SizeChangedEventArgs args) => LayoutOverlay();

    private void LayoutOverlay()
    {
        var width = ActualWidth;
        var height = ActualHeight;
        if (width <= 0 || height <= 0) return;

        var left = _crop.X * width;
        var top = _crop.Y * height;
        var rectW = Math.Max(1, _crop.Width * width);
        var rectH = Math.Max(1, _crop.Height * height);
        var right = left + rectW;
        var bottom = top + rectH;

        OuterRect.Rect = new Rect(0, 0, width, height);
        InnerRect.Rect = new Rect(left, top, rectW, rectH);

        // Move interior
        MoveSurface.Width = rectW;
        MoveSurface.Height = rectH;
        Canvas.SetLeft(MoveSurface, left);
        Canvas.SetTop(MoveSurface, top);

        // Rule-of-thirds lines
        GridV1.X1 = GridV1.X2 = left + rectW / 3.0;
        GridV1.Y1 = top; GridV1.Y2 = bottom;
        GridV2.X1 = GridV2.X2 = left + rectW * 2.0 / 3.0;
        GridV2.Y1 = top; GridV2.Y2 = bottom;
        GridH1.X1 = left; GridH1.X2 = right;
        GridH1.Y1 = GridH1.Y2 = top + rectH / 3.0;
        GridH2.X1 = left; GridH2.X2 = right;
        GridH2.Y1 = GridH2.Y2 = top + rectH * 2.0 / 3.0;

        // Edge handles
        var hLen = Math.Max(20, rectW - 28);
        var vLen = Math.Max(20, rectH - 28);

        TopHandle.Width = hLen; TopHandle.Height = 16;
        TopGrip.Width = Math.Min(24, hLen * 0.5);
        Canvas.SetLeft(TopHandle, left + (rectW - hLen) / 2.0);
        Canvas.SetTop(TopHandle, top - 8);

        BottomHandle.Width = hLen; BottomHandle.Height = 16;
        BottomGrip.Width = Math.Min(24, hLen * 0.5);
        Canvas.SetLeft(BottomHandle, left + (rectW - hLen) / 2.0);
        Canvas.SetTop(BottomHandle, bottom - 8);

        LeftHandle.Width = 16; LeftHandle.Height = vLen;
        LeftGrip.Height = Math.Min(24, vLen * 0.5);
        Canvas.SetLeft(LeftHandle, left - 8);
        Canvas.SetTop(LeftHandle, top + (rectH - vLen) / 2.0);

        RightHandle.Width = 16; RightHandle.Height = vLen;
        RightGrip.Height = Math.Min(24, vLen * 0.5);
        Canvas.SetLeft(RightHandle, right - 8);
        Canvas.SetTop(RightHandle, top + (rectH - vLen) / 2.0);

        // Corner handles (28x28 centered at each corner)
        Canvas.SetLeft(TopLeftHandle, left - 14); Canvas.SetTop(TopLeftHandle, top - 14);
        Canvas.SetLeft(TopRightHandle, right - 14); Canvas.SetTop(TopRightHandle, top - 14);
        Canvas.SetLeft(BottomLeftHandle, left - 14); Canvas.SetTop(BottomLeftHandle, bottom - 14);
        Canvas.SetLeft(BottomRightHandle, right - 14); Canvas.SetTop(BottomRightHandle, bottom - 14);

        // Live pixel readout badge
        var px = CreatorMath.CroppedPixels(_sourceWidth, _sourceHeight, _crop);
        SizeBadgeText.Text = $"{px.Width} × {px.Height}";
        SizeBadge.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        var badgeW = SizeBadge.DesiredSize.Width;
        var badgeH = SizeBadge.DesiredSize.Height;
        var badgeX = Math.Clamp(left + (rectW - badgeW) / 2.0, 6, Math.Max(6, width - badgeW - 6));
        var badgeY = top > badgeH + 10 ? top - badgeH - 8 : top + 8;
        Canvas.SetLeft(SizeBadge, badgeX);
        Canvas.SetTop(SizeBadge, badgeY);
    }

    private void SetDraggingVisual(bool active)
    {
        GridLines.Opacity = active ? 1 : 0;
        SizeBadge.Opacity = active ? 1 : 0;
    }

    private void OnMovePressed(object sender, PointerRoutedEventArgs args)
    {
        if (!_interactive || !args.GetCurrentPoint(OverlayCanvas).Properties.IsLeftButtonPressed) return;
        _isMoving = true;
        _activeHandle = null;
        _movedDuringDrag = false;
        _dragInitial = _crop;
        _dragOrigin = args.GetCurrentPoint(OverlayCanvas).Position;
        MoveSurface.CapturePointer(args.Pointer);
        args.Handled = true;
    }

    private void OnMoveMoved(object sender, PointerRoutedEventArgs args)
    {
        if (!_isMoving || _dragInitial is not { } origin) return;
        var pos = args.GetCurrentPoint(OverlayCanvas).Position;
        var dxPx = pos.X - _dragOrigin.X;
        var dyPx = pos.Y - _dragOrigin.Y;
        if (!_movedDuringDrag && double.Hypot(dxPx, dyPx) >= 3)
        {
            _movedDuringDrag = true;
            SetDraggingVisual(true);
        }
        if (!_movedDuringDrag) return;
        var next = CreatorMath.MoveCrop(origin, dxPx / Math.Max(1, ActualWidth), dyPx / Math.Max(1, ActualHeight));
        _crop = next;
        LayoutOverlay();
        CropChanged?.Invoke(this, next);
        args.Handled = true;
    }

    private void OnMoveReleased(object sender, PointerRoutedEventArgs args)
    {
        if (!_isMoving) return;
        var wasTap = !_movedDuringDrag;
        EndDrag(MoveSurface, args.Pointer);
        if (wasTap) TappedForPlayback?.Invoke(this, EventArgs.Empty);
        args.Handled = true;
    }

    private void OnMoveDoubleTapped(object sender, DoubleTappedRoutedEventArgs args)
    {
        if (!_interactive) return;
        ResetRequested?.Invoke(this, EventArgs.Empty);
        args.Handled = true;
    }

    private void OnHandlePressed(object sender, PointerRoutedEventArgs args)
    {
        if (!_interactive || sender is not FrameworkElement { Tag: string tag } element
            || !args.GetCurrentPoint(OverlayCanvas).Properties.IsLeftButtonPressed) return;
        if (!Enum.TryParse<CreatorCropHandle>(tag, out var handle)) return;
        _activeHandle = handle;
        _isMoving = false;
        _dragInitial = _crop;
        _dragOrigin = args.GetCurrentPoint(OverlayCanvas).Position;
        element.CapturePointer(args.Pointer);
        SetDraggingVisual(true);
        args.Handled = true;
    }

    private void OnHandleMoved(object sender, PointerRoutedEventArgs args)
    {
        if (_activeHandle is not { } handle || _dragInitial is not { } origin) return;
        var pos = args.GetCurrentPoint(OverlayCanvas).Position;
        var dx = (pos.X - _dragOrigin.X) / Math.Max(1, ActualWidth);
        var dy = (pos.Y - _dragOrigin.Y) / Math.Max(1, ActualHeight);

        if (!handle.IsCorner() && _aspectLock is not null)
        {
            _aspectLock = null;
            AspectUnlocked?.Invoke(this, EventArgs.Empty);
        }

        var next = CreatorMath.ResizeCrop(origin, handle, dx, dy, _aspectLock, _sourceWidth, _sourceHeight);
        _crop = next;
        LayoutOverlay();
        CropChanged?.Invoke(this, next);
        args.Handled = true;
    }

    private void OnHandleReleased(object sender, PointerRoutedEventArgs args)
    {
        if (_activeHandle is null || sender is not UIElement element) return;
        EndDrag(element, args.Pointer);
        args.Handled = true;
    }

    private void OnPointerCaptureLost(object sender, PointerRoutedEventArgs args)
    {
        _isMoving = false;
        _activeHandle = null;
        _dragInitial = null;
        _movedDuringDrag = false;
        SetDraggingVisual(false);
    }

    private void EndDrag(UIElement element, Pointer pointer)
    {
        element.ReleasePointerCapture(pointer);
        _isMoving = false;
        _activeHandle = null;
        _dragInitial = null;
        _movedDuringDrag = false;
        SetDraggingVisual(false);
    }
}
