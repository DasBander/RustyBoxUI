# Package a Windows application

A generated project has two build modes. Use `cargo rustybox dev` while editing and `cargo rustybox build` to produce the directory you distribute. Both commands run from the project root. The project also carries a copy of the CLI under `tools/`.

| Output | Contents | Intended use |
| --- | --- | --- |
| `target/live/` | Host, DLLs and editable web files with the reload client | Local development |
| `out/` | Host, runtime DLLs, authenticated `.rbpak` and license notices | Portable application |
| `dist/*.msix` and `dist/*-portable.zip` | Windows installer and portable application archive | Distribution |
| `dist/updates/` | App Installer descriptor, signed versioned MSIX packages and detached descriptor signatures | Hosting a feed prepared by `cargo rustybox publish` |

## Windows branding

New apps include a `branding/app.png` and a Cargo build script that embeds Windows resources. Change the image and edit the project’s `rustybox.json`. Paths are relative to that file.

| Field | Where it appears | If omitted |
| --- | --- | --- |
| `name` | Explorer product name, installer display name | Required |
| `version` | Product version and MSIX package version | Required |
| `publisherDisplayName` | Installer publisher display name and default Explorer company name | Required |
| `publisher` | Installer publisher’s certificate subject | Required for packaging |
| `identity` | Stable MSIX package identity | Required for packaging |
| `fileVersion` | Explorer file version for this build | Uses `version` |
| `executableName` | Filename in `out/`, the installer and development staging, plus Explorer’s original filename | `rustybox-starter.exe` in a generated project |
| `description` | Explorer file description and installer description | Uses `name` |
| `companyName` | Explorer company name | Uses `publisherDisplayName` |
| `copyright`, `trademarks` | Explorer legal notices | Each is optional |
| `icon` | Executable, native window and installer artwork | Uses the built-in RustyBox cube |

Both versions take four decimal numbers from 0 to 65535, such as `2.3.0.17`. Choose a single Windows filename ending in `.exe`. A square transparent PNG at 256 × 256 or larger works well. The builder creates nine icon sizes from 16 to 256 pixels. A supplied Windows ICO keeps its original frames. The installer uses 44, 50 and 150 pixel PNGs derived from the same artwork.

Run `cargo rustybox build` and open **Properties → Details** on the executable in `out/` to check the result. Existing five-field manifests still build with the defaults above. For a project generated with an older SDK, generate a sibling project with the new SDK, move your UI and native commands across, then copy over your manifest and keep your existing identity and profile path. Rust hosts with their own Cargo layout can add `rustybox-build` as a build dependency and call `rustybox_build::embed("your-cargo-bin-name")` from `build.rs`. [Windows version resources](https://learn.microsoft.com/en-us/windows/win32/menurc/versioninfo-resource)

## Create an installer

Set the identity, four-part version, display name and publisher in `rustybox.json`. The Windows SDK supplies MakeAppx and SignTool. Run the following in the generated project.

```powershell
cargo rustybox package
```

For a signed MSIX, set `RUSTYBOX_SIGN_THUMBPRINT` to the thumbprint of your code-signing certificate in the current user's personal certificate store before packaging. The manifest publisher must match the certificate subject. The build binds assets before signing and asks SignTool to use the certificate in the store. A recipient's machine must trust the signer to install the MSIX. The portable ZIP also runs directly after extraction with WebView2 Evergreen installed.

The installer also registers the app’s declared file types and protocols. For updates, prepare a signed feed with `cargo rustybox publish`. [Shell associations](ACTIVATION.md) · [Hosting an update feed](UPDATES.md)

## What the asset package protects

The packer walks the application's web directory, skips development artifacts such as source maps and type declarations, and generates a fresh 256-bit content key. AES-GCM authenticates each encrypted file and the encrypted directory containing names and offsets. The host carries the content key and adjacent package name in a Windows executable resource. Ship the matching executable and package together.

WebView2 requests a page, module or image at the app's ordinary local HTTPS origin. The runtime authenticates the requested file and serves the bytes from memory. Media can request byte ranges. Assets under `_native/` are available to Rust through `rustybox_assets::Assets::read` and are excluded from the packaged web origin. Missing or damaged packages and invalid authentication tags cause loading to fail. App profile data and files that the user explicitly exports are separate from packaged assets.

Opening the portable ZIP shows the encrypted package. A person who controls a running client can inspect its web content or recover the embedded key. Keep signing keys, service credentials and proprietary logic that must stay confidential on a server. Package encryption covers casual extraction and detects corruption. Publisher identity comes from a trusted signature on the final distribution.

## Package the framework examples

From the framework checkout, run `.\scripts\package.ps1`. A previously built checkout can use `-SkipBuild`. The **apps ZIP** contains ten sealed applications and their required runtime files. The **SDK ZIP** also contains editable examples, headers, tooling and documentation for developers. The two launcher hosts share one storefront package. Asset credits and license notices remain readable in both distributions.
