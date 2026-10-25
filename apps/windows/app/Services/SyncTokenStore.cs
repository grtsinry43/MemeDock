using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;

namespace MemeDock.Windows.Services;

/// <summary>
/// Platform-owned storage for the device token. The core never persists the token,
/// so each client keeps it out of the library directory and out of preferences.
/// </summary>
public interface ISyncTokenStore
{
    Task<string?> LoadAsync(string libraryId, string origin, CancellationToken cancellation);
    Task SaveAsync(string libraryId, string origin, string token, CancellationToken cancellation);
    Task ClearAsync(string libraryId, string origin, CancellationToken cancellation);
}

/// <summary>A token could not be read, written or cleared by the platform store.</summary>
public sealed class SyncCredentialException(string message, Exception? inner = null) : Exception(message, inner);

/// <summary>
/// Generic credentials in the Windows Credential Manager, scoped per library and origin.
/// Unlike a file this never leaves a plaintext token on disk.
/// </summary>
public sealed class WindowsCredentialTokenStore : ISyncTokenStore
{
    private const uint GenericCredential = 1;
    private const uint PersistLocalMachine = 2;
    private const int ErrorNotFound = 1168;

    public Task<string?> LoadAsync(string libraryId, string origin, CancellationToken cancellation)
        => Task.Run(() => Read(Target(libraryId, origin)), cancellation);

    public Task SaveAsync(string libraryId, string origin, string token, CancellationToken cancellation)
        => Task.Run(() => Write(Target(libraryId, origin), token), cancellation);

    public Task ClearAsync(string libraryId, string origin, CancellationToken cancellation)
        => Task.Run(() => Delete(Target(libraryId, origin)), cancellation);

    private static string Target(string libraryId, string origin)
    {
        var digest = SHA256.HashData(Encoding.UTF8.GetBytes(libraryId + "\n" + origin));
        return "MemeDock.Sync." + Convert.ToHexString(digest.AsSpan(0, 16)).ToLowerInvariant();
    }

    private static string? Read(string target)
    {
        if (!OperatingSystem.IsWindows()) return null;
        if (!CredRead(target, GenericCredential, 0, out var handle))
        {
            var error = Marshal.GetLastWin32Error();
            if (error == ErrorNotFound) return null;
            throw new SyncCredentialException($"CredRead failed with Windows error {error}.");
        }
        try
        {
            var credential = Marshal.PtrToStructure<CredentialW>(handle);
            if (credential.CredentialBlob == IntPtr.Zero || credential.CredentialBlobSize == 0) return null;
            var bytes = new byte[credential.CredentialBlobSize];
            Marshal.Copy(credential.CredentialBlob, bytes, 0, bytes.Length);
            try
            {
                var token = Encoding.Unicode.GetString(bytes).TrimEnd('\0');
                return token.Length == 0 ? null : token;
            }
            finally { CryptographicOperations.ZeroMemory(bytes); }
        }
        finally { CredFree(handle); }
    }

    private static void Write(string target, string token)
    {
        if (!OperatingSystem.IsWindows())
            throw new SyncCredentialException("The Windows Credential Manager is unavailable.");
        var bytes = Encoding.Unicode.GetBytes(token);
        var blob = Marshal.AllocHGlobal(bytes.Length);
        try
        {
            Marshal.Copy(bytes, 0, blob, bytes.Length);
            var credential = new CredentialW
            {
                Type = GenericCredential,
                TargetName = target,
                CredentialBlobSize = (uint)bytes.Length,
                CredentialBlob = blob,
                Persist = PersistLocalMachine,
                UserName = "MemeDock"
            };
            if (!CredWrite(ref credential, 0))
                throw new SyncCredentialException(
                    $"CredWrite failed with Windows error {Marshal.GetLastWin32Error()}.");
        }
        finally
        {
            CryptographicOperations.ZeroMemory(bytes);
            if (bytes.Length > 0) Marshal.Copy(bytes, 0, blob, bytes.Length);
            Marshal.FreeHGlobal(blob);
        }
    }

    private static void Delete(string target)
    {
        if (!OperatingSystem.IsWindows()) return;
        if (!CredDelete(target, GenericCredential, 0))
        {
            var error = Marshal.GetLastWin32Error();
            if (error != ErrorNotFound)
                throw new SyncCredentialException($"CredDelete failed with Windows error {error}.");
        }
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct CredentialW
    {
        public uint Flags;
        public uint Type;
        [MarshalAs(UnmanagedType.LPWStr)] public string TargetName;
        [MarshalAs(UnmanagedType.LPWStr)] public string? Comment;
        public long LastWritten;
        public uint CredentialBlobSize;
        public IntPtr CredentialBlob;
        public uint Persist;
        public uint AttributeCount;
        public IntPtr Attributes;
        [MarshalAs(UnmanagedType.LPWStr)] public string? TargetAlias;
        [MarshalAs(UnmanagedType.LPWStr)] public string? UserName;
    }

    [DllImport("advapi32.dll", EntryPoint = "CredReadW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredRead(string target, uint type, uint flags, out IntPtr credential);

    [DllImport("advapi32.dll", EntryPoint = "CredWriteW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredWrite(ref CredentialW credential, uint flags);

    [DllImport("advapi32.dll", EntryPoint = "CredDeleteW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredDelete(string target, uint type, uint flags);

    [DllImport("advapi32.dll", EntryPoint = "CredFree", SetLastError = true)]
    private static extern void CredFree(IntPtr buffer);
}
