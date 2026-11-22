# Publish and recover Windows updates

Windows App Installer checks a stable feed address for a new signed MSIX. RustyBox prepares the files locally, so the same workflow works with a static web server or a release pipeline. Clients start their first installation by opening the `.appinstaller` file.

## Prepare a release

Keep the same package `identity`, `publisher` and architecture across releases. Set `version` to a new four-part package version. Add an `updates` object to the existing `rustybox.json`.

```json
{
  "updates": {
    "uri": "https://downloads.example.com/studio/studio.appinstaller",
    "hoursBetweenUpdateChecks": 12
  }
}
```

Choose an HTTPS URI with a simple filename ending in `.appinstaller`. A loopback HTTP address is supported for local development. The interval defaults to 12 hours and accepts 0 through 255. Zero requests a check at every launch.

From the project directory, use the thumbprint of a code-signing certificate with an accessible private key in `CurrentUser\My`. Its subject must match the manifest publisher. Windows on the build machine and the client must trust the package signer.

```powershell
$env:RUSTYBOX_SIGN_THUMBPRINT = 'YOUR_CERTIFICATE_THUMBPRINT'
cargo rustybox package
cargo rustybox publish
```

`publish` consumes the package for the configured version in `dist/`. It builds a missing package, signs an unsigned input and verifies the MSIX with Windows SignTool before advancing the feed. Set `RUSTYBOX_TIMESTAMP_URL` to your RFC 3161 timestamp service when signing production packages. Retained releases are immutable. Repeating a publish with the same signed bytes is safe. If you change an already released build, increase the package version.

## Host the feed

Copy `dist/updates/packages/` and `dist/updates/signatures/` to the server first. Upload the `.appinstaller` descriptor last. Preserve relative paths and use a short cache lifetime or revalidation for the descriptor. Serve `.appinstaller` as `application/appinstaller` and `.msix` as `application/msix`. Provide the HTTPS descriptor URL to users. Windows uses the launch check and an automatic background task according to its update policy. [App Installer file format](https://learn.microsoft.com/en-us/windows/msix/app-installer/how-to-create-appinstaller-file) · [Windows update settings](https://learn.microsoft.com/en-us/windows/msix/app-installer/update-settings)

Windows authenticates the MSIX package. RustyBox also signs the exact descriptor bytes with detached CMS/SHA-256. The file `signatures/<sha256-of-descriptor>.p7s` lets release tooling audit those bytes. Windows does its package verification independently. Keep the complete release directory between publishes, including its signatures. The CLI pins the existing descriptor to the configured release certificate. Plan an explicitly verified feed migration when changing that key.

## Roll back a release

Select a version already present under `dist/updates/packages/`. For example, if the current release is `2.1.0.0`, this command republishes `2.0.0.0`.

```powershell
cargo rustybox rollback 2.0.0.0
```

The feed revision moves forward and the descriptor temporarily allows Windows to install the selected lower package version. Upload the new signature and then the descriptor at the same URL. A normal forward publish clears the downgrade option. On an affected device, open the current descriptor to run the check immediately. A package rollback preserves the app’s profile, so keep persisted data compatible with your supported recovery version. [Windows downgrade setting](https://learn.microsoft.com/en-us/windows/msix/app-installer/update-settings)

### An upgrade and rollback on Windows

The 0.5.0 starter was verified with a private signing certificate and a loopback App Installer feed. Windows reported the newer package as available. Applying the descriptor at each step produced these installed versions.

| Operation | Feed revision | Installed package |
| --- | --- | --- |
| First installation | `0.4.1.0` | `0.4.1.0` |
| Publish the upgrade | `0.5.0.0` | `0.5.0.0` |
| Run `cargo rustybox rollback 0.4.1.0` | `0.5.0.1` | `0.4.1.0` |
| Publish `0.5.0.0` again | `0.5.0.2` | `0.5.0.0` |
