using System.Diagnostics;
using System.Text.Json;

namespace MemeDock.Windows.Services;

public enum LanguageMode { System, Chinese, English }

public sealed class AppPreferences
{
    private readonly string _path;
    private readonly SemaphoreSlim _writes = new(1);
    public LanguageMode Language { get; private set; }

    public AppPreferences(string root) => _path = Path.Combine(root, "preferences.json");

    public async Task LoadAsync()
    {
        Language = LanguageMode.System;
        try
        {
            await using var stream = File.OpenRead(_path);
            var saved = await JsonSerializer.DeserializeAsync<SavedPreferences>(stream);
            Language = saved is not null && Enum.IsDefined(saved.Language) ? saved.Language : LanguageMode.System;
        }
        catch (FileNotFoundException) { }
        catch (DirectoryNotFoundException) { }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException)
        {
            // A damaged preference file never prevents opening the business library.
            Trace.WriteLine($"Preferences could not be loaded: {error}");
        }
    }

    public async Task SaveLanguageAsync(LanguageMode language)
    {
        if (!Enum.IsDefined(language)) throw new ArgumentOutOfRangeException(nameof(language));
        await _writes.WaitAsync();
        var temporary = _path + ".tmp";
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(_path)!);
            await using (var stream = new FileStream(temporary, FileMode.Create, FileAccess.Write, FileShare.None,
                4096, FileOptions.Asynchronous | FileOptions.WriteThrough))
            {
                await JsonSerializer.SerializeAsync(stream, new SavedPreferences(language));
                await stream.FlushAsync();
            }
            File.Move(temporary, _path, overwrite: true);
            Language = language;
        }
        finally
        {
            try { File.Delete(temporary); }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException) { Trace.WriteLine(error); }
            _writes.Release();
        }
    }

    private sealed record SavedPreferences(LanguageMode Language);
}
