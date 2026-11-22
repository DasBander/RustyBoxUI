# RustyBoxUI

RustyBoxUI is a Windows x64 framework for building desktop apps with Rust, HTML, CSS and JavaScript. One DLL hosts WebView2 and connects the page to native commands. A second DLL lets a Rust or C++ host put a Direct3D 11 texture behind HTML controls in the same window.

![RustyBox storefront showing the featured game Afterlight and the offline game catalog](docs/images/storefront.jpg)

## Run the starter

From a source checkout, open PowerShell in the repository root. Install Rust stable for Windows MSVC, the Visual Studio C++ build tools with a Windows SDK, and the WebView2 Evergreen Runtime. The developer tools use Node.js 22 or newer. [Prerequisites and installation links](docs/GETTING_STARTED.md#prerequisites)

```powershell
.\scripts\build.ps1
.\scripts\run.ps1 -Starter
```

Enter `Ada` and select **Greet**. The HTML form calls Rust and displays **Hello, Ada.** If you have the extracted Windows SDK, you can launch `rustybox-starter.exe` directly.

## Build your own app

From the built checkout or the extracted SDK, generate a project in a sibling directory and launch it.

```powershell
.\scripts\new-app.ps1 -Destination ..\HelloRustyBox
..\HelloRustyBox\build.ps1 -Run
```

Edit the page in `ui/` and implement native commands in `src/main.rs`. The starter includes a typed bridge, a draggable title bar and the files needed to ship the resulting app. [Build your first app](docs/GETTING_STARTED.md) · [Bridge, components, D3D11 and C++ embedding](docs/INTEGRATION.md)

## Examples

| Steam-inspired game library | Foundry material editor |
| --- | --- |
| [![Library home with game collections and offline radio demos](docs/images/steam-home.jpg)](docs/images/steam-home.jpg) | [![Foundry showing a copper sphere in D3D11 beside HTML material controls](docs/images/material.jpg)](docs/images/material.jpg) |

The repository also includes a controller-operated library, a playable race, a patcher that can resume an install, a WAV editor, a satellite texture inspector and a C++ embedding example. [Screenshots, launch commands and source](docs/EXAMPLES.md)

## How the framework fits together

The Rust SDK loads the native DLL through a small C ABI. The browser and native event loop share one UI thread, while a worker persists settings off that thread. The graphics extension presents one rectangular D3D11 viewport per native view. [Runtime architecture and source map](docs/ARCHITECTURE.md) · [Public C header](include/rustybox.h)

The current Windows x64 developer preview is **0.3.6**. Run `.\scripts\package.ps1` from the source checkout to create the SDK ZIP in `dist/`. [Packaging instructions](docs/GETTING_STARTED.md#package-the-framework)

## License and credits

The framework and original example code use the [Apache License 2.0](LICENSE). Bundled fonts, photographs, film artwork and software dependencies have their own terms. [Storefront artwork and fonts](ui/art/CREDITS.md) · [Showcase fonts](examples/showcase/common/fonts) · [Racing typeface](examples/showcase/racing/ui/fonts) · [Atlas imagery](examples/atlas/ui/plates/CREDITS.md) · [Radio guide photograph](examples/steam/ui/media/CREDITS.md) · [Third-party software notices](licenses/README.md)
