using Microsoft.UI.Input;
using Microsoft.UI.Xaml.Controls;

namespace MemeDock.Windows.Controls;

/// <summary>
/// Thin Grid subclass exposing <see cref="Microsoft.UI.Xaml.UIElement.ProtectedCursor"/>
/// so crop and timeline handles display native Windows resize and move cursors.
/// </summary>
public sealed class CursorGrid : Grid
{
    private InputSystemCursorShape _cursorShape = InputSystemCursorShape.Arrow;

    public InputSystemCursorShape CursorShape
    {
        get => _cursorShape;
        set
        {
            if (_cursorShape == value && ProtectedCursor is not null) return;
            _cursorShape = value;
            ProtectedCursor = InputSystemCursor.Create(value);
        }
    }
}
