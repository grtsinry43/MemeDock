using MemeDock.Bridge;

namespace MemeDock.Windows.Services;

/// <summary>Staged creator video held by the core until discarded.</summary>
public interface ICreatorVideo : IAsyncDisposable
{
    string Path { get; }
    VideoInfo Info { get; }
}

/// <summary>Leased preview frame or generated WebP animation artifact.</summary>
public interface ICreatorArtifact : IDisposable
{
    string Path { get; }
    string FileName { get; }
    string Mime { get; }
    ulong ByteSize { get; }
    ArtifactLeaseHandle? Lease { get; }
}

public sealed class CreatorGeneratedAnimation(
    ICreatorArtifact artifact,
    uint width,
    uint height,
    ulong durationMs,
    double frameRate) : IDisposable
{
    public ICreatorArtifact Artifact { get; } = artifact;
    public uint Width { get; } = width;
    public uint Height { get; } = height;
    public ulong DurationMs { get; } = durationMs;
    public double FrameRate { get; } = frameRate;

    public void Dispose() => Artifact.Dispose();
}

/// <summary>
/// Workspace boundary for video-to-sticker creation. Implemented by <see cref="LibrarySession"/>
/// and faked by unit tests.
/// </summary>
public interface ICreatorWorkspace
{
    Task<ICreatorVideo> OpenVideoAsync(string path, IProgress<string>? stages, CancellationToken cancellation);
    Task<ICreatorArtifact> RenderPreviewFrameAsync(ICreatorVideo video, ulong timeMs, CancellationToken cancellation);
    Task<CreatorGeneratedAnimation> CreateWebpAsync(ICreatorVideo video, VideoEditOptions edit, IProgress<string>? stages, CancellationToken cancellation);
    Task<ImportOutcome> AddToDockAsync(ICreatorArtifact artifact, string originalName, string title, string? collectionId, CancellationToken cancellation);
    Task<ExportArtifact> PrepareHandoffAsync(ICreatorArtifact artifact, CancellationToken cancellation);
}
