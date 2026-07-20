# Release signing

Windows releases are designed to be signed through the free SignPath Foundation program. No private signing key or certificate is stored in this repository or in GitHub Actions.

## One-time SignPath setup

1. Apply for the Little Big Mouse project at SignPath Foundation and connect the `mgth/LittleBigMouse` repository through the SignPath GitHub App.
2. Configure an application artifact configuration whose root is the GitHub artifact ZIP and which Authenticode-signs the packaged LittleBigMouse executables and eligible DLLs.
3. Configure an installer artifact configuration whose root is the GitHub artifact ZIP and which Authenticode-signs `LittleBigMouse_*.exe`.
4. Configure a release signing policy using the SignPath Foundation certificate.
5. Add the API token as the `SIGNPATH_API_TOKEN` GitHub Actions secret. Add these Actions variables:
   - `SIGNPATH_ORGANIZATION_ID`
   - `SIGNPATH_PROJECT_SLUG`
   - `SIGNPATH_SIGNING_POLICY_SLUG`
   - `SIGNPATH_APPLICATION_ARTIFACT_CONFIGURATION_SLUG`
   - `SIGNPATH_INSTALLER_ARTIFACT_CONFIGURATION_SLUG`

Tag builds deliberately fail closed when signing is unavailable or misconfigured. The workflow verifies that both the UI/hook and installer have a valid Authenticode signature whose simple publisher name is exactly `SignPath Foundation`. Only verified files are packaged, checksummed, attested, and attached to a draft release.

Users can verify a downloaded file with Windows `Get-AuthenticodeSignature` and verify build provenance with:

```powershell
gh attestation verify --repo mgth/LittleBigMouse .\LittleBigMouse_6.0.0.exe
```

Free code signing provided by SignPath.io, certificate by SignPath Foundation.
