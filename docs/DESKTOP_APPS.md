# Building desktop apps with RustyBoxUI 0.8

## Edit and recover a document

Open **Splice Room** from a built checkout with `.\scripts\run.ps1 -Example audio`. Choose **Open WAV…** or drop a WAV on the window. Drag across the waveform, or type the start and end times. Select **Trim to selection**, close the window, then reopen it with the same profile. The file, trim and Undo history return. **Export WAV…** or **Ctrl+S** writes the current selection to a destination you choose. Cancelling the dialog leaves the edit available.

The editor keeps a copy of the source in the application's WebView2 profile. It stores edits as sample offsets. Uncompressed PCM and floating-point WAVs retain their original sample bytes when the frame layout matches. Other WAV encodings supported by the decoder export as 16-bit PCM. This example accepts WAVs up to 64 MiB and ten minutes. [Editor source](../examples/showcase/audio/ui) · [WAV writer](../examples/showcase/audio/ui/wave.js)

To use the same workflow in your app, create a `DocumentDraft` for each independently recoverable document slot. Await `replace(name, sourceBlob, initialEdit)` before accepting edits. Pass small, JSON-serializable changes to `update(edit)`. On launch, `await draft.read()` returns the source and latest edit together. Use the same app profile on each launch and a separate draft name for each concurrently edited document.

Register the save command on the Rust host with one `DocumentSaves` per editor window. This example lets the user choose a WAV destination.

```rust
commands.register("host.saveDocument", |state, view, request| {
    state.saves.handle(view.hwnd(), "wav", request)
});
```

In the page, call `await saveDocument(rustybox, wavBlob, "recording-trim.wav")` from `./sdk/documents.js`. A successful export returns `true`. Cancel returns `false`. Rust writes chunks to a temporary file alongside the destination and commits it when all bytes arrive. Call `saves.cancel_pending()` on the native `navigation_starting` event to abandon an interrupted export. In the browser preview, export uses the browser's download behavior. [JS document API](../ui/sdk/documents.d.ts) · [Rust host implementation](../crates/sdk/src/documents.rs)

## Develop with Vite and TypeScript

With the 0.8 CLI on your path, generate a project and install the frontend dependencies. Use Node 22.12+ or a newer supported Node release.

```powershell
cargo rustybox new ..\MyStudio --template vite-ts
Set-Location ..\MyStudio
npm ci --prefix frontend
cargo rustybox dev
```

Edit `frontend/src/main.ts` for the web UI and `src/main.rs` for native commands. Vite updates the running window. Rust changes restart the native host. The typed client for `commands.json` lives in `ui/commands.js` with matching TypeScript declarations. The sample uses vanilla TypeScript. To use React, Vue or Svelte, install its Vite plugin under `frontend/` and configure `frontend/vite.config.mjs`. Keep the entry at `frontend/index.html` and production output in `frontend/dist/`.

Run `cargo rustybox package` for the portable ZIP and MSIX. The production build type-checks TypeScript, bundles assets, and seals the Vite output. The default HTML starter remains available through `cargo rustybox new <directory>`. [Vite template](../templates/vite-ts)

## Integrate with the Windows shell

Relay's **Continue in system tray** option lets the updater keep working after its window closes. Click its tray icon to reopen it. Its tray menu and page controls share the same update commands. Completion uses the Windows notification area and follows the user's notification settings. [Relay implementation](../examples/showcase/patcher/ui/app.js)

```js
await rustybox.invoke('shell.tray', {
  tooltip: 'Studio export', keepRunning: true,
  menu: [{ id: 'pause', label: 'Pause export', enabled: true }],
});
rustybox.addEventListener('shell-action', ({ detail }) => {
  if (detail.id === 'pause') pauseExport();
});
await rustybox.invoke('shell.progress', { state: 'normal', completed: 25, total: 100 });
```

Use `shell.notify` with `{title, body}` after registering a tray icon. `shell.show` restores the window, `shell.quit` exits, and `shell.tray` with `{enabled:false}` removes the icon. Use progress state `none` to clear the taskbar. When you register Rust app actions, `bindActions` connects matching tray IDs, shortcuts and page controls to the same handler. An embedded view delegates desktop integration to the application that owns its top-level window. [Bridge types](../ui/sdk/rustybox.d.ts) · [Action binding](../ui/sdk/actions.js)

## Install optional game content

Talus includes a local feed for **Red Switchback**, a 1.86 km course with seven gates. Open **Settings → Red Switchback pack → Install**, then start a flight. The game remembers the selected course and a separate best time. Remove the pack in Settings to return to Copper Narrows. Installing it works offline from the bundled feed.

Generate your own publisher keys outside the app's resources. Build the feed in a third directory.

```powershell
cargo rustybox content keygen ..\publisher-keys
cargo rustybox content pack .\course-source ..\game-feed canyon 1 ..\publisher-keys
```

Bundle `client.json` from the key directory with your app. Keep `signing.jwk` on the publishing machine. Distribute `canyon.rbcontent` and its adjacent `blocks/` directory together. For a new version, reuse the same publisher keys and output directory, and increment the version. Publish the blocks first and the new manifest last. A remote feed needs HTTPS, an allowed `connect-src` in the page's CSP, and CORS for the app origin if the feed is cross-origin.

```js
import { ContentPacks } from './sdk/content-packs.js';

const config = await (await fetch('./packs/client.json')).json();
const packs = new ContentPacks('my-game', config);
await packs.install('canyon', './packs/canyon.rbcontent', { signal, onProgress });
const level = await (await packs.read('canyon', 'course.json')).text();
```

`signal` is an `AbortSignal`. `onProgress` receives `{verified, total}` in bytes. Both are optional. The installer verifies the publisher signature and each encrypted block before activating the version. A retry reuses verified blocks. The previous version stays readable during an interrupted update. `installed(id)` returns the active version, and `remove(id)` deletes both active and partial data from the app profile. The feed uses 1 MiB data blocks. `read()` materializes one file as a Blob. Use the base package's range reader for streaming very large individual assets. The client holds a decryption key to use installed content. The publisher's private key is required to sign changes. [Content API](../ui/sdk/content-packs.d.ts) · [Base resource packaging](PACKAGING.md#large-game-resources)

## Diagnose an installation or a bridge command

Run `cargo rustybox doctor .\out\MyStudio.exe` with your app's actual executable name. It checks both DLL ABIs, reports the installed WebView2 version and DXGI adapters, and authenticates a sealed host's package index and entry page. Pass an extracted SDK directory to check its runtime DLLs. A failed check returns a nonzero exit code.

In `cargo rustybox dev`, press **Ctrl+Shift+F2** to inspect recent bridge calls, elapsed time and error messages. The inspector keeps up to 120 entries in the current window. Command arguments stay out of the trace.

## Embed in a C# application

The Windows x64 NuGet package targets .NET 10 and includes the two native DLLs and the JavaScript bridge. Build the package with the .NET 10 SDK from the built framework checkout or extracted SDK. The example consumes the resulting package from a local feed.

```powershell
.\scripts\package-dotnet.ps1
dotnet restore .\examples\dotnet-host --source .\dist
dotnet run --project .\examples\dotnet-host --no-restore
```

In a WinForms or WPF UI dispatcher, await `View.OpenAsync(parentHwnd, title, assetsDirectory, profileDirectory)`. Register a typed command, call `view.Pump()` on a UI timer, and pass new client dimensions to `view.Resize(width, height)`. Dispose the view on that thread before it exits. NuGet copies the bridge to `ui/sdk/rustybox.js` in the application output.

```csharp
view.Register<Greeting, Message>("host.greet", input => new Message($"Hello, {input.Name}."));
```

The page calls `rustybox.invoke('host.greet', { name: 'Ada' })` and receives `{text: "Hello, Ada."}`. [WinForms example](../examples/dotnet-host/Program.cs) · [Managed API](../bindings/dotnet/RustyBoxUI/View.cs)
