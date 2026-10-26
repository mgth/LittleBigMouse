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
