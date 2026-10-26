using System.Diagnostics;
using System.Text.Json;
using System.Text.Json.Serialization;
using MemeDock.Bridge;
using MemeDock.Windows.Models;

namespace MemeDock.Windows.Services;

public enum LanguageMode { System, Chinese, English }

public sealed class AppPreferences : IOutputDefaults
{
    private const string OutputKey = "Output";
    private const string CopyKey = "Copy";
    private const string DragKey = "Drag";
    private const string StillKey = "Still";
    private const string AnimatedKey = "Animated";

    // Enum names instead of ordinals: a hand-edited file stays readable and a value
    // added to any of these enums cannot silently change what an existing file means.
    private static readonly JsonSerializerOptions Json = new() { Converters = { new JsonStringEnumConverter() } };

    private readonly string _path;
    private readonly SemaphoreSlim _writes = new(1);
    public LanguageMode Language { get; private set; }
    public AppearanceMode Appearance { get; private set; }
    public OutputDefaults CopyOutput { get; private set; } = OutputDefaults.CopyDefault;
    public OutputDefaults DragOutput { get; private set; } = OutputDefaults.DragDefault;
    OutputDefaults IOutputDefaults.Copy => CopyOutput;
    OutputDefaults IOutputDefaults.Drag => DragOutput;

    public AppPreferences(string root) => _path = Path.Combine(root, "preferences.json");

    /// <summary>
    /// Reads the file before the first frame is rendered. The file is a few hundred
    /// bytes of local JSON, so a synchronous read here avoids showing the wrong theme
    /// for one frame while the window loads.
    /// </summary>
    public void Load()
    {
        try
        {
            if (File.Exists(_path)) Apply(File.ReadAllText(_path));
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException)
        {
            // A damaged preference file never prevents opening the business library.
            Trace.WriteLine($"Preferences could not be loaded: {error}");
        }
    }

    public async Task LoadAsync()
    {
        try
        {
            if (File.Exists(_path)) Apply(await File.ReadAllTextAsync(_path));
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException)
        {
            // A damaged preference file never prevents opening the business library.
            Trace.WriteLine($"Preferences could not be loaded: {error}");
        }
    }

    public Task SaveLanguageAsync(LanguageMode language)
    {
        if (!Enum.IsDefined(language)) throw new ArgumentOutOfRangeException(nameof(language));
        return SaveAsync(language, null, null);
    }

    public Task SaveAppearanceAsync(AppearanceMode appearance)
    {
        if (!Enum.IsDefined(appearance)) throw new ArgumentOutOfRangeException(nameof(appearance));
        return SaveAsync(null, appearance, null);
    }

    public Task SaveOutputPresetAsync(OutputAction action, bool animated, ExportPreset preset)
    {
        if (!Enum.IsDefined(action)) throw new ArgumentOutOfRangeException(nameof(action));
        if (!StickerOutputPresets.Supports(preset, animated))
            throw new ArgumentOutOfRangeException(nameof(preset), preset, "Preset does not support this content kind.");
        return SaveAsync(null, null, (action, animated, preset));
    }

    /// <summary>
    /// Applies one change at a time so concurrent saves of different fields compose
    /// instead of overwriting each other with a stale snapshot.
    /// </summary>
    private async Task SaveAsync(LanguageMode? language, AppearanceMode? appearance,
        (OutputAction Action, bool Animated, ExportPreset Preset)? output)
    {
        await _writes.WaitAsync();
        var temporary = _path + ".tmp";
        try
        {
            var nextLanguage = language ?? Language;
            var nextAppearance = appearance ?? Appearance;
            var nextCopy = CopyOutput;
            var nextDrag = DragOutput;
            if (output is { } preset)
            {
                if (preset.Action == OutputAction.Copy)
                    nextCopy = nextCopy.WithPreset(preset.Animated, preset.Preset)
                        .Sanitized(OutputDefaults.CopyDefault);
                else
                    nextDrag = nextDrag.WithPreset(preset.Animated, preset.Preset)
                        .Sanitized(OutputDefaults.DragDefault);
            }

            var saved = new SavedPreferences(nextLanguage, nextAppearance,
                new SavedOutput(new SavedDefaults(nextCopy.Still, nextCopy.Animated),
                    new SavedDefaults(nextDrag.Still, nextDrag.Animated)));
            Directory.CreateDirectory(Path.GetDirectoryName(_path)!);
            await using (var stream = new FileStream(temporary, FileMode.Create, FileAccess.Write, FileShare.None,
                4096, FileOptions.Asynchronous | FileOptions.WriteThrough))
            {
                await JsonSerializer.SerializeAsync(stream, saved, Json);
                await stream.FlushAsync();
            }
            File.Move(temporary, _path, overwrite: true);

            // The in-memory state only advances after the new file is in place, so a
            // failed write leaves both the file and this instance on the old value.
            Language = nextLanguage;
            Appearance = nextAppearance;
            CopyOutput = nextCopy;
            DragOutput = nextDrag;
        }
        finally
        {
            try { File.Delete(temporary); }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException) { Trace.WriteLine(error); }
            _writes.Release();
        }
    }

    private void Apply(string json)
    {
        JsonDocument document;
        try { document = JsonDocument.Parse(json); }
        catch (JsonException error)
        {
            Trace.WriteLine($"Preferences are not valid JSON: {error}");
            return;
        }
        using (document)
        {
            var root = document.RootElement;
            if (root.ValueKind != JsonValueKind.Object) return;
            // Older files only contain Language; every other field keeps its default
            // instead of the whole file being rejected.
            Language = ReadEnum(root, nameof(Language), LanguageMode.System);
            Appearance = ReadEnum(root, nameof(Appearance), AppearanceMode.System);
            CopyOutput = OutputDefaults.CopyDefault;
            DragOutput = OutputDefaults.DragDefault;
            if (root.TryGetProperty(OutputKey, out var output) && output.ValueKind == JsonValueKind.Object)
            {
                CopyOutput = ReadDefaults(output, CopyKey, OutputDefaults.CopyDefault);
                DragOutput = ReadDefaults(output, DragKey, OutputDefaults.DragDefault);
            }
        }
    }

    private static OutputDefaults ReadDefaults(JsonElement parent, string name, OutputDefaults fallback)
    {
        if (!parent.TryGetProperty(name, out var element) || element.ValueKind != JsonValueKind.Object)
            return fallback;
        var still = ReadEnum(element, StillKey, fallback.Still);
        var animated = ReadEnum(element, AnimatedKey, fallback.Animated);
        return new OutputDefaults(still, animated).Sanitized(fallback);
    }

    private static T ReadEnum<T>(JsonElement parent, string name, T fallback) where T : struct, Enum
    {
        if (!parent.TryGetProperty(name, out var element)) return fallback;
        if (element.ValueKind == JsonValueKind.Number && element.TryGetInt32(out var number))
        {
            var candidate = (T)Enum.ToObject(typeof(T), number);
            return Enum.IsDefined(candidate) ? candidate : fallback;
        }
        if (element.ValueKind == JsonValueKind.String
            && Enum.TryParse<T>(element.GetString(), ignoreCase: true, out var parsed)
            && Enum.IsDefined(parsed))
            return parsed;
        return fallback;
    }

    private sealed record SavedPreferences(LanguageMode Language, AppearanceMode Appearance, SavedOutput Output);

    private sealed record SavedOutput(SavedDefaults Copy, SavedDefaults Drag);

    private sealed record SavedDefaults(ExportPreset Still, ExportPreset Animated);
}
