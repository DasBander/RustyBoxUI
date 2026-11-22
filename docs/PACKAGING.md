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

## Large game resources

Place a level archive, video or model under `ui/` and keep its usual relative URL. Starting with 0.6, the packer streams files into RBPAK v2 using 64-bit lengths and offsets. V1 authenticated each file in one operation, even for a short range request. Its 128 MiB file limit, 65,535-entry limit and 8 MiB encrypted index limit bounded those allocations. In v2 the index grows with the number and length of asset names. The runtime holds that lookup table in memory. Available storage, memory for metadata and the target filesystem still set practical limits. Windows x64 verification included a 4,294,983,687-byte file and a separate package with 65,537 input files.

| Field in `rustybox.json` → `resources` | Default | Effect |
| --- | --- | --- |
| `chunkSizeKiB` | `1024` | Plaintext per authenticated block, from 4 through 16384 KiB. Smaller blocks read less data for scattered requests. Larger blocks reduce the number of authentication tags for sequential reads. |
| `partSizeMiB` | Omitted | A positive integer ceiling for each on-disk data part, including the 16-byte tag per block. One full encrypted block must fit. Omit the field to keep everything in the main package. |

For example, set `"resources": { "chunkSizeKiB": 256, "partSizeMiB": 16 }` to use 256 KiB blocks and data parts of at most 16 MiB. A single source file can cross any number of blocks and parts. Small files share blocks. The main `starter-ui.rbpak` holds the encrypted index and footer. In split mode, `starter-ui.rbpak.chunks-<generation>/00000000.rbchunk` and its numbered siblings hold the data. The part ceiling applies to those data files. Always distribute the complete `out/` from the same build. Both portable ZIP and MSIX packaging collect the directory automatically. For a Microsoft Store submission, its size limit applies to the complete MSIX package. [Microsoft Store package requirements](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/package-version-numbering#package-format-requirements)

A web game can fetch a region of a level with a standard HTTP byte range. The end byte is inclusive.

```js
const response = await fetch("./levels/world.bin", {
  headers: { Range: "bytes=1048576-1114111" }
});
if (response.status !== 206) throw new Error("Could not read level region");
const region = new Uint8Array(await response.arrayBuffer());
if (region.byteLength !== 65_536) throw new Error("Incomplete level region");
```

A full GET also streams. Consume `response.body` to process a large resource incrementally. Calling `arrayBuffer()` on the full response collects the entire file in JavaScript memory. The packaged origin supports one closed, open-ended or suffix range per request, reports the size on HEAD and returns 416 for an unsatisfiable range. Media elements can issue their own range requests. The runtime authenticates a block before returning bytes from it. An unreadable first block returns HTTP 500. If a later block fails after the headers have been sent, WebView2 can deliver a short response. Check the received byte count when the game needs a complete record.

For Rust-only data, use `ui/_native/`. From a generated project, run `cargo add --path vendor/assets` to use the following Rust API. `Assets::reader` implements `Read` and `Seek` in development and release builds. `Assets::read` is a convenience for loading an entire small file.

```rust
use rustybox_assets::Assets;
use std::io::{Read, Seek, SeekFrom};

fn level_header() -> Result<[u8; 32], Box<dyn std::error::Error>> {
    let web_root = std::env::current_exe()?.with_file_name("starter-ui");
    let assets = Assets::open(&web_root)?;
    let mut level = assets.reader("_native/world.bin")?.ok_or("Missing world")?;
    level.seek(SeekFrom::Start(1_048_576))?;
    let mut header = [0; 32];
    level.read_exact(&mut header)?;
    Ok(header)
}
```

The 0.6 runtime also opens packages made with 0.5 and earlier v1 tooling. For those packages, a read authenticates the entire entry. To use v2 in an existing app, update the CLI, DLLs and generated build script together, then rebuild the executable and resources. Older DLLs require v1 packages. Each cook uses a fresh key and, for split output, a new part directory. Custom deployment tools should replace the matching executable, index and parts as one build. Retain older part directories as long as a running client or rollback release needs them. Independent DLC installation and cross-release delta patching require a separate delivery mechanism.

## What the asset package protects

The packer generates a fresh 256-bit content key for every build. AES-GCM authenticates the payload blocks, the encrypted filename index and the footer that locates them. The host carries the content key and adjacent package name in a Windows executable resource. WebView2 receives plaintext on demand at the app’s own HTTPS origin. Rust-only files under `_native/` are excluded from that origin.

Opening the portable ZIP shows the encrypted package. A person who controls a running client can inspect its web content or recover the embedded key. Keep signing keys, service credentials and proprietary logic that must stay confidential on a server. Package encryption covers casual extraction and detects corruption. Publisher identity comes from a trusted signature on the final distribution.

## Package the framework examples

From the framework checkout, run `.\scripts\package.ps1`. A previously built checkout can use `-SkipBuild`. The **apps ZIP** contains eleven sealed applications and their required runtime files. The **SDK ZIP** also contains editable examples, headers, tooling and documentation for developers. The two launcher hosts share one storefront package. Asset credits and license notices remain readable in both distributions.
