#nullable enable
using System;
using System.Security.Cryptography;
using System.Text;

namespace LittleBigMouse.Plugins.Persistence;

/// <summary>
/// The name a layout is stored under, derived from its id.
/// <para>
/// A layout id is the "+"-joined list of its monitor ids (<c>LayoutIdExtensions.ComputeId</c>):
/// 26 to 31 characters per monitor on Windows, more on Linux. Both backends cap a name —
/// Windows refuses a registry key name over 255 characters, the common Linux filesystems a
/// file name over 255 bytes — and nine monitors are enough to go over. The registry then
/// threw at the first read and the UI never came up, which read as an "8-monitor limit" (#589).
/// </para>
/// <para>
/// An id that fits is stored as-is, so every existing configuration keeps its key. A longer
/// one keeps a readable head and ends with the SHA-256 of the whole id: two setups sharing
/// their first monitors still get distinct entries, and one setup always maps to the same.
/// </para>
/// </summary>
public static class LayoutStoreKey
{
    /// <summary>Longest name the backends accept, registry key names and file names alike.</summary>
    public const int MaxLength = 255;

    /// <summary>Between the readable head and the digest; never part of an id.</summary>
    const char Separator = '~';

    /// <param name="layoutId">The layout id, as computed from the monitor set.</param>
    /// <param name="maxLength">
    /// The backend's cap. A file store passes the cap minus its extension.
    /// </param>
    public static string For(string layoutId, int maxLength = MaxLength)
    {
        ArgumentNullException.ThrowIfNull(layoutId);
        if (layoutId.Length <= maxLength) return layoutId;

        var digest = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(layoutId)));

        var head = maxLength - digest.Length - 1;
        if (head < 0)
            throw new ArgumentOutOfRangeException(nameof(maxLength), maxLength,
                $"A store key needs room for the {digest.Length}-character digest and its separator.");

        // Ids are ASCII in practice; never cut a surrogate pair should one show up.
        if (head > 0 && char.IsHighSurrogate(layoutId[head - 1])) head--;

        return $"{layoutId[..head]}{Separator}{digest}";
    }
}
