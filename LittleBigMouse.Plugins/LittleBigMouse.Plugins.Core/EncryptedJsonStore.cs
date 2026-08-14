#nullable enable
using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;

namespace LittleBigMouse.Plugins;

/// <summary>A stored setting identified by the monitor it belongs to.</summary>
public interface IMonitorSetting
{
    string MonitorId { get; }
}

/// <summary>
/// Atomic, plugin-owned storage for per-monitor settings that carry a credential — smart-TV
/// pairing tokens today. The whole file is encrypted by <see cref="SecretProtector"/>, and on
/// Unix restricted to its owner as well.
///
/// Two migrations run on the first read and are then invisible: a file still in clear, as
/// 5.6.0 and earlier wrote it, is rewritten encrypted; a file found at
/// <c>legacyFilePath</c> is moved to where <see cref="LbmPaths"/> says it belongs and the old
/// copy deleted.
///
/// A file that cannot be read — lost key, another account's DPAPI blob, corruption — costs
/// the user a re-pairing rather than leaving the feature stuck: the store starts empty and
/// carries on. That is the right trade for a token authorising volume and input on a
/// television, and would not be for anything harder to re-obtain.
/// </summary>
public abstract class EncryptedJsonStore<T> where T : class, IMonitorSetting
{
    static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    readonly object _lock = new();
    readonly string _label;
    readonly string _filePath;
    readonly string? _legacyFilePath;
    readonly SecretProtector _protector;
    Dictionary<string, T>? _items;

    /// <param name="label">Names this store in the messages it writes to standard error.</param>
    /// <param name="legacyFilePath">
    /// Where a previous version of the application left this file. Read once, then removed, if
    /// <paramref name="filePath"/> does not exist yet. Null for a store that should never look
    /// outside the path it was given.
    /// </param>
    protected EncryptedJsonStore(string label, string filePath, string? legacyFilePath = null)
    {
        _label = label;
        _filePath = filePath;
        _legacyFilePath = legacyFilePath;
        _protector = SecretProtector.ForFile(filePath);
    }

    /// <summary>
    /// A private copy. Callers hold on to what they save and to what they read, and these
    /// settings are mutable, so neither may share an instance with the dictionary.
    /// </summary>
    protected abstract T Clone(T item);

    public T? Get(string monitorId)
    {
        lock (_lock)
        {
            EnsureLoaded();
            return _items!.TryGetValue(monitorId, out var item) ? Clone(item) : null;
        }
    }

    public void Save(T item)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(item.MonitorId);

        lock (_lock)
        {
            EnsureLoaded();
            _items![item.MonitorId] = Clone(item);
            SaveLocked();
        }
    }

    void EnsureLoaded()
    {
        if (_items is not null) return;

        var source = _filePath;
        var movedFromLegacy = false;

        if (!File.Exists(source) && _legacyFilePath is not null
            && !SamePath(_legacyFilePath, _filePath) && File.Exists(_legacyFilePath))
        {
            source = _legacyFilePath;
            movedFromLegacy = true;
        }

        var wasInClear = false;
        try
        {
            if (File.Exists(source))
            {
                var content = File.ReadAllText(source);
                wasInClear = !SecretProtector.IsProtected(content);
                if (!wasInClear) content = _protector.Unprotect(content);

                _items = JsonSerializer.Deserialize<Dictionary<string, T>>(content, JsonOptions) ?? [];
            }
            else _items = [];
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"{_label} settings unreadable, starting fresh: {e.Message}");
            _items = [];
            wasInClear = false;
            movedFromLegacy = false;
        }

        if (!wasInClear && !movedFromLegacy) return;

        // Drop the old file only once the encrypted copy is on disk — and only if we actually
        // read it from elsewhere. Leaving it behind would leave the tokens in clear, which is
        // the whole point of this.
        if (!SaveLocked() || !movedFromLegacy) return;

        try
        {
            File.Delete(source);
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"{_label} settings moved to {_filePath} but {source} could not be removed: {e.Message}");
        }
    }

    bool SaveLocked()
    {
        try
        {
            var directory = Path.GetDirectoryName(_filePath);
            if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);

            // The temporary file holds ciphertext, so the window before the move and the
            // chmod below exposes nothing.
            var temporary = _filePath + ".tmp";
            File.WriteAllText(temporary, _protector.Protect(JsonSerializer.Serialize(_items, JsonOptions)));
            File.Move(temporary, _filePath, overwrite: true);

            if (!OperatingSystem.IsWindows())
                File.SetUnixFileMode(_filePath, UnixFileMode.UserRead | UnixFileMode.UserWrite);

            return true;
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"{_label} settings not saved: {e.Message}");
            return false;
        }
    }

    static bool SamePath(string a, string b) => string.Equals(
        Path.GetFullPath(a), Path.GetFullPath(b),
        OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal);
}
