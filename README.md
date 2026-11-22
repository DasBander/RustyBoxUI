# RustyBoxUI

RustyBoxUI is a Windows x64 framework for desktop apps built with Rust and web UI. One DLL hosts WebView2 and connects the page to native commands. A second DLL lets Rust and C++ hosts put Direct3D 11 textures behind HTML controls. The C# binding embeds the UI in a .NET 10 window.

Release builds stream HTML, scripts and game data from encrypted resources. Ship one `.rbpak` or split large payloads into part files. [Packaging resources](#packaging-resources) · [Talus gameplay screenshots](#talus)

## Windows downloads · 0.8.0

| Download | How to use it |
| --- | --- |
| [Windows x64 SDK](https://github.com/DasBander/RustyBoxUI/releases/download/v0.8.0/RustyBoxUI-0.8.0-windows-x64.zip) | Extract the ZIP, open `RustyBoxUI-0.8.0` and run `rustybox-starter.exe`. The SDK includes the DLLs, CLI, headers, examples and docs. |
| [Splice Room WAV editor](https://github.com/DasBander/RustyBoxUI/releases/download/v0.8.0/Splice-Room-0.8.0-windows-x64.zip) | Extract the ZIP and open `rustybox-audio.exe`. Trim a WAV, reopen the editor to recover the edit, then choose **Export WAV…**. |
| [All eleven Windows examples](https://github.com/DasBander/RustyBoxUI/releases/download/v0.8.0/RustyBoxUI-0.8.0-apps-windows-x64.zip) | Extract the ZIP and launch any example. Open `rustybox-talus.exe` to fly Copper Narrows or install Red Switchback from Settings. |
| [RustyBoxUI NuGet package](https://github.com/DasBander/RustyBoxUI/releases/download/v0.8.0/RustyBoxUI.0.8.0.nupkg) | Save the `.nupkg` in a `packages` folder in your Windows x64 .NET 10 project. From that project, run `dotnet add package RustyBoxUI --version 0.8.0 --source ./packages`. |

The Windows apps use the Microsoft Edge WebView2 Evergreen Runtime. [Get WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) · [0.8.0 release notes](https://github.com/DasBander/RustyBoxUI/releases/tag/v0.8.0)

[![The Last Signal in a 1560 × 618 storefront window, with View game and Wishlist visible beneath the description](docs/images/storefront-short.jpg)](docs/images/storefront-short.jpg)

## What's new in 0.8.0

- **Splice Room** trims WAVs, restores the source, selection and Undo history after reopening, and exports through Windows Save As. Try it with `.\scripts\run.ps1 -Example audio`. [Document editing](docs/DESKTOP_APPS.md#edit-and-recover-a-document)
- **Vite and TypeScript** support hot updates in the native window and bundle the frontend for distribution. Start with `cargo rustybox new MyStudio --template vite-ts`. [Frontend setup](docs/DESKTOP_APPS.md#develop-with-vite-and-typescript)
- **Windows shell integration** gives Relay a tray menu, notifications and taskbar progress. [Desktop actions](docs/DESKTOP_APPS.md#integrate-with-the-windows-shell)
- **Signed content packs** install, resume and update independently. Try the optional Red Switchback course in Talus. [Content packs](docs/DESKTOP_APPS.md#install-optional-game-content)
- **Diagnostics** include `cargo rustybox doctor` and a development inspector for bridge calls. [Diagnostics](docs/DESKTOP_APPS.md#diagnose-an-installation-or-a-bridge-command)
- **C# embedding** ships as a Windows x64 NuGet package. The SDK includes a WinForms example that calls a typed C# command from the page. [Managed host guide](docs/DESKTOP_APPS.md#embed-in-a-c-application)

[![Splice Room after reopening a trimmed 24-bit stereo WAV, with Undo and Export available](docs/images/splice-trim.jpg)](docs/images/splice-trim.jpg)

## Run the starter

From a source checkout, open PowerShell in the repository root. Install Rust stable for Windows MSVC, the Visual Studio C++ build tools with a Windows SDK, and the WebView2 Evergreen Runtime. The developer tools use Node.js 22 or newer. [Prerequisites and installation links](docs/GETTING_STARTED.md#prerequisites)

```powershell
.\scripts\build.ps1
.\scripts\run.ps1 -Starter
```

Enter `Ada` and select **Greet**. The HTML form calls Rust and displays **Hello, Ada.** If you have the extracted Windows SDK, you can launch `rustybox-starter.exe` directly.

## Build your own app

From the built checkout, add the CLI to this PowerShell session, generate a project and start the development host. From the extracted SDK, add the SDK root to your path instead of the checkout’s build directory.

```powershell
$env:Path = "$PWD\target\release;$env:Path"
cargo rustybox new ..\HelloRustyBox
Set-Location ..\HelloRustyBox
cargo rustybox dev
```

Edit the page in `ui/` and implement native commands in `src/main.rs`. The starter includes a typed bridge, a draggable title bar and the files needed to ship the resulting app. [Build your first app](docs/GETTING_STARTED.md) · [Bridge, components, D3D11 and C++ embedding](docs/INTEGRATION.md)

## Packaging resources

Put your app’s resources under `ui/`, with `index.html` at the top level. The packer walks this directory recursively and preserves relative paths. For example, a game library might have these files.

```text
ui/
  index.html
  app.js
  styles.css
  images/cover.webp
  fonts/interface.woff2
  data/catalog.json
```

In `index.html`, use `<img src="./images/cover.webp" alt="Featured game cover">`. In `app.js`, `fetch("./data/catalog.json")` reads the catalog. Both paths work with `cargo rustybox dev` and in the packaged app.

A game can ship a multi-gigabyte level, video or texture archive as one source file. The packer reads it in 1 MiB chunks by default. WebView2 can seek to a byte range and the runtime authenticates only the chunks it reads. The packer skips dot-prefixed entries, `node_modules/`, `*.map` and `*.d.ts`. Files under `ui/_native/` are available to Rust through `Assets::reader` and return HTTP 404 at the packaged web origin.

For separate distribution files, merge these settings into `rustybox.json`. The example keeps each encrypted data part at or below 64 MiB. The game continues using paths such as `./levels/world.bin`.

```json
{
  "resources": { "chunkSizeKiB": 1024, "partSizeMiB": 64 }
}
```

Omit `partSizeMiB` to keep the chunks inside one `.rbpak`. With split output, ship the adjacent `starter-ui.rbpak.chunks-…/` directory along with the main package. The portable ZIP and MSIX include both. [Streaming, byte ranges and format compatibility](docs/PACKAGING.md#large-game-resources)

Run the command for the output you want from the generated project directory.

| Command | What it produces |
| --- | --- |
| `cargo rustybox build` | A runnable `out/` folder containing the executable, `starter-ui.rbpak`, `rustybox_ui.dll`, `rustybox_graphics.dll` and `licenses/`. Ship the complete folder. |
| `cargo rustybox package` | Builds the app and writes `dist/<identity>-<version>-portable.zip` and `dist/<identity>-<version>.msix` with the sealed resources. |

Each release build embeds a fresh AES-256-GCM key in the executable. The packer uses it to encrypt and authenticate the assets and their filename index. Keep the executable, index and any part files from the same build together. This protects against casual extraction from the distribution. Someone who controls a running client can still inspect the content it loads. [Asset protection and installer signing](docs/PACKAGING.md)

## Give your app a Windows identity

Replace `branding/app.png` in a generated project and edit `rustybox.json`. The icon appears on the executable and the native window. Packaging generates matching Start menu artwork. The same manifest controls the product name, executable filename and the fields people see in **Properties → Details**.

```json
{
  "identity": "Northstar.Studio",
  "name": "Northstar Studio",
  "publisher": "CN=Northstar Software",
  "publisherDisplayName": "Northstar Software",
  "description": "A desktop editor for field recordings",
  "version": "2.3.0.0",
  "fileVersion": "2.3.0.17",
  "executableName": "Northstar Studio.exe",
  "icon": "branding/app.png",
  "copyright": "© 2026 Northstar Software"
}
```

Run `cargo rustybox build` to get `out/Northstar Studio.exe` alongside its DLLs and encrypted assets. Run `cargo rustybox package` for the portable ZIP and MSIX. You can also supply a multi-size `.ico`, a separate company name and a trademark notice. [Manifest fields and defaults](docs/PACKAGING.md#windows-branding)

## Examples

### Talus

Fly a hang glider through nine gates along the river in Copper Narrows. The 2.22 km WebGL time trial adds four seconds for each missed gate and saves your personal best. Steer with the arrow keys, hold **Space** to boost and press **F11** for a fullscreen view with the window controls hidden.

To play on Windows, extract the examples ZIP and open `rustybox-talus.exe`. From a built checkout, use `.\scripts\run.ps1 -Example talus`. For the browser edition, run `npm run preview:talus` from the repository root and open the local URL it prints. [Download the examples ZIP with Talus](https://github.com/DasBander/RustyBoxUI/releases/download/v0.8.0/RustyBoxUI-0.8.0-apps-windows-x64.zip) · [Controls and scoring](docs/EXAMPLES.md#fly-copper-narrows) · [Game source](examples/showcase/talus/ui)

[![The glider banks left toward the second gate at 307 km/h in fullscreen](docs/images/talus-bank.jpg)](docs/images/talus-bank.jpg)

| Windowed main menu | Approaching the first gate |
| --- | --- |
| [![Talus main menu with Start flight and the Copper Narrows course details](docs/images/talus-menu.jpg)](docs/images/talus-menu.jpg) | [![A glider lines up with the first timing gate above the river in Copper Narrows](docs/images/talus-flight.jpg)](docs/images/talus-flight.jpg) |

### More examples

| Steam-inspired game library | Foundry material editor |
| --- | --- |
| [![Library home with game collections and offline radio demos](docs/images/steam-home.jpg)](docs/images/steam-home.jpg) | [![Foundry showing a copper sphere in D3D11 beside HTML material controls](docs/images/material.jpg)](docs/images/material.jpg) |

The repository also includes a controller-operated library, a playable race, a patcher that can resume an install, a WAV editor, a satellite texture inspector and a C++ embedding example. The gallery shows how the storefront fits short windows and how Relay arranges its installation controls at 4K. [All eleven examples and launch commands](docs/EXAMPLES.md) · [Window sizes and screenshots](docs/EXAMPLES.md#window-sizes)

Atlas is a folder-based image browser built with the same SDK. The tree and virtualized table show thumbnails while a Rust worker scans. Select a texture to compare channels in two GPU viewports or open a separate preview window.

![Atlas project browser with local images grouped by folder and their pixel dimensions](docs/images/atlas-project.jpg)

## How the framework fits together

The Rust SDK loads the native DLL through a small C ABI. The browser and native event loop share one UI thread, while a worker persists settings off that thread. The app loop shares state across native windows. Each window can host multiple D3D11 viewports with independent textures and display settings. Maximized windows fit the current monitor’s work area, including on mixed-DPI desktops. [Runtime architecture and source map](docs/ARCHITECTURE.md) · [Public C header](include/rustybox.h)

Run `.\scripts\package.ps1` from the source checkout to create the Windows SDK, examples and standalone Splice Room ZIPs in `dist/`. Use `.\scripts\package-dotnet.ps1` for the NuGet package. To distribute your own app, run `cargo rustybox package` in its project directory. [Asset protection and signing](docs/PACKAGING.md) · [Archive names and contents](docs/GETTING_STARTED.md#package-the-framework)

## Desktop integration

- **Open files and app links.** List extensions and protocols in `rustybox.json`. The packaged starter registers `.rbnote` and `rustybox-note:`, opens a text preview and sends subsequent launches to the same window. [Shell activation and Rust API](docs/ACTIVATION.md)
- **Distribute signed updates.** Set your signing thumbprint and feed URI, then run `cargo rustybox publish`. The command prepares a Windows App Installer descriptor, its detached signature and a versioned MSIX in `dist/updates/`. An explicit rollback can point the same feed at a retained release. [Publish, host and recover an update](docs/UPDATES.md)
- **Navigate with the keyboard.** The shared grid, tree, menu and pane divider expose names, selection and range values to Windows UI Automation. Atlas demonstrates cell navigation, folder type-ahead, visible focus and adjustable pane widths. [Keyboard and Narrator walkthrough](docs/ACCESSIBILITY.md)

![Starter showing a note opened from Explorer and a greeting opened through an app link](docs/images/starter-activation.jpg)

## License and credits

The framework and original example code use the [Apache License 2.0](LICENSE). Bundled fonts, photographs, film artwork and software dependencies have their own terms. [Storefront artwork and fonts](ui/art/CREDITS.md) · [Showcase fonts](examples/showcase/common/fonts) · [Talus artwork and fonts](examples/showcase/talus/ui/CREDITS.md) · [Racing typeface](examples/showcase/racing/ui/fonts) · [Atlas imagery](examples/atlas/ui/plates/CREDITS.md) · [Radio guide photograph](examples/steam/ui/media/CREDITS.md) · [Third-party software notices](licenses/README.md)
