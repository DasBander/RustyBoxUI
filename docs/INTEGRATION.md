# Framework integration

A RustyBoxUI application combines a native host, web assets with an `index.html` entry point, and a writable profile for WebView2 and native settings. The host loads `rustybox_ui.dll` from beside the executable. Apps with a GPU viewport also load `rustybox_graphics.dll`. [Runnable starter](GETTING_STARTED.md) · [C ABI header](../include/rustybox.h)

## Open a Rust window

Call `rustybox_sdk::enable_per_monitor_dpi()` at startup. Pass a title and absolute paths for the assets and profile to `Window::new`. Set the logical size on `config.size`, then open the view with `config.open_with_origin(AssetOrigin::Reserved)`. The starter resolves its assets relative to its executable so it can launch from another working directory. [Starter host](../examples/starter/src/main.rs)

The starter queues its main window, then gives its shared state and generated command router to the SDK. A second window can share the state, profile and command registry. Opening an existing label focuses its window. Closing the final window exits the loop.

```rust
let windows = rustybox_sdk::Windows::default();
windows.open("main", config);
rustybox_sdk::App::new(App, register::<App>(), windows).run()
```

Use `View::label()` inside a handler to distinguish windows. Use `App::action` to register a native title, shortcut, enabled check and handler. In the page, call `bindActions(client)` from `sdk/actions.js` and assign the action ID to toolbar buttons or menu items with `data-rb-action`. Post an `actions-changed` event after a state change to refresh the controls. The host checks availability again on invocation.

A host that owns its own message loop can continue calling `view.poll()`, dispatching `view.next_event()?` to its router, and calling `view.wait(timeout_ms)` until work arrives. [App and action API](../crates/sdk/src/app.rs) · [Multiwindow example](../examples/atlas/src/main.rs)

The reserved origin serves local assets at `https://rustybox.example`. `Window::open()` and C hosts that omit `RB_RESERVED_ORIGIN` use the legacy `https://rustybox.local` origin. When migrating an existing app, browser cookies, IndexedDB and localStorage belong to the original origin. Native `storage.*` values follow the profile directory.

## Call native commands from JavaScript

Import the client in a page module. Each invocation returns a promise. Built-in methods work across hosts. Your application defines the `host.*` methods it supports.

```js
import { rustybox } from './sdk/rustybox.js';

const runtime = await rustybox.invoke('runtime.info');
await rustybox.invoke('storage.set', { key: 'theme', value: 'dark' });
const theme = await rustybox.invoke('storage.get', { key: 'theme' });
```

| Method | Behavior |
| --- | --- |
| `runtime.info`, `diagnostics.echo` | Inspect the runtime or round-trip a JSON value. |
| `storage.get`, `storage.set` | Read or write JSON settings through the profile's storage worker. |
| `window.minimize`, `window.maximize`, `window.close`, `window.drag` | Control the top-level window. Maximize toggles restore. |
| `graphics.attach`, `graphics.detach`, `graphics.info` | Set the viewport bounds in CSS pixels, release it, or read its current state. |
| `graphics.view` | Change zoom (0.25–8), color channel (`rgb`, `red`, `green`, `blue`, `alpha`) and animated pan. |
| `host.*` | Send an application-defined request to the native host. |

For a typed application command, declare its schema in `commands.json`, generate the Rust and JS bindings, then implement the generated trait. The starter implements `host.greet`. [Add a typed command](GETTING_STARTED.md#add-a-web-to-rust-command) · [JavaScript and TypeScript API](../ui/sdk/rustybox.d.ts)

```js
import { bindCommands } from './commands.js';
import { rustybox } from './sdk/rustybox.js';

const api = bindCommands(rustybox);
const { text } = await api.greet({ name: 'Ada' });
```

Each request can specify `{ timeout, signal }` as its final options argument. A timeout or an `AbortSignal` ends the caller's wait. Work already accepted by the host can continue. Keep bridge payloads within 256 KiB and at most 128 JS requests pending. Native settings have a 256 KiB total quota. Views in one process can share a profile. Concurrent processes need distinct profiles.

The runtime accepts bridge messages from the mapped local origin and blocks external document navigation and popups. Keep your page's Content Security Policy in `index.html` and validate command arguments in the host. [Starter page and CSP](../examples/starter/ui/index.html)

## Use web components and a draggable title bar

Link `theme.css` in the page. In an external module import `./sdk/components.js` to register `rb-button`, `rb-toggle`, `rb-field`, `rb-menu`, `rb-dialog`, `rb-game-card` and `rb-native-view`. The stylesheet also supplies `rb-badge`. [Component implementations](../ui/sdk/components.js) · [Forms, menus and dialogs](../ui/sdk/forms.js)

```html
<header data-rb-drag>
  <strong>My app</strong>
  <button id="close" aria-label="Close window">×</button>
</header>
<rb-button variant="accent">Continue</rb-button>
<rb-toggle label="Reduced motion"></rb-toggle>
```

```js
import './sdk/components.js';
import './sdk/titlebar.js';
import { rustybox } from './sdk/rustybox.js';

document.querySelector('#close').addEventListener('click', () => rustybox.invoke('window.close'));
```

Dragging the caption moves the native window and double-clicking toggles maximize. Buttons, links and form fields keep their normal behavior. Mark other interactive regions with `data-rb-no-drag`. For game tiles, assign an object from the catalog to the element's `.game` property and listen for `game-open` and `game-wishlist`. [Game catalog](../ui/catalog.js)

## Fullscreen windows

From a user gesture, query or set `window.fullscreen` on the RustyBox client. A standalone Windows view fills its current monitor, including the taskbar area, and restores its prior normal or maximized placement on exit. An embedded child view reports `supported: false` because its parent owns the top-level window.

```js
const state = await rustybox.invoke('window.fullscreen');
if (state.supported) {
  await rustybox.invoke('window.fullscreen', { enabled: !state.enabled });
}
```

The browser preview client implements the same command with the document Fullscreen API. Cache the current mode and issue the setter directly in the click or key handler to preserve browser user activation. Listen for `fullscreenchange` in the browser to keep labels current when the browser exits fullscreen. The native runtime also responds when WebView2 reports that an HTML element entered fullscreen. [Talus display controls](../examples/showcase/talus/ui/game.js) · [WebView2 fullscreen events](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2#add_containsfullscreenelementchanged)

## Show a Direct3D 11 texture

1. Set `config.composed = true` before opening a Rust view, or add `RB_COMPOSED` to the C flags. This places HTML controls above a DirectComposition surface. The default mode places a native child HWND above the page.
2. Add `<rb-native-view id="preview"></rb-native-view>` to the page. Give it a size in CSS, for example `display: block; width: 100%; height: 320px`. Keep its ancestors transparent where you want the GPU image to show.
3. Import the components and assign the bridge client. The element tracks layout, scroll and DPI changes. Listen for `native-ready` when your page needs to tell the host the viewport has attached.

```js
import './sdk/components.js';
import { rustybox } from './sdk/rustybox.js';

const preview = document.querySelector('#preview');
preview.client = rustybox;
await preview.sync();
```

Once attached, the viewport displays the built-in scene. To supply your own image, obtain the device using `view.device()?`. Create a single-sample, single-slice `DXGI_FORMAT_R8G8B8A8_UNORM` or `DXGI_FORMAT_B8G8R8A8_UNORM` texture on that same device with `D3D11_BIND_SHADER_RESOURCE`. Call `view.set_texture(Some(&texture))?`, then submit updates on the view thread before the next tick. `view.set_texture(None)?` restores the built-in scene. The corresponding C calls are `rb_graphics_device` and `rb_graphics_texture`. Release the owning COM reference returned by `rb_graphics_device` on the view thread.

Give each additional `<rb-native-view viewport="1">` its own numeric ID and assign its `client`. Each window supports up to 16 active rectangular viewports. Use `view.device_at(id)` and `view.set_texture_at(id, texture)` or the C functions `rb_graphics_device_at` and `rb_graphics_texture_at`. Omitting the ID uses viewport zero. Keep each element inside the visible client area. When the host receives `graphics_restored`, recreate its texture on the replacement device. Atlas retains the decoded pixels for that purpose. [Atlas texture upload](../examples/atlas/src/image.rs) · [Device recovery in the host](../examples/atlas/src/main.rs) · [Graphics ABI](../include/rustybox_graphics.h)

## Embed the DLL in a C++ application

Construct `rustybox::Runtime` on the UI thread with an absolute path to `rustybox_ui.dll`. The wrapper loads the DLL, checks the ABI version, and fills `struct_size` and `abi_version` when you call `open`. This C++17 helper opens a child view in an existing Win32 client area. [RAII wrapper](../include/rustybox.hpp) · [Working Win32 host](../examples/cpp-host/main.cpp)

```cpp
#include "rustybox.hpp"
#include <filesystem>

void open_child(rustybox::Runtime& ui, HWND parent,
                const std::filesystem::path& assets,
                const std::filesystem::path& profile) {
    RECT bounds{};
    if (!GetClientRect(parent, &bounds)) throw std::runtime_error("Invalid parent window");
    const auto web = std::filesystem::absolute(assets).u8string();
    const auto data = std::filesystem::absolute(profile).u8string();
    RBConfig config{};
    config.parent_window = reinterpret_cast<uintptr_t>(parent);
    config.width = static_cast<uint32_t>(bounds.right - bounds.left);
    config.height = static_cast<uint32_t>(bounds.bottom - bounds.top);
    config.flags = RB_RESERVED_ORIGIN;
    config.assets = reinterpret_cast<const uint8_t*>(web.data());
    config.assets_len = web.size();
    config.data_dir = reinterpret_cast<const uint8_t*>(data.data());
    config.data_dir_len = data.size();
    ui.open(config);
}
```

Forward `WM_SIZE` to `ui.resize`, run the application's message pump, and call `ui.tick()` regularly. Drain `ui.next_event(json)` each iteration. For an `invoke` event, dispatch `request.method` and reply with `ui.post`. Copy the request's `id` and `session` into the response. A success reply looks like this.

```json
{"v":1,"id":7,"session":"document-session-id","ok":true,"result":{"text":"Hello, Ada."}}
```

For an error, set `ok` to `false` and provide an `error` object with `code` and `message`. Sessions prevent a late reply from resolving a request on a new page.

For a standalone native window, use `parent_window = 0` and `rb_poll` to pump messages. Keep handles on the creating COM STA thread, initialize outside UI callbacks and `DllMain`, and destroy all views before unloading the DLLs. For cancellable startup, use `rb_create_begin`, pump STA messages and poll `rb_create_status`. After cancelling or timing out, keep pumping and call `rb_collect_startups` until it returns `RB_OK`. WebView2 may finish reading a packaged response on another thread after its window closes. Once it serves a nonempty packaged response, the UI DLL stays mapped until process exit so those COM callbacks remain valid. Destroying a view still releases its window and controller. All configuration strings are copied during creation. `rb_next_event` supports a non-consuming size query and returns UTF-8 JSON without a terminating NUL. [C API and lifetime rules](../include/rustybox.h)

## Jobs and filesystem access

Launch long-running work with `Job::spawn`. The worker receives a typed `JobContext<P>`. Call `ctx.checkpoint()?` between files and `ctx.report(progress)?` for incremental results. Drain `job.progress()` and `job.try_finish()` in an idle handler. Call `job.cancel()` to request cancellation. UI and D3D objects stay on the owning thread. [Job API](../crates/sdk/src/jobs.rs) · [Atlas folder worker](../examples/atlas/src/project.rs)

Use `files::pick_file` or `files::pick_folder` with the owning HWND. A `FileGrant` resolves relatives inside the granted location. Native file drops arrive as `files_dropped` events. Keep absolute filesystem paths in the host and expose IDs to the page.

## Desktop controls

Import `./sdk/desktop.js` and link `sdk/desktop.css`. Give `rb-table` a `columns` array with keys and labels, plus a `rows` array with stable IDs. It renders visible rows and supports arrow and page keys. Give `rb-tree` items with `id`, `label` and optional `children`. Both controls dispatch `selection-change` with the selected record in `event.detail`. Give each control an accessible name.

Place `rb-split` between panes. Its parent exposes the current percentage as `--rb-split` for your grid or positioning rules. Arrow keys, Home and End adjust the split. Set `min`, `max` and `value` as percentages. The tree, table and split share CSS variables including `--rb-focus`, `--rb-panel`, `--rb-selected` and `--rb-separator`. [Atlas folder browser](../examples/atlas/ui/project-browser.js)
