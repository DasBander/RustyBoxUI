# RustyBoxUI architecture

## Source map

| Directory in the source checkout | Responsibility |
| --- | --- |
| `crates/core/src/lib.rs` | C-compatible configuration, status codes and JSON envelopes |
| `crates/ui/src/lib.rs` | Exported `rb_*` functions, argument validation and handle lifetime |
| `crates/ui/src/platform/` | Win32 window messages, WebView2 initialization, native command routing and D3D viewport updates |
| `crates/ui/src/composition.rs`, `storage.rs` | Composition input routing and per-profile storage workers |
| `crates/graphics/src/` | D3D11 device, swap chain, texture ownership and rendering |
| `crates/assets/src/` | Authenticated package format, executable binding and native reads |
| `crates/cli/src/` | Cargo subcommand and build tool invocation |
| `crates/sdk/src/` | Rust window builder and DLL loading in `lib.rs`, typed command routing in `commands.rs` |
| `ui/sdk/` | Browser client, web components, title bar behavior and TypeScript types |

In the Windows runtime, start with `platform/window.rs` for dragging, resizing and frame timers. `startup.rs` owns asynchronous creation and cancellation. `webview.rs` sets the local origin and navigation handlers, `bridge.rs` routes page messages and `viewport.rs` manages graphics attachment and recovery. In the storefront, `ui/app.js` handles navigation and interaction, while `ui/storefront/views.js` renders the pages and `profile.js` maintains saved state.

## Runtime boundaries

The custom title bar uses the native window’s client area. A maximized Win32 resize border can extend offscreen. During non-client layout the runtime intersects the proposed client rectangle with the destination monitor’s work area, so the title bar and footer stay visible at that monitor’s DPI. The C++ example does the same for the top-level window it owns.

The native ABI is C with explicit version and structure-size checks. A Rust host and a C++ host dynamically load the same `rustybox_ui.dll`. The UI DLL statically links its Rust implementation dependencies and WebView2 loader. The actual WebView2 engine remains an installed Microsoft runtime.

The C ABI carries handles, byte spans, primitive values, and caller-owned buffers. Handles belong to one thread-local registry. COM objects and WebView2 controllers remain on the creating STA thread. Hosts that own their message loops call `rb_tick`; standalone hosts can call `rb_poll`. An attached Direct3D viewport also owns a Win32 frame timer on its view window. Windows dispatches that timer inside the modal move/size loop, including when an external host owns the top-level window. Resizing the swap chain presents its new back buffer immediately. Detaching the viewport or destroying the view releases the timer.

## Message flow

1. A Web Component calls `RustyBoxClient.invoke`.
2. The client sends a versioned JSON request containing ID, per-document session, method, and parameters.
3. WebView2 delivers a message to the STA thread. The runtime validates origin, size, and envelope.
4. Immediate built-ins run on the UI thread. Storage requests enter a bounded worker queue. `host.*` requests enter the C event queue.
5. Worker replies return through `rb_tick`. Host replies arrive through `rb_post_json`.
6. A response includes both request ID and document session. The JS promise resolves or rejects; late, canceled, and obsolete-document replies are ignored.

The public Rust SDK routes generated command types through `rustybox_sdk::Commands`. The contract generator maps records, arrays, nullable types and primitives to Serde and TypeScript. The application implements a trait and receives ordinary runtime events alongside its commands.

`rb_create_begin` starts WebView2 initialization on the caller's STA. Polling a ticket advances environment and controller creation. The caller can cancel or set a deadline. A retired startup keeps its native window, STA and a DLL reference until WebView2's late callback can be collected. The original synchronous entry point drives the same state machine with a 30-second deadline.

A profile registry shares one locked settings file and writer thread among in-process views using the same canonical directory. Requests reserve a slot on the requesting view's reply queue. The worker posts a window wakeup. The last owner joins the worker before releasing the lock.

## Assets and trust

During development the host supplies a local directory mapped to one HTTPS origin per view. A sealed host resolves its executable-bound package, authenticates entries and serves the same origin from memory through WebView2 resource requests. [Package format and distribution](PACKAGING.md) `RB_RESERVED_ORIGIN` selects `rustybox.example`. With the flag absent, existing hosts keep `rustybox.local`. The view checks its selected origin on navigation, frame navigation, incoming commands and outgoing replies. Native settings belong to the profile, and browser-managed state belongs to the origin. External top-level navigation and popups are blocked. Generic host objects are disabled, and device permissions default to denied. The supplied frontend restricts subresources with CSP. Future remote-content support needs a separate unprivileged view policy; it must not inherit native command access implicitly.

## Graphics

Both modes use a D3D11 device, a DXGI flip-model swap chain and a shader resource view for the current texture. The host obtains the viewport device through the C ABI. The graphics DLL checks device identity, array size, sample count and format before replacing the existing texture. Sampling, zoom, channel isolation and alpha preview run on the GPU.

The default mode uses a child HWND. With `RB_COMPOSED`, a DirectComposition root holds an independent swap-chain visual per viewport below a transparent WebView2 visual. The DOM receives mouse movement, capture, scrolling and focus through the composition controller. UI Automation requests use the controller's automation provider. [WebView2 visual hosting](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2compositioncontroller)

Layout crosses the bridge in CSS pixels. The runtime combines the WebView2 controller’s rasterization scale and zoom factor, then rounds the rectangle’s edges to device pixels. It uses window DPI as the fallback for older controllers. Out-of-viewport panels detach. The renderer checks the D3D11 device status even when a hidden swap chain accepts presentation. The runtime retries device creation with bounded backoff after frame failure. The host receives `graphics_restored` and resubmits its content. Atlas keeps one decoded image for this purpose. `rb_graphics_reset` exercises the same recovery path.

## Shutdown

Close the web controller and native surface, remove the context from the registry, destroy its HWND, join the storage worker, unregister the window class when the last context leaves, balance COM initialization, then release the DLL. C++ RAII and the Rust example follow this order.
