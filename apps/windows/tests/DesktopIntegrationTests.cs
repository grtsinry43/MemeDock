using System.ComponentModel;
using System.Text.Json;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using Shortcut = MemeDock.Windows.Models.GlobalShortcut;

internal static class DesktopIntegrationTests
{
    private static void Check(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
    public static async Task RunAsync()
    {
        Placement(); PanelActivation(); PasteProtection(); Shortcuts(); Lifetime(); TrayRecovery();
        await PreferencesAsync();
        Console.WriteLine("PASS: panel activation/dismissal, placement, paste protection, shortcut recording/conflicts, tray lifetime and preference compatibility");
    }
    private static void PanelActivation()
    {
        nint current = 10;
        var shown = false;
        Check(PanelFocus.Activate(20, () => shown = true, window =>
        {
            Check(shown, "Foreground was requested before showing the panel.");
            current = window; return true;
        }, () => current), "A background panel was not activated after showing.");
        current = 10;
        Check(!PanelFocus.Activate(20, () => { }, _ => false, () => current),
            "A rejected foreground request was reported as successful activation.");
        current = 20;
        Check(PanelFocus.Activate(20, () => { }, _ => throw new InvalidOperationException("Already active"), () => current),
            "An already activated panel unnecessarily requested foreground again.");
        Check(PanelFocus.ShouldDismiss(false, false, false, false, false, false), "An external click did not dismiss the panel.");
        Check(PanelFocus.ShouldDismiss(false, false, true, false, false, false), "A stale menu flag blocked external dismissal.");
        Check(!PanelFocus.ShouldDismiss(true, true, false, false, false, false), "A focused panel dismissed itself.");
        Check(!PanelFocus.ShouldDismiss(false, true, true, false, false, false), "An owned flyout dismissed its panel.");
        Check(!PanelFocus.ShouldDismiss(false, false, false, true, false, false), "A drag was interrupted by dismissal.");
        Check(!PanelFocus.ShouldDismiss(false, false, false, false, true, false), "A save dialog was interrupted by dismissal.");
        Check(!PanelFocus.ShouldDismiss(false, false, false, false, false, true), "A paste handoff was interrupted by dismissal.");
    }
    private static void Placement()
    {
        var work = new ScreenRect(-1920, -120, 1920, 1040);
        var lowerRight = PanelPlacement.Place(work, new(-20, 890, 20, 20), 480, 360, 8);
        Check(lowerRight == new ScreenRect(-480, 522, 480, 360), "Picker escaped the secondary display or failed to flip above its input.");
        var upperLeft = PanelPlacement.Place(work, new(-2200, -200, 1, 1), 480, 360, 8);
        Check(upperLeft.X == work.X && upperLeft.Y == work.Y, "Off-screen input was not constrained to the work area.");
        var small = PanelPlacement.Place(new(10, 10, 240, 180), new(20, 20, 1, 1), 480, 360, 8);
        Check(small == new ScreenRect(10, 10, 240, 180), "Panel did not fit a small work area.");
        foreach (var dpi in new[] { 96, 120, 144, 192 })
        {
            var width = 480 * dpi / 96;
            var placed = PanelPlacement.Place(work, new(-1000, 300, 1, 20), width, 360 * dpi / 96, 8 * dpi / 96);
            Check(placed.X >= work.X && placed.Y >= work.Y && placed.Right <= work.Right && placed.Bottom <= work.Bottom,
                $"Panel escaped work area at {dpi} DPI.");
        }
    }
    private static void PasteProtection()
    {
        Check(PastePolicy.Refusal(42, 1, true, false, true, 7, 7) is null, "Valid paste was refused.");
        Check(PastePolicy.Refusal(1, 1, true, false, true, 7, 7) == "picker_paste_unsupported", "Self-targeted paste was permitted.");
        Check(PastePolicy.Refusal(42, 1, true, true, true, 7, 7) == "picker_paste_unsupported", "Plain-text or password input was permitted.");
        Check(PastePolicy.Refusal(42, 1, false, false, true, 7, 7) == "picker_paste_missing", "A destroyed target was permitted.");
        Check(PastePolicy.Refusal(0, 1, true, false, true, 7, 7) == "picker_paste_missing", "Missing process was permitted.");
        Check(PastePolicy.Refusal(42, 1, true, false, false, 7, 7) == "picker_paste_changed", "Focus switched to another application but paste was permitted.");
        Check(PastePolicy.Refusal(42, 1, true, false, true, 7, 8) == "picker_clipboard_changed", "Replaced clipboard contents were pasted.");
    }
    private static void Shortcuts()
    {
        Check(new Shortcut(3, 0x4D).IsValid && new Shortcut(3, 0x4D).ToString() == "Ctrl+Alt+M", "Shortcut rendering differs from registration.");
        Check(!new Shortcut(0, 0x4D).IsValid && !new Shortcut(16, 0x4D).IsValid && !new Shortcut(3, 0x10).IsValid,
            "Bare keys, invalid modifiers or modifier-only shortcuts were accepted.");
        Check(!new Shortcut(2, 0x7B).IsValid, "The debugger-reserved F12 key was accepted.");
        foreach (var key in new uint[] { 0x08, 0x09, 0x0D, 0x1B, 0x20, 0x25, 0x2E, 0x60, 0x6B, 0x7C, 0x87, 0xBA, 0xE2 })
            Check(new Shortcut(8, key).IsValid, $"Windows shortcut dropped bindable key {key:X}.");
        Check(new Shortcut(15, 0x20).Keys.SequenceEqual(new uint[] { 0x5B, 0x11, 0x12, 0x10, 0x20 }), "Keycaps have an inconsistent modifier order.");
        Check(new Shortcut(8, 0x20).ToString() == "Win+Space" && new Shortcut(2, 0x61).ToString() == "Ctrl+Num 1",
            "Special keys were rendered as control characters.");
        var capture = new ShortcutCapture();
        capture.KeyDown(0xA2); capture.KeyDown(0xA4);
        Check(capture.KeyDown(0x20) && capture.Candidate == new Shortcut(3, 0x20), "Space shortcut was not recorded.");
        capture.KeyUp(0x20); capture.KeyUp(0xA2); capture.KeyUp(0xA4);
        Check(capture.Candidate == new Shortcut(3, 0x20), "Releasing keys erased the draft shortcut.");
        capture.Reset([]);
        Check(!capture.KeyDown(0x09) && !capture.KeyDown(0x1B), "Dialog navigation was swallowed by recording.");
        capture.KeyDown(0xA0);
        Check(!capture.KeyDown(0x09), "Shift+Tab navigation was swallowed.");
        capture.KeyDown(0x5B);
        Check(capture.KeyDown(0x25) && capture.Candidate == new Shortcut(12, 0x25), "Windows+Shift+Arrow was dropped.");
        capture.Reset([0xA2, 0xA3]); capture.KeyUp(0xA2); capture.KeyDown(0x61);
        Check(capture.Candidate == new Shortcut(2, 0x61), "Releasing left Ctrl also released held right Ctrl.");
        capture.KeyDown(0x7B);
        Check(capture.Candidate is null, "Recorder accepted debugger-reserved F12.");
        Check(!new Shortcut(2, 0x5B).IsValid && !new Shortcut(2, 0x01).IsValid && !new Shortcut(2, 0xFF).IsValid,
            "Modifier-only, mouse or unknown keys were accepted as the shortcut's main key.");
        var occupied = new Dictionary<int, Shortcut>();
        var conflict = new Shortcut(3, 0x4E);
        using var registration = new MemeDock.Windows.Services.GlobalShortcut(0,
            (id, shortcut) => { if (shortcut == conflict) return false; occupied.Add(id, shortcut); return true; },
            id => occupied.Remove(id));
        var original = new Shortcut(3, 0x4D);
        registration.Apply(original);
        var originalId = occupied.Single().Key;
        try { registration.Apply(conflict); throw new InvalidOperationException("Conflicting hotkey was accepted."); }
        catch (Win32Exception) { }
        Check(occupied.Count == 1 && occupied[originalId] == original && registration.Handles(originalId), "Conflict lost the existing shortcut.");
        registration.Apply(new(6, 0x50));
        Check(occupied.Count == 1 && !occupied.ContainsKey(originalId) && !registration.Handles(originalId), "Shortcut replacement leaked an old binding.");
        registration.Apply(null);
        Check(occupied.Count == 0, "Clearing the shortcut left a global binding installed.");
    }
    private static void Lifetime()
    {
        Check(ResidentLifetime.ShouldHide(false, true), "Closing the window terminated an available resident app.");
        Check(!ResidentLifetime.ShouldHide(true, true), "Explicit quit only hid the window.");
        Check(!ResidentLifetime.ShouldHide(false, false), "A failed tray left an unreachable resident process.");
    }
    private static void TrayRecovery()
    {
        var calls = new List<(TrayOperation Operation, bool Guid)>();
        using var legacy = new TrayRegistration((operation, guid) =>
        {
            calls.Add((operation, guid));
            return operation is TrayOperation.Add or TrayOperation.Delete;
        });
        Check(legacy.Install() && legacy.Available && !legacy.Version4,
            "Failed version negotiation removed a successfully registered icon.");
        Check(!calls.Any(call => call.Operation == TrayOperation.Delete), "Negotiation failure issued NIM_DELETE.");
        var count = calls.Count;
        legacy.Install();
        Check(calls.Count == count, "Repeated registration duplicated the resident icon.");
        legacy.TaskbarRestarted(); legacy.Install();
        Check(calls.Count(call => call.Operation == TrayOperation.Add) == 2 && legacy.Available,
            "Explorer restart did not re-register the icon.");
        legacy.Dispose(); legacy.Dispose();
        Check(calls.Count(call => call.Operation == TrayOperation.Delete) == 1, "Tray cleanup was not idempotent.");
        using var fallback = new TrayRegistration((operation, guid) => !guid && operation != TrayOperation.Modify);
        Check(fallback.Install() && !fallback.UsesGuid && fallback.Version4,
            "A GUID rejected for a moved debug binary did not fall back to window identity.");
        using var missing = new TrayRegistration((_, _) => false);
        Check(!missing.Install() && !missing.Available, "Failed registration was reported as a usable tray.");
    }
    private static async Task PreferencesAsync()
    {
        var root = Path.Combine(Path.GetTempPath(), "MemeDock.DesktopTests." + Guid.NewGuid());
        Directory.CreateDirectory(root);
        try
        {
            var path = Path.Combine(root, "preferences.json");
            await File.WriteAllTextAsync(path, "{\"Language\":\"English\",\"Appearance\":\"Dark\"}");
            var preferences = new AppPreferences(root);
            await preferences.LoadAsync();
            Check(preferences.Desktop == new DesktopPreferences() && preferences.Language == LanguageMode.English,
                "Pre-desktop preferences lost their defaults or language.");
            var desktop = new DesktopPreferences(false, false, new(3, 0x4D), true);
            await Task.WhenAll(preferences.SaveDesktopAsync(desktop), preferences.SaveLanguageAsync(LanguageMode.Chinese));
            var loaded = new AppPreferences(root); loaded.Load();
            Check(loaded.Desktop == desktop && loaded.Language == LanguageMode.Chinese && loaded.Appearance == AppearanceMode.Dark,
                "Concurrent desktop and language writes lost an unrelated preference.");
            using (var json = JsonDocument.Parse(await File.ReadAllTextAsync(path)))
            {
                Check(!json.RootElement.TryGetProperty("Token", out _), "Desktop preferences serialized a credential.");
                Check(!json.RootElement.GetProperty("Desktop").GetProperty("Shortcut").TryGetProperty("Keys", out _),
                    "Presentation keycaps leaked into persisted shortcut data.");
            }
            await File.WriteAllTextAsync(path, "{\"Language\":\"English\",\"Desktop\":{\"Shortcut\":{\"Modifiers\":16,\"Key\":77}}}");
            loaded.Load();
            Check(loaded.Desktop.Shortcut is null && loaded.Language == LanguageMode.English, "Invalid persisted shortcut was registered or invalidated language.");
            await File.WriteAllTextAsync(path, "{\"Language\":\"English\",\"Desktop\":{\"AnchorToInput\":\"bad\"}}");
            loaded.Load();
            Check(loaded.Desktop.AnchorToInput && loaded.Language == LanguageMode.English, "Damaged desktop settings invalidated unrelated preferences.");
        }
        finally { Directory.Delete(root, true); }
    }
}
