# Release signing

Windows releases are Authenticode-signed through the free [SignPath Foundation](https://signpath.org/) program. No private key or certificate lives in this repository or in GitHub Actions: the workflow uploads the build output to SignPath and gets signed files back.

Signing exists because Windows 11 Smart App Control blocks unsigned installers outright, with no way for the user to override it ([#535](https://github.com/mgth/LittleBigMouse/issues/535)). It is not an instant fix — a certificate gives every release one stable publisher identity, and Microsoft's reputation for that identity builds up over subsequent releases.

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
