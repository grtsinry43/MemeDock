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

    /// <summary>Two views that replace each other in place only cross-fade.</summary>
    public static void UseCrossFade(UIElement element) => UseEntrance(element, offset: 0);

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
