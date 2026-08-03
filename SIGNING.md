# Release signing

Windows releases are Authenticode-signed through the free [SignPath Foundation](https://signpath.org/) program. No private key or certificate lives in this repository or in GitHub Actions: the workflow uploads the build output to SignPath and gets signed files back.

Signing exists because Windows 11 Smart App Control blocks unsigned installers outright, with no way for the user to override it ([#535](https://github.com/mgth/LittleBigMouse/issues/535)). It is not an instant fix — a certificate gives every release one stable publisher identity, and Microsoft's reputation for that identity builds up over subsequent releases.

This file doubles as the project's code signing policy, which the SignPath Foundation program requires published.

## Team roles

| Role | Who | What they may do |
| --- | --- | --- |
| Author | anyone | Open a pull request. Authors cannot merge their own work or trigger a signature. |
| Reviewer | [@mgth](https://github.com/mgth) | Approve and merge pull requests into `master`. |
| Approver | [@mgth](https://github.com/mgth) | Push a `v*` tag and approve the signing request in SignPath. |

Multi-factor authentication is required on the GitHub and SignPath accounts of everyone holding a Reviewer or Approver role.

Only releases built by the workflow in this repository, from this repository's own sources, are ever submitted for signing. Nothing is signed from a local machine.

## Third-party components

Little Big Mouse ships third-party binaries it does not build: the .NET runtime libraries, Avalonia, and the NuGet and crates.io dependencies listed in the project files and in `LittleBigMouse-Hook-Rust/Cargo.lock`. Those are signed as part of the packaged application where the signing policy allows it; upstream projects are encouraged to obtain their own signatures rather than rely on ours. No third-party binary is signed on behalf of its author.

## User data

Little Big Mouse collects no analytics and sends no telemetry. What it stores, and the only cases where it reaches the network, are described in [PRIVACY.md](PRIVACY.md). The application can be removed through Programs & Features, or by the uninstaller in the install directory.

## One-time SignPath setup

1. Apply for Little Big Mouse at SignPath Foundation and connect the `mgth/LittleBigMouse` repository through the SignPath GitHub App.
2. Add an *application* artifact configuration: root is the GitHub artifact ZIP, signing `LittleBigMouse.Ui.Avalonia.exe`, `LittleBigMouse.Hook.exe` and the eligible DLLs it contains.
3. Add an *installer* artifact configuration: root is the GitHub artifact ZIP, signing `LittleBigMouse_*.exe`.
4. Add a release signing policy using the SignPath Foundation certificate.
5. Store the API token as the `SIGNPATH_API_TOKEN` Actions **secret**, and add these Actions **variables**:
   - `SIGNPATH_ORGANIZATION_ID`
   - `SIGNPATH_PROJECT_SLUG`
   - `SIGNPATH_SIGNING_POLICY_SLUG`
   - `SIGNPATH_APPLICATION_ARTIFACT_CONFIGURATION_SLUG`
   - `SIGNPATH_INSTALLER_ARTIFACT_CONFIGURATION_SLUG`

`SIGNPATH_PROJECT_SLUG` is the switch. While it is unset, a `v*` tag drafts the unsigned installer exactly as it did before; once it is set, the signed release job takes over and the unsigned path stands down. Nothing has to be merged or reverted on the day the SignPath application is approved.

## What the tag build does

Signing the setup `.exe` alone would leave the binaries inside it unsigned, and those are the ones Windows judges once they are installed on disk. So the binaries are signed first, then the installer is **recompiled** from the signed files (`ISCC /DSourceDir=...`) and signed in turn.

Both stages fail closed: the workflow asserts that `LittleBigMouse.Ui.Avalonia.exe`, `LittleBigMouse.Hook.exe` and the installer each carry a `Valid` signature whose simple publisher name is exactly `SignPath Foundation`. A signing request that comes back successful without having signed those files stops the release.

The draft release carries the installer and nothing else, deliberately: the in-app updater downloads `assets[0]`, so an extra asset sorting ahead of it would be handed to existing users in place of the setup program.

## Verifying a download

```powershell
Get-AuthenticodeSignature .\LittleBigMouse_5.6.1.exe | Format-List Status, SignerCertificate
```

Free code signing provided by [SignPath.io](https://signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).
