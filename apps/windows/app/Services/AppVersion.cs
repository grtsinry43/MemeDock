using System.Reflection;

namespace MemeDock.Windows.Services;

public static class AppVersion
{
    public const string Unknown = "—";

    /// <summary>
    /// The app is deployed unpackaged, so <c>Package.Current</c> is unavailable. The
    /// version comes from assembly metadata, which the project sets from the single
    /// <c>&lt;Version&gt;</c> property that also feeds debug, CI and release builds.
    /// </summary>
    public static string FromAssembly(Assembly assembly) => Display(
        assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion,
        assembly.GetName().Version);

    /// <summary>
    /// Informational versions may carry a <c>+commit</c> suffix from source revision
    /// stamping; users only need the release version.
    /// </summary>
    public static string Display(string? informationalVersion, Version? assemblyVersion)
    {
        if (!string.IsNullOrWhiteSpace(informationalVersion))
        {
            var metadata = informationalVersion.IndexOf('+');
            var text = (metadata >= 0 ? informationalVersion[..metadata] : informationalVersion).Trim();
            if (text.Length > 0) return text;
        }
        if (assemblyVersion is null) return Unknown;
        return assemblyVersion.Build >= 0 ? assemblyVersion.ToString(3) : assemblyVersion.ToString(2);
    }

    /// <summary>
    /// Reads the workspace version from <c>Cargo.toml</c>. Tests compare it with the
    /// project's <c>&lt;Version&gt;</c> so the app never advertises a stale number.
    /// </summary>
    public static string? WorkspaceVersion(string cargoToml)
    {
        var inWorkspacePackage = false;
        foreach (var line in cargoToml.Split('\n'))
        {
            var trimmed = line.Trim();
            if (trimmed.StartsWith('['))
            {
                inWorkspacePackage = trimmed == "[workspace.package]";
                continue;
            }
            if (!inWorkspacePackage || !trimmed.StartsWith("version", StringComparison.Ordinal)) continue;
            var equals = trimmed.IndexOf('=');
            if (equals < 0) continue;
            return trimmed[(equals + 1)..].Trim().Trim('"');
        }
        return null;
    }
}
