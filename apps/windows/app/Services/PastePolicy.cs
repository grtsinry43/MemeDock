namespace MemeDock.Windows.Services;

public static class PastePolicy
{
    public static string? Refusal(int targetProcess, int ownProcess, bool targetExists, bool unsupported,
        bool foregroundMatches, uint expectedClipboard, uint currentClipboard)
    {
        if (targetProcess == ownProcess || unsupported) return "picker_paste_unsupported";
        if (!targetExists || targetProcess == 0) return "picker_paste_missing";
        if (!foregroundMatches) return "picker_paste_changed";
        return expectedClipboard != currentClipboard ? "picker_clipboard_changed" : null;
    }
}
