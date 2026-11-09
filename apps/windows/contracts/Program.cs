using System.Buffers.Binary;
using System.IO.Compression;
using MemeDock.Bridge;

internal static class Program
{
    private static readonly TimeSpan Timeout = TimeSpan.FromSeconds(20);

    private static async Task<int> Main()
    {
        var root = Path.Combine(Path.GetTempPath(), "MemeDock-Windows-contracts-" + Guid.NewGuid().ToString("N"));
        LibraryHandle? library = null;
        try
        {
            var config = new LibraryConfiguration(Path.Combine(root, "data"), Path.Combine(root, "cache"),
                Path.Combine(root, "share"), MemeDockNative.DefaultResourceConfiguration());
            library = await MemeDockNative.OpenLibrary(config).WaitAsync(Timeout);
            var identity = library.Identity();
            await ExpectError(ErrorCode.AlreadyOpen, async () =>
            {
                using var duplicate = await MemeDockNative.OpenLibrary(config).WaitAsync(Timeout);
            });
            Console.WriteLine("PASS: production library ownership and typed errors");

            using (var notifications = library.Subscribe())
            {
                var pending = notifications.Next();
                Check(!pending.IsCompleted, "Empty notification stream did not suspend.");
                notifications.Unsubscribe();
                Check(await pending.WaitAsync(Timeout) is Notification.Closed, "Unsubscribe did not wake the reader.");
            }
            Console.WriteLine("PASS: production subscription waits and unsubscribes");

            var red = Png(255, 0, 0);
            var first = await Import(library, red, "中文🙂.png");
            Check(first.Status == ImportStatus.Created, "Image import did not create a sticker.");
            var repeated = await Import(library, red, "重复.png");
            Check(repeated.Status == ImportStatus.Reused && repeated.Sticker.Id == first.Sticker.Id, "Import deduplication changed identity.");
            await Import(library, Png(0, 255, 0), "绿色.png");
            using (var query = library.ListStickers(Query()))
            using (var page = await query.AwaitResult().WaitAsync(Timeout))
            {
                Check(page.Stickers.Length == 1 && page.Next is not null, "Keyset pagination did not produce a cursor.");
                using var nextQuery = library.ListStickers(Query(page.Next));
                using var nextPage = await nextQuery.AwaitResult().WaitAsync(Timeout);
                Check(nextPage.Stickers.Length == 1 && page.Stickers[0].Id != nextPage.Stickers[0].Id, "Pagination repeated or lost a sticker.");
            }
            using (var thumbnail = library.RequestThumbnail(first.Sticker.Id, Priority.Visible))
            {
                var output = await thumbnail.AwaitResult().WaitAsync(Timeout);
                Check(File.Exists(output.Path), "Native thumbnail did not publish a file.");
            }
            Console.WriteLine("PASS: real PNG import, deduplication, pagination and thumbnail");

            using (var create = library.CreateCollection("contract trash", null))
            {
                var collection = await create.AwaitResult().WaitAsync(Timeout);
                using var delete = library.DeleteCollection(new EntityReference(collection.Id, collection.Lifecycle.Generation));
                var deletedCollection = await delete.AwaitResult().WaitAsync(Timeout);
                using (var deleted = library.Collections(true))
                {
                    var values = await deleted.AwaitResult().WaitAsync(Timeout);
                    Check(values.Any(value => value.Id == collection.Id), "Deleted collection was not queryable.");
                }
                using var restore = library.RestoreCollection(
                    new EntityReference(deletedCollection.Id, deletedCollection.Lifecycle.Generation),
                    deletedCollection.Lifecycle.Revision);
                var restored = await restore.AwaitResult().WaitAsync(Timeout);
                Check(restored.Lifecycle.Generation > deletedCollection.Lifecycle.Generation,
                    "Restoring a collection did not advance its generation.");
                using var deleteAgain = library.DeleteCollection(new EntityReference(restored.Id, restored.Lifecycle.Generation));
                var deletedAgain = await deleteAgain.AwaitResult().WaitAsync(Timeout);
                using var purge = library.PurgeCollection(
                    new EntityReference(deletedAgain.Id, deletedAgain.Lifecycle.Generation),
                    deletedAgain.Lifecycle.Revision);
                await purge.AwaitResult().WaitAsync(Timeout);
                using var after = library.Collections(true);
                Check(!(await after.AwaitResult().WaitAsync(Timeout)).Any(value => value.Id == collection.Id),
                    "Purged collection remained in the deleted catalog.");
            }

            using (var create = library.CreateTag("contract-tag"))
            {
                var tag = await create.AwaitResult().WaitAsync(Timeout);
                using var delete = library.DeleteTag(new EntityReference(tag.Id, tag.Lifecycle.Generation));
                var deletedTag = await delete.AwaitResult().WaitAsync(Timeout);
                using (var deleted = library.Tags(true))
                {
                    var values = await deleted.AwaitResult().WaitAsync(Timeout);
                    Check(values.Any(value => value.Id == tag.Id), "Deleted tag was not queryable.");
                }
                using var purge = library.PurgeTag(
                    new EntityReference(deletedTag.Id, deletedTag.Lifecycle.Generation),
                    deletedTag.Lifecycle.Revision);
                await purge.AwaitResult().WaitAsync(Timeout);
                using var after = library.Tags(true);
                Check(!(await after.AwaitResult().WaitAsync(Timeout)).Any(value => value.Id == tag.Id),
                    "Purged tag remained in the deleted catalog.");
            }
            Console.WriteLine("PASS: deleted collection/tag query, restore and purge contracts");

            using (var export = library.ExportOriginal(first.Sticker.Id))
            using (var lease = await export.AwaitResult().WaitAsync(Timeout))
            {
                using var handoff = library.PrepareHandoff(lease);
                var artifact = await handoff.AwaitResult().WaitAsync(Timeout);
                Check((await File.ReadAllBytesAsync(artifact.Path)).SequenceEqual(red), "Export changed original bytes.");
                using var protect = library.ProtectClipboard(lease);
                var reference = await protect.AwaitResult().WaitAsync(Timeout);
                using var observe = library.ReconcileClipboard(reference);
                await observe.AwaitResult().WaitAsync(Timeout);
                using var clear = library.ReconcileClipboard(null);
                await clear.AwaitResult().WaitAsync(Timeout);
            }
            Console.WriteLine("PASS: verified export, handoff and clipboard protection contracts");

            // Collection export crosses the same task seam: real archives with the
            // original bytes, typed empty/deleted/missing outcomes, and replacement
            // of the chosen destination only after the new archive is complete.
            string exportCollectionId;
            using (var create = library.CreateCollection("contract export", null))
                exportCollectionId = (await create.AwaitResult().WaitAsync(Timeout)).Id;
            var blue = Png(0, 0, 255);
            using (var createInput = library.CreateImportInput())
            using (var input = await createInput.AwaitResult().WaitAsync(Timeout))
            {
                await File.WriteAllBytesAsync(input.Path(), blue);
                using var request = library.ImportStaged(input, new ImportOptions("蓝色.png", null, exportCollectionId));
                await request.AwaitResult().WaitAsync(Timeout);
            }
            var singleArchive = Path.Combine(root, "contract-single.zip");
            using (var export = library.ExportCollection(exportCollectionId, singleArchive, 480))
            {
                var summary = await export.AwaitResult().WaitAsync(Timeout);
                Check(summary.Exported == 1 && summary.Skipped == 0 && summary.Collections == 1,
                    "Single-collection export summary did not match the library.");
                Check(summary.Path == singleArchive && summary.FileName == "contract-single.zip"
                      && summary.ByteSize == (ulong)new FileInfo(singleArchive).Length,
                    "Export summary did not describe the written file.");
                using var zip = ZipFile.OpenRead(singleArchive);
                var entry = zip.Entries.Single();
                Check(entry.FullName == "蓝色.png", $"Single-collection entry lost its name: {entry.FullName}");
                using var stream = entry.Open();
                using var buffer = new MemoryStream();
                await stream.CopyToAsync(buffer);
                Check(buffer.ToArray().SequenceEqual(blue), "Collection export changed original bytes.");
            }
            var allArchive = Path.Combine(root, "contract-all.zip");
            using (var export = library.ExportCollection(null, allArchive, 480))
            {
                var summary = await export.AwaitResult().WaitAsync(Timeout);
                Check(summary.Exported == 1 && summary.Collections == 1,
                    "Export-all counted unorganized stickers.");
                using var zip = ZipFile.OpenRead(allArchive);
                Check(zip.Entries.Single().FullName == "contract export/蓝色.png",
                    "Export-all did not write one folder per collection.");
            }
            using (var export = library.ExportCollection(exportCollectionId, singleArchive, 480))
                await export.AwaitResult().WaitAsync(Timeout);
            Check(File.Exists(singleArchive), "Replacing the destination lost the archive.");
            CollectionMetadata emptyCollection;
            using (var create = library.CreateCollection("contract empty", null))
                emptyCollection = await create.AwaitResult().WaitAsync(Timeout);
            await ExpectError(ErrorCode.InvalidInput, async () =>
            {
                using var export = library.ExportCollection(emptyCollection.Id, Path.Combine(root, "empty.zip"), 480);
                await export.AwaitResult().WaitAsync(Timeout);
            });
            Check(!File.Exists(Path.Combine(root, "empty.zip")), "An empty export created a file.");
            using (var delete = library.DeleteCollection(new EntityReference(emptyCollection.Id, emptyCollection.Lifecycle.Generation)))
            {
                var deleted = await delete.AwaitResult().WaitAsync(Timeout);
                await ExpectError(ErrorCode.EntityDeleted, async () =>
                {
                    using var export = library.ExportCollection(deleted.Id, Path.Combine(root, "deleted.zip"), 480);
                    await export.AwaitResult().WaitAsync(Timeout);
                });
            }
            await ExpectError(ErrorCode.NotFound, async () =>
            {
                using var export = library.ExportCollection(UnusedCollectionId(), Path.Combine(root, "missing.zip"), 480);
                await export.AwaitResult().WaitAsync(Timeout);
            });
            Console.WriteLine("PASS: collection export archives, replacement and typed error contracts");

            await library.Shutdown().WaitAsync(Timeout);
            library.Dispose();
            library = await MemeDockNative.OpenLibrary(config).WaitAsync(Timeout);
            Check(library.Identity() == identity, "Library identity changed after restart.");
            Console.WriteLine("PASS: explicit shutdown and persistent reopen");
            return 0;
        }
        catch (Exception error)
        {
            Console.Error.WriteLine(error);
            return 1;
        }
        finally
        {
            if (library is not null)
            {
                try { await library.Shutdown().WaitAsync(Timeout); }
                finally { library.Dispose(); }
            }
            var expectedParent = Path.TrimEndingDirectorySeparator(Path.GetTempPath());
            if (Directory.Exists(root)
                && string.Equals(Path.GetDirectoryName(root), expectedParent, StringComparison.OrdinalIgnoreCase)
                && Path.GetFileName(root).StartsWith("MemeDock-Windows-contracts-", StringComparison.Ordinal))
                Directory.Delete(root, recursive: true);
        }
    }

    private static StickerQuery Query(QueryCursorHandle? cursor = null) => new(
        MemeDockNative.NewRequestId(), "", null, [], null, false, StickerSort.Recent, 1, cursor);

    private static async Task<ImportOutcome> Import(LibraryHandle library, byte[] bytes, string name)
    {
        using var create = library.CreateImportInput();
        using var input = await create.AwaitResult().WaitAsync(Timeout);
        await File.WriteAllBytesAsync(input.Path(), bytes);
        using var request = library.ImportStaged(input, new ImportOptions(name, null, null));
        return await request.AwaitResult().WaitAsync(Timeout);
    }

    private static async Task ExpectError(ErrorCode code, Func<Task> operation)
    {
        try { await operation(); }
        catch (BridgeException.Failure error) when (error.code == code) { return; }
        throw new InvalidOperationException($"Expected native error {code}.");
    }

    // Collection ids must be UUIDv7 (version 7, RFC4122 variant); Guid.NewGuid()
    // is v4 and would be rejected as InvalidInput before any lookup. Rewrite the
    // version and variant nibbles of a random id to get a well-formed id that was
    // never inserted, so the lookup itself is what reports NotFound.
    private static string UnusedCollectionId()
    {
        var chars = Guid.NewGuid().ToString().ToCharArray();
        chars[14] = '7';
        chars[19] = "89ab"[Random.Shared.Next(4)];
        return new string(chars);
    }

    // Small real PNG fixture generated with BCL compression and PNG CRCs.
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

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
