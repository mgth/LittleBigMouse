using System.Diagnostics;
using System.Security.AccessControl;
using LittleBigMouse.Platform.Windows;
using Xunit;
using static LittleBigMouse.Platform.Windows.AdminOnlyPath;

#pragma warning disable CA1416

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The guard that keeps an elevated logon task off programs anyone can replace. The
/// descriptors below are the shapes Windows really hands out: Program Files, a folder
/// created at the root of a drive (which inherits "Authenticated Users: Modify"), the
/// root itself.
/// </summary>
public class AdminOnlyPathTests
{
    const string LocalSystem = "S-1-5-18";
    const string Administrators = "S-1-5-32-544";
    const string TrustedInstaller = "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464";
    const string Users = "S-1-5-32-545";
    const string AuthenticatedUsers = "S-1-5-11";
    const string CreatorOwner = "S-1-3-0";
    const string AllApplicationPackages = "S-1-15-2-1";
    const string SomeUser = "S-1-5-21-1004336348-1177238915-682003330-1001";

    const int FullControl = 0x1F01FF;
    const int ReadAndExecute = 0x1200A9;
    const int Modify = 0x1301BF;
    const int GenericAll = 0x10000000;

    static Ace Allow(string sid, int mask, bool inheritOnly = false) => new(sid, mask, AceKind.Allow, inheritOnly);

    static readonly Ace[] ProgramFiles =
    [
        Allow(TrustedInstaller, FullControl),
        Allow(LocalSystem, FullControl),
        Allow(Administrators, FullControl),
        Allow(Users, ReadAndExecute),
        Allow(CreatorOwner, GenericAll, inheritOnly: true),
        Allow(AllApplicationPackages, ReadAndExecute),
    ];

    [Fact]
    public void ProgramFilesIsAdminOnly()
    {
        Assert.True(OnlyTrustedHold(new Descriptor(TrustedInstaller, ProgramFiles), ReplaceRights));
        Assert.True(OnlyTrustedHold(new Descriptor(Administrators, ProgramFiles), ReplaceRights));
    }

    [Fact]
    public void FolderAtTheRootOfADriveIsWritable()
    {
        Ace[] inherited = [.. ProgramFiles, Allow(AuthenticatedUsers, Modify)];
        Assert.False(OnlyTrustedHold(new Descriptor(Administrators, inherited), ReplaceRights));
        Assert.False(OnlyTrustedHold(new Descriptor(Administrators, inherited), RenameRights));
    }

    [Fact]
    public void TheRootLetsUsersCreateFoldersButNotReplaceThem()
    {
        Ace[] root =
        [
            Allow(Administrators, FullControl),
            Allow(LocalSystem, FullControl),
            Allow(Users, ReadAndExecute),
            Allow(AuthenticatedUsers, Modify, inheritOnly: true),
            Allow(AuthenticatedUsers, 0x4), // FILE_ADD_SUBDIRECTORY: create folders, this folder only
        ];
        var descriptor = new Descriptor(TrustedInstaller, root);

        Assert.True(OnlyTrustedHold(descriptor, RenameRights));
        Assert.False(OnlyTrustedHold(descriptor, ReplaceRights));
    }

    [Fact]
    public void AnUntrustedOwnerCanRewriteAnyDacl()
    {
        Assert.False(OnlyTrustedHold(new Descriptor(SomeUser, ProgramFiles), ReplaceRights));
        Assert.False(OnlyTrustedHold(new Descriptor(null, ProgramFiles), ReplaceRights));
    }

    [Fact]
    public void ANullDaclGrantsEverything()
    {
        Assert.False(OnlyTrustedHold(new Descriptor(Administrators, null), ReplaceRights));
        Assert.True(OnlyTrustedHold(new Descriptor(Administrators, []), ReplaceRights));
    }

    [Fact]
    public void TheUsersOwnAccountIsNotTrusted()
    {
        // What a profile folder looks like: Downloads, %TEMP%, a portable copy.
        Ace[] profile = [Allow(LocalSystem, FullControl), Allow(Administrators, FullControl), Allow(SomeUser, FullControl)];
        Assert.False(OnlyTrustedHold(new Descriptor(LocalSystem, profile), ReplaceRights));
    }

    [Fact]
    public void DenyEntriesAreIgnoredAndUnreadableOnesRefuse()
    {
        Ace[] denied = [.. ProgramFiles, new(AuthenticatedUsers, FullControl, AceKind.Deny, false)];
        Assert.True(OnlyTrustedHold(new Descriptor(Administrators, denied), ReplaceRights));

        Ace[] unreadable = [.. ProgramFiles, new("", -1, AceKind.Other, false)];
        Assert.False(OnlyTrustedHold(new Descriptor(Administrators, unreadable), ReplaceRights));
    }

    [Fact]
    public void ReadingRightsAloneIsHarmless()
    {
        Ace[] readable = [.. ProgramFiles, Allow(AuthenticatedUsers, ReadAndExecute)];
        Assert.True(OnlyTrustedHold(new Descriptor(Administrators, readable), ReplaceRights));
    }

    // ---- The walk: a real tree, descriptors from a stand-in ----

    /// <summary>
    /// An application tree in the temp folder, read through descriptors chosen per path:
    /// Program Files' everywhere, a profile folder's for the paths marked writable. The
    /// temp folder itself is not admin-only, so the stand-in is what makes a positive
    /// case possible at all.
    /// </summary>
    sealed class Install : IDisposable
    {
        public readonly string Root = Path.Combine(Path.GetTempPath(), "lbm-admin-only-" + Guid.NewGuid().ToString("N"));
        public readonly HashSet<string> Writable = new(StringComparer.OrdinalIgnoreCase);
        readonly List<string> _links = [];

        public Install()
        {
            Directory.CreateDirectory(Path.Combine(Root, "app", "runtimes", "win-x64", "native"));
            File.WriteAllText(Path.Combine(Root, "app", "LittleBigMouse.Ui.Avalonia.exe"), "");
            File.WriteAllText(Path.Combine(Root, "app", "LittleBigMouse.Ui.Avalonia.dll"), "");
            File.WriteAllText(Path.Combine(Root, "app", "runtimes", "win-x64", "native", "av_libglesv2.dll"), "");
        }

        public string Exe(string folder = "app") => Path.Combine(Root, folder, "LittleBigMouse.Ui.Avalonia.exe");

        public string At(params string[] parts) => Path.Combine([Root, .. parts]);

        public Descriptor Read(FileSystemInfo entry) =>
            Writable.Contains(Path.TrimEndingDirectorySeparator(entry.FullName))
                ? new Descriptor(LocalSystem, [Allow(LocalSystem, FullControl), Allow(Administrators, FullControl), Allow(SomeUser, FullControl)])
                : new Descriptor(Administrators, ProgramFiles);

        /// <summary>A junction under Windows (no privilege needed), a symbolic link elsewhere.</summary>
        public void Link(string link, string target)
        {
            if (OperatingSystem.IsWindows())
            {
                using var mklink = Process.Start(new ProcessStartInfo("cmd.exe", $"/c mklink /J \"{link}\" \"{target}\"")
                {
                    CreateNoWindow = true,
                    UseShellExecute = false,
                    RedirectStandardOutput = true,
                })!;
                mklink.WaitForExit();
                Assert.Equal(0, mklink.ExitCode);
            }
            else
            {
                Directory.CreateSymbolicLink(link, target);
            }
            _links.Add(link);
        }

        public void Dispose()
        {
            foreach (var link in _links) Directory.Delete(link);
            Directory.Delete(Root, recursive: true);
        }
    }

    [Fact]
    public void AProtectedApplicationIsAdminOnly()
    {
        using var install = new Install();
        Assert.True(IsAdminOnly(install.Exe(), install.Read));
    }

    [Fact]
    public void AWritableAssemblyBesideAProtectedExeIsNotAdminOnly()
    {
        using var install = new Install();
        install.Writable.Add(install.At("app", "LittleBigMouse.Ui.Avalonia.dll"));
        Assert.False(IsAdminOnly(install.Exe(), install.Read));
    }

    [Fact]
    public void AWritableFileOrFolderDeepInTheApplicationIsNotAdminOnly()
    {
        using var install = new Install();
        install.Writable.Add(install.At("app", "runtimes", "win-x64", "native", "av_libglesv2.dll"));
        Assert.False(IsAdminOnly(install.Exe(), install.Read));

        install.Writable.Clear();
        install.Writable.Add(install.At("app", "runtimes", "win-x64"));
        Assert.False(IsAdminOnly(install.Exe(), install.Read));
    }

    [Fact]
    public void AReplaceableFolderAboveIsNotAdminOnly()
    {
        using var install = new Install();
        install.Writable.Add(install.Root);
        Assert.False(IsAdminOnly(install.Exe(), install.Read));
    }

    [Fact]
    public void AJunctionAboveTheApplicationIsNotAdminOnly()
    {
        // The reviewer's case: a protected "Program Files\LittleBigMouse" junction into a
        // protected target whose own parent is replaceable. Every descriptor on the path as
        // written is protected; only the link gives the target away.
        using var install = new Install();
        Directory.CreateDirectory(install.At("programs"));
        install.Link(install.At("programs", "app"), install.At("app"));

        Assert.True(IsAdminOnly(install.Exe(), install.Read));
        Assert.False(IsAdminOnly(Path.Combine(install.At("programs", "app"), "LittleBigMouse.Ui.Avalonia.exe"), install.Read));
    }

    [Fact]
    public void ALinkInsideTheApplicationIsNotAdminOnly()
    {
        using var install = new Install();
        Directory.CreateDirectory(install.At("elsewhere"));
        install.Link(install.At("app", "plugins"), install.At("elsewhere"));
        Assert.False(IsAdminOnly(install.Exe(), install.Read));
    }

    [Fact]
    public void AMissingExeIsNotAdminOnly()
    {
        using var install = new Install();
        Assert.False(IsAdminOnly(install.At("app", "missing.exe"), install.Read));
    }

    // ---- Real descriptors (Windows) ----

    static byte[] Binary(string sddl)
    {
        var sd = new RawSecurityDescriptor(sddl);
        var bytes = new byte[sd.BinaryLength];
        sd.GetBinaryForm(bytes, 0);
        return bytes;
    }

    [WindowsFact]
    public void ReadsOwnerEntriesAndInheritOnly()
    {
        var descriptor = Read(Binary("O:BAD:PAI(A;;FA;;;SY)(A;;FA;;;BA)(A;;0x1200a9;;;BU)(A;OICIIO;GA;;;CO)(D;;FA;;;WD)"));

        Assert.Equal(Administrators, descriptor.Owner);
        Assert.NotNull(descriptor.Dacl);
        Assert.Equal(5, descriptor.Dacl!.Count);
        Assert.Equal(new Ace(CreatorOwner, GenericAll, AceKind.Allow, true), descriptor.Dacl[3]);
        Assert.Equal(AceKind.Deny, descriptor.Dacl[4].Kind);
        Assert.True(OnlyTrustedHold(descriptor, ReplaceRights));
    }

    [WindowsFact]
    public void ReadsANullDaclAsNull()
    {
        Assert.Null(Read(Binary("O:BAD:NO_ACCESS_CONTROL")).Dacl);
    }

    [WindowsFact]
    public void AFileInTheUsersTempFolderIsNotAdminOnly()
    {
        var file = Path.GetTempFileName();
        try
        {
            Assert.False(IsAdminOnly(file));
        }
        finally
        {
            File.Delete(file);
        }
    }

    [WindowsFact]
    public void AMissingFileIsNotAdminOnly()
    {
        Assert.False(IsAdminOnly(Path.Combine(Path.GetTempPath(), Guid.NewGuid() + ".exe")));
    }
}
