using System.Globalization;
using System.Buffers.Binary;
using System.IO.Compression;
using System.Text.RegularExpressions;
using System.Xml.Linq;
using MemeDock.Bridge;
using MemeDock.Windows;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;

internal static class Program
{
    private static async Task<int> Main()
    {
        try
        {
            QueryScopes();
            RequestGenerations();
            ResourceParity(FindRepository());
            await PreferencesAsync();
            await SessionQueriesAsync();
            Console.WriteLine("PASS: Windows browsing state, localization and preference checks");
            return 0;
        }
        catch (Exception error) { Console.Error.WriteLine(error); return 1; }
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }

    private static void QueryScopes()
    {
        var all = LibraryFilter.All.Query("request", "  猫 cat  ", StickerSort.Recent);
        Check(all.Text == "猫 cat" && all.CollectionId is null && all.TagIds.Length == 0 && all.Starred is null && !all.Deleted,
            "All-library search acquired an implicit organization or deletion filter.");
        var starred = new LibraryFilter(LibraryScope.Starred).Query("request", "", StickerSort.Recent);
        Check(starred.Starred == true && starred.CollectionId is null, "Starred filter did not reach the native query.");
        var recent = new LibraryFilter(LibraryScope.Recent).Query("request", "", StickerSort.Recent);
        Check(recent.Sort == StickerSort.LastUsed, "Recently used was queried as recently imported.");
        var collection = new LibraryFilter(LibraryScope.Collection, "collection-id", "合集");
        var ordered = collection.Query("request", "猫", StickerSort.CollectionOrder);
        Check(ordered.CollectionId == "collection-id" && ordered.Text == "猫" && ordered.Sort == StickerSort.CollectionOrder,
            "Search escaped its collection or lost collection ordering.");
        var tag = new LibraryFilter(LibraryScope.Tag, "tag-id").Query("request", "", StickerSort.CollectionOrder);
        Check(tag.TagIds.SequenceEqual(["tag-id"]) && tag.Sort == StickerSort.Recent && tag.CollectionId is null,
            "Tag filtering leaked collection ordering.");
        try
        {
            new LibraryFilter(LibraryScope.Collection).Query("request", "", StickerSort.Recent);
            throw new InvalidOperationException("An unidentified collection became an all-library query.");
        }
        catch (ArgumentException) { }
        Console.WriteLine("PASS: native query scoping and sort rules");
    }

    private static void RequestGenerations()
    {
        var state = new BrowseRequestState();
        var oldSearch = state.Begin();
        var newSearch = state.Begin();
        Check(!state.IsCurrent(oldSearch) && state.IsCurrent(newSearch), "A late search could replace newer results.");
        state.Invalidate();
        Check(!state.IsCurrent(newSearch), "Closing the library still admitted a pending result.");
        Console.WriteLine("PASS: stale search and shutdown result rejection");
    }

    private static string FindRepository()
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory); directory is not null; directory = directory.Parent)
            if (File.Exists(Path.Combine(directory.FullName, "AGENTS.md")) && Directory.Exists(Path.Combine(directory.FullName, "apps/android")))
                return directory.FullName;
        throw new DirectoryNotFoundException("Run the checks from a checkout of MemeDock.");
    }

    private static Dictionary<string, string> ReadResw(string path)
    {
        var entries = XDocument.Load(path).Root!.Elements("data").ToArray();
        Check(entries.Length == entries.Select(entry => (string)entry.Attribute("name")!).Distinct().Count(), $"Duplicate resource keys: {path}");
        return entries.ToDictionary(entry => (string)entry.Attribute("name")!, entry => entry.Element("value")!.Value);
    }

    private static void ResourceParity(string root)
    {
        var chinese = ReadResw(Path.Combine(root, "apps/windows/app/Strings/zh-CN/Resources.resw"));
        var english = ReadResw(Path.Combine(root, "apps/windows/app/Strings/en-US/Resources.resw"));
        Check(chinese.Keys.Order().SequenceEqual(english.Keys.Order()), "Chinese and English resource keys differ.");
        foreach (var key in chinese.Keys)
        {
            var zhArguments = Regex.Matches(chinese[key], @"\{\d+\}").Select(match => match.Value).Order();
            var enArguments = Regex.Matches(english[key], @"\{\d+\}").Select(match => match.Value).Order();
            Check(zhArguments.SequenceEqual(enArguments), $"Placeholder mismatch: {key}");
            Check(!string.IsNullOrWhiteSpace(chinese[key]) && !string.IsNullOrWhiteSpace(english[key]), $"Empty resource: {key}");
        }
        var desktopKeys = new Dictionary<string, string>
        {
            ["desktop_settings"] = "settings", ["desktop_sort"] = "sortHint", ["desktop_sort_recent"] = "sortRecent",
            ["desktop_sort_collection"] = "sortCollectionOrder", ["desktop_select_all"] = "selectAll",
            ["desktop_clear_selection"] = "clearSelection", ["desktop_selection_loaded_only"] = "selectionLoadedOnly"
        };
        var macos = File.ReadAllText(Path.Combine(root, "apps/macos/Sources/MemeDock/Support/Localization.swift"));
        var split = macos.IndexOf("private static let chinese:", StringComparison.Ordinal);
        foreach (var (locale, resources, desktopSection) in new[]
        {
            ("values", chinese, macos[split..]), ("values-en", english, macos[..split])
        })
        {
            var android = XDocument.Load(Path.Combine(root, $"apps/android/app/src/main/res/{locale}/strings.xml"))
                .Root!.Elements("string").ToDictionary(entry => (string)entry.Attribute("name")!, entry => entry.Value);
            foreach (var (key, value) in resources)
            {
                if (desktopKeys.TryGetValue(key, out var sourceKey))
                {
                    var match = Regex.Match(desktopSection, "\\." + sourceKey + ": \"([^\"]*)\"");
                    Check(match.Success && match.Groups[1].Value == value, $"Desktop copy differs from existing macOS copy: {key}/{locale}");
                    continue;
                }
                Check(android.TryGetValue(key, out var source), $"Resource has no approved Android/desktop source: {key}");
                var expected = Regex.Replace(source!, @"%(\d+)\$[ds]", match =>
                    "{" + (int.Parse(match.Groups[1].Value, CultureInfo.InvariantCulture) - 1) + "}");
                Check(expected == value, $"Windows copy differs from Android: {key}/{locale}");
            }
        }
        Console.WriteLine($"PASS: {chinese.Count} bilingual resource keys, placeholders and source-copy parity");
    }

    private static async Task SessionQueriesAsync()
    {
        var parent = Path.TrimEndingDirectorySeparator(Path.GetFullPath(Path.GetTempPath()));
        var root = Path.Combine(parent, "MemeDock-Windows-session-" + Guid.NewGuid().ToString("N"));
        try
        {
            var configuration = new LibraryConfiguration(Path.Combine(root, "data"), Path.Combine(root, "cache"),
                Path.Combine(root, "share"), MemeDockNative.DefaultResourceConfiguration());
            string collectionId;
            string tagId;
            using (var library = await MemeDockNative.OpenLibrary(configuration))
            {
                using var collection = library.CreateCollection("中文合集", null);
                collectionId = (await collection.AwaitResult()).Id;
                using var tag = library.CreateTag("猫");
                tagId = (await tag.AwaitResult()).Id;
                await library.Shutdown();
            }
            var redPath = Path.Combine(root, "猫🙂.png");
            var greenPath = Path.Combine(root, "green.png");
            await File.WriteAllBytesAsync(redPath, Png(255, 0, 0));
            await File.WriteAllBytesAsync(greenPath, Png(0, 255, 0));
            await using (var session = await LibrarySession.OpenAsync(root))
            {
                var first = await session.ImportAsync(redPath, collectionId, CancellationToken.None);
                await session.ImportAsync(greenPath, null, CancellationToken.None);
                Check((await session.CollectionsAsync(CancellationToken.None)).Single().Name == "中文合集", "Collection catalog did not cross the adapter.");
                Check((await session.TagsAsync(CancellationToken.None)).Single().Id == tagId, "Tag catalog did not cross the adapter.");
                using var all = await session.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, CancellationToken.None);
                Check(all.Stickers.Length == 2, "Adapter did not return both real imports.");
                using var scoped = await session.QueryAsync(new(LibraryScope.Collection, collectionId), "猫", StickerSort.CollectionOrder, null, CancellationToken.None);
                Check(scoped.Stickers.Length == 1 && scoped.Stickers[0].Id == first.Sticker.Id, "Collection search returned unrelated stickers.");
                using var noMatch = await session.QueryAsync(new(LibraryScope.Collection, collectionId), "green", StickerSort.Recent, null, CancellationToken.None);
                Check(noMatch.Stickers.Length == 0, "Collection search escaped into the rest of the library.");
                using var tagQuery = await session.QueryAsync(new(LibraryScope.Tag, tagId), "", StickerSort.Recent, null, CancellationToken.None);
                Check(tagQuery.Stickers.Length == 0, "An empty tag returned untagged stickers.");
                Check(File.Exists(await session.ThumbnailAsync(first.Sticker.Id, CancellationToken.None)), "Adapter did not resolve a real thumbnail.");
                using var cancelled = new CancellationTokenSource();
                cancelled.Cancel();
                try
                {
                    using var result = await session.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, cancelled.Token);
                    throw new InvalidOperationException("An already cancelled query was admitted.");
                }
                catch (OperationCanceledException) { }
            }
            await using (var reopened = await LibrarySession.OpenAsync(root))
            {
                using var page = await reopened.QueryAsync(LibraryFilter.All, "", StickerSort.Recent, null, CancellationToken.None);
                Check(page.Stickers.Length == 2, "Adapter shutdown/reopen lost data or retained the library lock.");
            }
            Console.WriteLine("PASS: real session import, scoped search, catalogs, cancellation, thumbnail and reopen");
        }
        finally
        {
            if (Directory.Exists(root) && string.Equals(Path.GetDirectoryName(root), parent, StringComparison.OrdinalIgnoreCase)
                && Path.GetFileName(root).StartsWith("MemeDock-Windows-session-", StringComparison.Ordinal))
                Directory.Delete(root, recursive: true);
        }
    }

    // A valid 1px RGBA PNG, using the same fixture construction as NativeContracts.
    private static byte[] Png(byte red, byte green, byte blue)
    {
        using var output = new MemoryStream();
        output.Write(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 });
        var header = new byte[13];
        BinaryPrimitives.WriteUInt32BigEndian(header.AsSpan(0, 4), 1);
        BinaryPrimitives.WriteUInt32BigEndian(header.AsSpan(4, 4), 1);
        header[8] = 8;
        header[9] = 6;
        Chunk(output, "IHDR"u8, header);
        using var compressed = new MemoryStream();
        using (var zlib = new ZLibStream(compressed, CompressionLevel.Fastest, leaveOpen: true))
            zlib.Write(new byte[] { 0, red, green, blue, 255 });
        Chunk(output, "IDAT"u8, compressed.ToArray());
        Chunk(output, "IEND"u8, []);
        return output.ToArray();
    }

    private static void Chunk(Stream output, ReadOnlySpan<byte> type, ReadOnlySpan<byte> bytes)
    {
        Span<byte> word = stackalloc byte[4];
        BinaryPrimitives.WriteUInt32BigEndian(word, (uint)bytes.Length);
        output.Write(word);
        output.Write(type);
        output.Write(bytes);
        uint crc = uint.MaxValue;
        foreach (var value in type.ToArray().Concat(bytes.ToArray()))
        {
            crc ^= value;
            for (var bit = 0; bit < 8; bit++) crc = (crc >> 1) ^ (0xedb88320u & (0u - (crc & 1)));
        }
        BinaryPrimitives.WriteUInt32BigEndian(word, ~crc);
        output.Write(word);
    }

    private static async Task PreferencesAsync()
    {
        var parent = Path.GetFullPath(Path.GetTempPath());
        var root = Path.Combine(parent, "MemeDock-Windows-preferences-" + Guid.NewGuid().ToString("N"));
        try
        {
            var preferences = new AppPreferences(root);
            await preferences.LoadAsync();
            Check(preferences.Language == LanguageMode.System, "A fresh install did not follow the system language.");
            await preferences.SaveLanguageAsync(LanguageMode.Chinese);
            var reopened = new AppPreferences(root);
            await reopened.LoadAsync();
            Check(reopened.Language == LanguageMode.Chinese, "Language did not survive a process restart.");
            await Task.WhenAll(preferences.SaveLanguageAsync(LanguageMode.English), preferences.SaveLanguageAsync(LanguageMode.System));
            await reopened.LoadAsync();
            Check(reopened.Language == LanguageMode.System, "Concurrent preference writes lost the final selection.");
            await File.WriteAllTextAsync(Path.Combine(root, "preferences.json"), "broken json");
            var damaged = new AppPreferences(root);
            await damaged.LoadAsync();
            Check(damaged.Language == LanguageMode.System, "Damaged preferences prevented system-language fallback.");
            Console.WriteLine("PASS: preference persistence, serialized writes and damaged-file recovery");
        }
        finally
        {
            if (Directory.Exists(root) && string.Equals(Path.GetDirectoryName(root), Path.TrimEndingDirectorySeparator(parent), StringComparison.OrdinalIgnoreCase)
                && Path.GetFileName(root).StartsWith("MemeDock-Windows-preferences-", StringComparison.Ordinal))
                Directory.Delete(root, recursive: true);
        }
    }
}
