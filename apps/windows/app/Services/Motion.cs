using System.Numerics;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.UI.ViewManagement;

namespace MemeDock.Windows.Services;

/// <summary>
/// Fluent motion values (Windows motion guidance): direct entrance cubic-bezier(0,0,0,1)
/// at 250 ms, exits faded out at 83 ms linear. Every animation is skipped when the
/// system "Animation effects" setting is off.
/// </summary>
internal static class Motion
{
    private static readonly UISettings Settings = new();
    private static readonly TimeSpan Entrance = TimeSpan.FromMilliseconds(250);
    private static readonly TimeSpan Fade = TimeSpan.FromMilliseconds(167);
    private static readonly TimeSpan Exit = TimeSpan.FromMilliseconds(83);
    private static readonly TimeSpan WorkspaceExit = TimeSpan.FromMilliseconds(120);
    private static readonly TimeSpan Swap = TimeSpan.FromMilliseconds(200);

    public static bool Enabled => Settings.AnimationsEnabled;

    /// <summary>
    /// Shown elements rise <paramref name="offset"/> epx while fading in; hidden ones fade out.
    /// The animations run whenever Visibility changes, so callers keep toggling Visibility.
    /// </summary>
    public static void UseEntrance(UIElement element, float offset = 16)
    {
        if (!Enabled) return;
        var compositor = ElementCompositionPreview.GetElementVisual(element).Compositor;
        ElementCompositionPreview.SetIsTranslationEnabled(element, true);
        var show = compositor.CreateAnimationGroup();
        show.Add(Opacity(compositor, 0, 1, Fade, compositor.CreateLinearEasingFunction()));
        if (offset != 0)
        {
            var slide = compositor.CreateVector3KeyFrameAnimation();
            slide.Target = "Translation";
            slide.Duration = Entrance;
            slide.InsertKeyFrame(0, new Vector3(0, offset, 0));
            slide.InsertKeyFrame(1, Vector3.Zero, Decelerate(compositor));
            show.Add(slide);
        }
        ElementCompositionPreview.SetImplicitShowAnimation(element, show);
        ElementCompositionPreview.SetImplicitHideAnimation(element,
            Opacity(compositor, 1, 0, Exit, compositor.CreateLinearEasingFunction()));
    }

    /// <summary>
    /// Subtle Fluent depth-and-glide transition for switching between top-level workspaces
    /// (Library and Meme Creator). Keeps Mica unobstructed and respects system motion settings.
    /// </summary>
    public static void UseWorkspaceTransition(FrameworkElement element, bool isCreatorLayer)
    {
        if (!Enabled) return;
        var visual = ElementCompositionPreview.GetElementVisual(element);
        var compositor = visual.Compositor;
        ElementCompositionPreview.SetIsTranslationEnabled(element, true);

        element.SizeChanged += (_, args) =>
        {
            visual.CenterPoint = new Vector3((float)args.NewSize.Width / 2f, (float)args.NewSize.Height / 2f, 0f);
        };

        var enterOffset = isCreatorLayer ? 18f : -10f;
        var exitOffset = isCreatorLayer ? 12f : -8f;
        var enterScale = isCreatorLayer ? 0.985f : 0.99f;

        var show = compositor.CreateAnimationGroup();
        show.Add(Opacity(compositor, 0, 1, Fade, compositor.CreateLinearEasingFunction()));

        var showSlide = compositor.CreateVector3KeyFrameAnimation();
        showSlide.Target = "Translation";
        showSlide.Duration = Entrance;
        showSlide.InsertKeyFrame(0, new Vector3(0, enterOffset, 0));
        showSlide.InsertKeyFrame(1, Vector3.Zero, Decelerate(compositor));
        show.Add(showSlide);

        var showScale = compositor.CreateVector3KeyFrameAnimation();
        showScale.Target = "Scale";
        showScale.Duration = Entrance;
        showScale.InsertKeyFrame(0, new Vector3(enterScale, enterScale, 1f));
        showScale.InsertKeyFrame(1, Vector3.One, Decelerate(compositor));
        show.Add(showScale);

        var hide = compositor.CreateAnimationGroup();
        hide.Add(Opacity(compositor, 1, 0, WorkspaceExit, compositor.CreateLinearEasingFunction()));
        var hideSlide = compositor.CreateVector3KeyFrameAnimation();
        hideSlide.Target = "Translation";
        hideSlide.Duration = WorkspaceExit;
        hideSlide.InsertKeyFrame(0, Vector3.Zero);
        hideSlide.InsertKeyFrame(1, new Vector3(0, exitOffset, 0), compositor.CreateLinearEasingFunction());
        hide.Add(hideSlide);

        ElementCompositionPreview.SetImplicitShowAnimation(element, show);
        ElementCompositionPreview.SetImplicitHideAnimation(element, hide);
    }

    /// <summary>Two views that replace each other in place only cross-fade.</summary>
    public static void UseCrossFade(UIElement element) => UseEntrance(element, offset: 0);

    /// <summary>
    /// Smoothly glides and fades updated inline content (brand title swap, sync status text).
    /// </summary>
    public static void AnimateContentSwap(UIElement element, float fromOffsetY = 6f)
    {
        if (!Enabled) return;
        var visual = ElementCompositionPreview.GetElementVisual(element);
        var compositor = visual.Compositor;
        ElementCompositionPreview.SetIsTranslationEnabled(element, true);

        var opacity = Opacity(compositor, 0f, 1f, Swap, Decelerate(compositor));
        visual.StartAnimation("Opacity", opacity);

        if (fromOffsetY != 0f)
        {
            var slide = compositor.CreateVector3KeyFrameAnimation();
            slide.Target = "Translation";
            slide.Duration = Swap;
            slide.InsertKeyFrame(0, new Vector3(0, fromOffsetY, 0));
            slide.InsertKeyFrame(1, Vector3.Zero, Decelerate(compositor));
            visual.StartAnimation("Translation", slide);
        }
    }

    /// <summary>Smoothly rotates a small icon around its center (e.g., dropdown chevron).</summary>
    public static void AnimateRotation(FrameworkElement element, float degrees, int durationMs = 200)
    {
        var visual = ElementCompositionPreview.GetElementVisual(element);
        visual.CenterPoint = new Vector3((float)element.ActualWidth / 2f, (float)element.ActualHeight / 2f, 0f);
        if (!Enabled)
        {
            visual.StopAnimation("RotationAngleInDegrees");
            visual.RotationAngleInDegrees = degrees;
            return;
        }
        var compositor = visual.Compositor;
        var animation = compositor.CreateScalarKeyFrameAnimation();
        animation.Duration = TimeSpan.FromMilliseconds(durationMs);
        animation.InsertKeyFrame(1f, degrees, Decelerate(compositor));
        visual.StartAnimation("RotationAngleInDegrees", animation);
    }

    /// <summary>Starts a smooth infinite rotation on a sync indicator glyph.</summary>
    public static void StartContinuousSpin(FrameworkElement element)
    {
        var visual = ElementCompositionPreview.GetElementVisual(element);
        if (!Enabled)
        {
            visual.StopAnimation("RotationAngleInDegrees");
            visual.RotationAngleInDegrees = 0f;
            return;
        }
        var w = element.ActualWidth > 0 ? (float)element.ActualWidth : 14f;
        var h = element.ActualHeight > 0 ? (float)element.ActualHeight : 14f;
        visual.CenterPoint = new Vector3(w / 2f, h / 2f, 0f);
        var compositor = visual.Compositor;
        var spin = compositor.CreateScalarKeyFrameAnimation();
        spin.InsertKeyFrame(0f, 0f);
        spin.InsertKeyFrame(1f, 360f, compositor.CreateLinearEasingFunction());
        spin.Duration = TimeSpan.FromMilliseconds(1350);
        spin.IterationBehavior = AnimationIterationBehavior.Forever;
        visual.StartAnimation("RotationAngleInDegrees", spin);
    }

    public static void StopSpin(FrameworkElement element)
    {
        var visual = ElementCompositionPreview.GetElementVisual(element);
        visual.StopAnimation("RotationAngleInDegrees");
        visual.RotationAngleInDegrees = 0f;
    }

    /// <summary>
    /// Smoothly glides a side inspector pane into place from the right edge using Fluent
    /// decelerate timing. Runs off the UI thread and triggers deterministically on every open.
    /// </summary>
    public static void AnimateSlideIn(UIElement element, float fromOffsetX = 24f, int durationMs = 240)
    {
        if (!Enabled) return;
        var visual = ElementCompositionPreview.GetElementVisual(element);
        var compositor = visual.Compositor;
        ElementCompositionPreview.SetIsTranslationEnabled(element, true);

        visual.StartAnimation("Opacity", Opacity(compositor, 0f, 1f, Fade, compositor.CreateLinearEasingFunction()));

        var slide = compositor.CreateVector3KeyFrameAnimation();
        slide.Target = "Translation";
        slide.Duration = TimeSpan.FromMilliseconds(durationMs);
        slide.InsertKeyFrame(0, new Vector3(fromOffsetX, 0, 0));
        slide.InsertKeyFrame(1, Vector3.Zero, Decelerate(compositor));
        visual.StartAnimation("Translation", slide);
    }

    /// <summary>
    /// Gentle centered scale + lift settle for sticker preview switches, giving every
    /// selection change tactile Fluent feedback without detaching tiles from the gallery.
    /// </summary>
    public static void AnimatePreviewReveal(FrameworkElement element, int durationMs = 220)
    {
        if (!Enabled) return;
        var visual = ElementCompositionPreview.GetElementVisual(element);
        var compositor = visual.Compositor;
        ElementCompositionPreview.SetIsTranslationEnabled(element, true);
        var cx = element.ActualWidth > 0 ? (float)element.ActualWidth / 2f : 100f;
        var cy = element.ActualHeight > 0 ? (float)element.ActualHeight / 2f : 100f;
        visual.CenterPoint = new Vector3(cx, cy, 0f);

        var duration = TimeSpan.FromMilliseconds(durationMs);
        var easing = Decelerate(compositor);

        var scale = compositor.CreateVector3KeyFrameAnimation();
        scale.Target = "Scale";
        scale.Duration = duration;
        scale.InsertKeyFrame(0, new Vector3(0.94f, 0.94f, 1f));
        scale.InsertKeyFrame(1, Vector3.One, easing);
        visual.StartAnimation("Scale", scale);

        var lift = compositor.CreateVector3KeyFrameAnimation();
        lift.Target = "Translation";
        lift.Duration = duration;
        lift.InsertKeyFrame(0, new Vector3(0, 6f, 0));
        lift.InsertKeyFrame(1, Vector3.Zero, easing);
        visual.StartAnimation("Translation", lift);

        visual.StartAnimation("Opacity", Opacity(compositor, 0.35f, 1f, TimeSpan.FromMilliseconds(160), easing));
    }

    /// <summary>
    /// Smoothly cross-fades a decoded full-resolution image over its thumbnail stand-in
    /// so transparent stickers never flash blank or double-blend during decode handoff.
    /// </summary>
    public static void AnimateImageHandoff(UIElement incoming, UIElement outgoing, int durationMs = 140)
    {
        var incomingVisual = ElementCompositionPreview.GetElementVisual(incoming);
        var outgoingVisual = ElementCompositionPreview.GetElementVisual(outgoing);
        if (!Enabled)
        {
            incomingVisual.StopAnimation("Opacity");
            outgoingVisual.StopAnimation("Opacity");
            incomingVisual.Opacity = 1f;
            outgoingVisual.Opacity = 0f;
            return;
        }
        var compositor = incomingVisual.Compositor;
        var duration = TimeSpan.FromMilliseconds(durationMs);
        var linear = compositor.CreateLinearEasingFunction();
        incomingVisual.StartAnimation("Opacity", Opacity(compositor, 0f, 1f, duration, linear));
        outgoingVisual.StartAnimation("Opacity", Opacity(compositor, outgoingVisual.Opacity, 0f, duration, linear));
    }

    public static void SetVisualOpacity(UIElement element, float opacity)
    {
        var visual = ElementCompositionPreview.GetElementVisual(element);
        visual.StopAnimation("Opacity");
        visual.Opacity = opacity;
    }

    /// <summary>Connected animations share the entrance timing with the rest of the app.</summary>
    public static void ConfigureConnectedAnimations(UIElement anyElement)
    {
        var service = ConnectedAnimationService.GetForCurrentView();
        service.DefaultDuration = Entrance;
        service.DefaultEasingFunction = Decelerate(ElementCompositionPreview.GetElementVisual(anyElement).Compositor);
    }

    private static CubicBezierEasingFunction Decelerate(Compositor compositor)
        => compositor.CreateCubicBezierEasingFunction(new Vector2(0, 0), new Vector2(0, 1));

    private static ScalarKeyFrameAnimation Opacity(Compositor compositor, float from, float to, TimeSpan duration,
        CompositionEasingFunction easing)
    {
        var animation = compositor.CreateScalarKeyFrameAnimation();
        animation.Target = "Opacity";
        animation.Duration = duration;
        animation.InsertKeyFrame(0, from);
        animation.InsertKeyFrame(1, to, easing);
        return animation;
    }
}
