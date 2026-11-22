# Build your first RustyBoxUI app

The starter is a desktop window with an HTML form. Enter a name, and a Rust handler returns a greeting to the page. This guide runs that app, adds a second command, and produces a folder you can copy to another Windows machine.

## Prerequisites

Use a 64-bit PowerShell session on Windows x64.

| What you want to do | What you need |
| --- | --- |
| Run an app from the extracted SDK | The Microsoft Edge WebView2 Evergreen Runtime. [Runtime download](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) |
| Compile the framework or a generated Rust app | Rust stable with the `x86_64-pc-windows-msvc` toolchain. In the Visual Studio Installer select **Desktop development with C++**, the x64 MSVC tools and a Windows SDK. [Rust installer](https://rustup.rs/) · [C++ build tools](https://learn.microsoft.com/en-us/cpp/build/vscpp-step-0-installation) |
| Generate a project from source, edit command contracts, package or use the browser preview | Node.js 22 or newer. The scripts use the Node standard library. [Node.js downloads](https://nodejs.org/en/download) |

With Rust installed, `rustup show` lists the active toolchain. If its MSVC target is missing, run `rustup target add x86_64-pc-windows-msvc`. Cargo downloads the locked Rust dependencies on the first build.

## Run an example

**From a source checkout**, run these commands from the directory containing the top-level `Cargo.toml`.

```powershell
.\scripts\build.ps1
.\scripts\run.ps1 -Starter
```

The build stages the DLLs, hosts and web assets under `target\release`. Enter `Ada` and select **Greet** to see **Hello, Ada.** To open the storefront, run `.\scripts\run.ps1`. [Other examples](EXAMPLES.md)

**From an extracted SDK**, open PowerShell in the directory containing `rustybox_ui.dll`.

```powershell
.\scripts\install.ps1 -CheckOnly
.\rustybox-starter.exe
```

The check reports the installed WebView2 version. If the runtime is missing, install Evergreen from the prerequisites table. The optional `.\scripts\install.ps1` installs the example apps under the current user's Programs folder and can install Evergreen. For offline setup it accepts `-RuntimeInstaller` with the path to Microsoft's standalone installer.

## Create a project

From the built source checkout or the extracted SDK root, choose an empty or new sibling directory.

```powershell
.\scripts\new-app.ps1 -Destination ..\HelloRustyBox
Set-Location ..\HelloRustyBox
.\build.ps1 -Run
```

The generated project builds independently of the framework checkout. Close the running window before rebuilding.

| File or directory | What to edit |
| --- | --- |
| `ui/index.html`, `ui/starter.css`, `ui/app.js` | Page layout, appearance and events |
| `commands.json` | Names of host commands and their input and result types |
| `src/main.rs` | Rust implementations, window title, size and profile directory |
| `src/commands.rs`, `ui/commands.js`, `ui/commands.d.ts` | Generated bindings. Regenerate these after changing the contract. |
| `vendor/`, `runtime/`, `licenses/` | Rust SDK sources, native DLLs and redistribution notices |

Give your app its own profile path in `src/main.rs`. The starter uses `%LOCALAPPDATA%\RustyBoxUI\Starter` for its WebView2 profile and native settings. Give the window its display name in `Window::new`. [Starter host source](../examples/starter/src/main.rs)

## Add a web-to-Rust command

This example makes the existing form count words. In the generated project's `commands.json` replace the `commands` object with the following entries. The existing `Message` record already has a `text` field.

```json
{
  "greet": { "input": "Greeting", "output": "Message" },
  "countWords": { "input": "Message", "output": "u32" }
}
```

From the generated project directory, regenerate the bindings.

```powershell
node .\generate-commands.mjs .
```

In `src/main.rs` add this method inside the existing `impl AppCommands for App` block.

```rust
fn count_words(&mut self, _view: &View, input: Message) -> Result<u32> {
    Ok(input.text.split_whitespace().count() as u32)
}
```

In `ui/app.js` replace the two lines inside the form handler's `try` block with the following.

```js
const text = String(new FormData(event.target).get('name'));
const count = await api.countWords({ text });
document.querySelector('output').textContent = String(count);
```

Run `.\build.ps1 -Run`, enter `one two three` and submit the form. It displays **3**. The generated JS client invokes `host.countWords`, and the Rust command router checks the argument type before calling the handler. You can check for stale bindings with `node .\generate-commands.mjs . --check`. [Bridge and component reference](INTEGRATION.md)

## Distribute your app

Run `.\build.ps1` in the generated project and distribute the complete `out\` directory. It contains the executable, both runtime DLLs, `starter-ui\` with the HTML and bundled SDK, and `licenses\`. Launch `out\rustybox-starter.exe` on a Windows x64 machine with WebView2 Evergreen installed. The first launch creates the user's profile.

## Package the framework

From the framework source checkout, run `.\scripts\package.ps1`. If you already ran the release build, use `.\scripts\package.ps1 -SkipBuild`. The script assembles the DLLs, hosts, public headers, SDK sources, screenshots and licenses in `dist\RustyBoxUI-0.3.6` and writes `dist\RustyBoxUI-0.3.6-windows-x64.zip`.

For a frontend-only preview from source, run `npm run preview` for the storefront or `npm run preview:steam` for the library. Open `http://127.0.0.1:4173`. Browser previews use local storage. Use the Windows hosts to exercise native commands, window controls and Direct3D.
