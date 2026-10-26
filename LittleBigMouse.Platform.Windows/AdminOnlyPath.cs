#nullable enable
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.Versioning;
using System.Security.AccessControl;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Whether a program may be started elevated from where it lies: only SYSTEM,
/// Administrators and TrustedInstaller can replace it, or anything loaded from its
/// folder.
/// <para>
/// An elevated logon task (<see cref="AutostartExtensions"/>) runs its program with full
/// rights and no UAC prompt. If that program sits in a folder the user can write to (a
/// folder chosen at the root of a drive inherits "Authenticated Users: Modify", a portable
/// copy runs from Downloads), any program running as the user, unelevated, can swap the
/// exe or drop a DLL beside it and be started as administrator at the next logon.
/// </para>
/// <para>
/// The check reads the security descriptors: the exe's folder and everything in it, to
/// the last file of the last subfolder, must grant no write, delete or permission change
/// to anyone else, and no folder above may let anyone else rename or replace what it
/// holds. No junction or symbolic link may stand anywhere on the way. Anything it cannot
/// read or prove counts as writable; deny entries are ignored, which can only make it
/// stricter.
/// </para>
/// </summary>
public static class AdminOnlyPath
{
    /// <summary>The kind of an access control entry, as far as this check cares.</summary>
    public enum AceKind { Allow, Deny, Other }

    /// <summary>One access control entry: who, which rights, whether it applies to the object itself.</summary>
    public readonly record struct Ace(string Sid, int Mask, AceKind Kind, bool InheritOnly);

    /// <summary>A security descriptor's owner and DACL; a null DACL grants everything to everyone.</summary>
    public readonly record struct Descriptor(string? Owner, IReadOnlyList<Ace>? Dacl);

    // Access mask bits (winnt.h); the FileSystemRights enum is Windows-only, this check is not.
    const int WriteData = 0x2;            // FILE_WRITE_DATA / FILE_ADD_FILE
    const int AppendData = 0x4;           // FILE_APPEND_DATA / FILE_ADD_SUBDIRECTORY
    const int DeleteChild = 0x40;         // FILE_DELETE_CHILD
    const int Delete = 0x10000;           // DELETE
    const int WriteDac = 0x40000;         // WRITE_DAC
    const int WriteOwner = 0x80000;       // WRITE_OWNER
    const int GenericAll = 0x10000000;    // GENERIC_ALL
    const int GenericWrite = 0x40000000;  // GENERIC_WRITE

    /// <summary>Rights on the exe or its folder that let someone replace what runs.</summary>
    public const int ReplaceRights =
        WriteData | AppendData | DeleteChild | Delete | WriteDac | WriteOwner | GenericAll | GenericWrite;

    /// <summary>
    /// Rights on a folder above that let someone rename the folder below and put another
    /// in its place. Creating entries is not one of them: the root of a drive lets every
    /// user create folders, which replaces nothing.
    /// </summary>
    public const int RenameRights = DeleteChild | Delete | WriteDac | WriteOwner | GenericAll;

    static readonly HashSet<string> Trusted = new(StringComparer.OrdinalIgnoreCase)
    {
        "S-1-5-18",     // SYSTEM
        "S-1-5-32-544", // BUILTIN\Administrators
        "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464", // NT SERVICE\TrustedInstaller
    };

    /// <summary>
    /// True when a trusted account owns the object and only trusted accounts hold any of
    /// <paramref name="rights"/> on it. An owner can always rewrite the DACL, so an untrusted
    /// owner fails on its own.
    /// </summary>
    public static bool OnlyTrustedHold(Descriptor descriptor, int rights)
    {
        if (descriptor.Owner is null || !Trusted.Contains(descriptor.Owner)) return false;
        if (descriptor.Dacl is null) return false;

        foreach (var ace in descriptor.Dacl)
        {
            if (ace.InheritOnly || (ace.Mask & rights) == 0) continue;
            switch (ace.Kind)
            {
                case AceKind.Deny:
                    continue;
                case AceKind.Allow when Trusted.Contains(ace.Sid):
                    continue;
                default:
                    return false;
            }
        }
        return true;
    }

    /// <summary>
    /// True when <paramref name="exe"/>, everything in its folder and every folder above are
    /// out of reach of anyone but the trusted accounts. False on any doubt.
    /// </summary>
    [SupportedOSPlatform("windows")]
    public static bool IsAdminOnly(string exe) => IsAdminOnly(exe, ReadSecurity);

    /// <summary>
    /// <see cref="IsAdminOnly(string)"/> over any source of descriptors: the walk and the
    /// link refusal are the file system's, the descriptors are <paramref name="read"/>'s.
    /// </summary>
    public static bool IsAdminOnly(string exe, Func<FileSystemInfo, Descriptor> read)
    {
        try
        {
            var file = new FileInfo(exe);
            if (!file.Exists || file.Directory is not { } folder) return false;

            // A junction or a symbolic link anywhere on the way hides where the program
            // really lies: the folders checked below would not be the ones that hold it.
            bool Holds(FileSystemInfo entry, int rights) =>
                (entry.Attributes & FileAttributes.ReparsePoint) == 0 && OnlyTrustedHold(read(entry), rights);

            // The whole application, not only the exe: it loads its assemblies, runtimes\…
            // and plugins from its folder, and Windows checks each file against its own
            // descriptor, so the folder's says nothing about the files it holds.
            if (!Holds(folder, ReplaceRights)) return false;
            var everything = new EnumerationOptions
            {
                RecurseSubdirectories = true,
                AttributesToSkip = 0,
                IgnoreInaccessible = false,
            };
            foreach (var entry in folder.EnumerateFileSystemInfos("*", everything))
                if (!Holds(entry, ReplaceRights)) return false;

            for (var above = folder.Parent; above is not null; above = above.Parent)
                if (!Holds(above, RenameRights)) return false;

            return true;
        }
        catch (Exception)
        {
            // Unreadable descriptor, vanished file, network path that does not answer:
            // nothing proven, nothing elevated.
            return false;
        }
    }

    [SupportedOSPlatform("windows")]
    static Descriptor ReadSecurity(FileSystemInfo entry)
    {
        const AccessControlSections Sections = AccessControlSections.Owner | AccessControlSections.Access;
        return Read(entry switch
        {
            FileInfo f => f.GetAccessControl(Sections),
            DirectoryInfo d => d.GetAccessControl(Sections),
            _ => throw new ArgumentException($"Neither a file nor a folder: {entry.FullName}"),
        });
    }

    [SupportedOSPlatform("windows")]
    static Descriptor Read(ObjectSecurity security) => Read(security.GetSecurityDescriptorBinaryForm());

    /// <summary>A self-relative security descriptor, as <see cref="OnlyTrustedHold"/> reads it.</summary>
    [SupportedOSPlatform("windows")]
    public static Descriptor Read(byte[] binary)
    {
        var sd = new RawSecurityDescriptor(binary, 0);
        var present = (sd.ControlFlags & ControlFlags.DiscretionaryAclPresent) != 0;
        var dacl = present && sd.DiscretionaryAcl is { } acl
            ? acl.Cast<GenericAce>().Select(ToAce).ToList()
            : null;
        return new Descriptor(sd.Owner?.Value, dacl);
    }

    [SupportedOSPlatform("windows")]
    static Ace ToAce(GenericAce ace)
    {
        var inheritOnly = (ace.AceFlags & AceFlags.InheritOnly) != 0;
        return ace switch
        {
            CommonAce { AceQualifier: AceQualifier.AccessAllowed } c =>
                new Ace(c.SecurityIdentifier.Value, c.AccessMask, AceKind.Allow, inheritOnly),
            CommonAce { AceQualifier: AceQualifier.AccessDenied } c =>
                new Ace(c.SecurityIdentifier.Value, c.AccessMask, AceKind.Deny, inheritOnly),
            // Object entries are an Active Directory affair; on a file, read as granting
            // what their mask says, to anyone.
            QualifiedAce { AceQualifier: AceQualifier.AccessDenied } q =>
                new Ace(q.SecurityIdentifier.Value, q.AccessMask, AceKind.Deny, inheritOnly),
            KnownAce k => new Ace(k.SecurityIdentifier.Value, k.AccessMask, AceKind.Other, inheritOnly),
            // An entry this code cannot read could grant anything.
            _ => new Ace("", -1, AceKind.Other, inheritOnly),
        };
    }
}
