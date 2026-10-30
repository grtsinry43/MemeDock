using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace MemeDock.Windows.Controls;

/// <summary>
/// One settings card: icon, header, optional description, and the action on the right.
/// The template lives in App.xaml as the implicit style for this type.
/// </summary>
public sealed class SettingsRow : ContentControl
{
    public static readonly DependencyProperty HeaderProperty = DependencyProperty.Register(
        nameof(Header), typeof(string), typeof(SettingsRow), new PropertyMetadata(""));

    public static readonly DependencyProperty DescriptionProperty = DependencyProperty.Register(
        nameof(Description), typeof(string), typeof(SettingsRow), new PropertyMetadata("", OnPartChanged));

    public static readonly DependencyProperty GlyphProperty = DependencyProperty.Register(
        nameof(Glyph), typeof(string), typeof(SettingsRow), new PropertyMetadata("", OnPartChanged));

    private FrameworkElement? _description;
    private FrameworkElement? _icon;

    public string Header { get => (string)GetValue(HeaderProperty); set => SetValue(HeaderProperty, value); }
    public string Description { get => (string)GetValue(DescriptionProperty); set => SetValue(DescriptionProperty, value); }
    public string Glyph { get => (string)GetValue(GlyphProperty); set => SetValue(GlyphProperty, value); }

    protected override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        _description = GetTemplateChild("DescriptionText") as FrameworkElement;
        _icon = GetTemplateChild("HeaderIcon") as FrameworkElement;
        UpdateParts();
    }

    private static void OnPartChanged(DependencyObject sender, DependencyPropertyChangedEventArgs args)
        => ((SettingsRow)sender).UpdateParts();

    private void UpdateParts()
    {
        if (_description is not null)
            _description.Visibility = string.IsNullOrEmpty(Description) ? Visibility.Collapsed : Visibility.Visible;
        if (_icon is not null)
            _icon.Visibility = string.IsNullOrEmpty(Glyph) ? Visibility.Collapsed : Visibility.Visible;
    }
}
