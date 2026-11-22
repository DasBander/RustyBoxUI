# Open files and app links

A generated Windows app can own file extensions and URI schemes. Add these entries to its existing `rustybox.json`, then rebuild. Keep the same `identity` across releases.

```json
{
  "fileTypes": [".rbnote"],
  "protocols": ["rustybox-note"]
}
```

Use lowercase extensions, including the period, and lowercase scheme names without a colon. Both lists are optional. The MSIX manifest registers the handlers when Windows installs the package. Windows lets the user select the default application for each file type.

## Try the starter

From the generated project, run `cargo rustybox build`, then create and open a note. With a custom executable name, use that filename.

```powershell
Set-Content -Encoding utf8 .\session.rbnote 'Take 4 at the coast.'
Start-Process -FilePath .\out\rustybox-starter.exe -ArgumentList '-- .\session.rbnote'
```

When the note appears in the starter, send the greeting link from the same PowerShell session.

```powershell
& .\out\rustybox-starter.exe -- 'rustybox-note://greet?name=Marc'
```

Look for **Hello, Marc.** and the text of `session.rbnote` under **Opened in this window**. After installing the signed MSIX you can also use Explorer’s **Open with** menu or open `rustybox-note://greet?name=Marc` from Windows Run. Choose your own scheme and matching routes for a real application. The starter renders file contents and link text using text nodes. [Starter screenshot](images/starter-activation.jpg)

## Receive an activation in Rust

`Activation::from_arguments` converts the launching process’s relative filenames to absolute paths and checks input against the registered extensions and protocols. It returns `Target::File(PathBuf)` or `Target::Url(String)`. A no-argument launch has an empty target list.

Call `SingleInstance::claim(&profile, identity, &launch)` before creating a window. A forwarded process returns `Ok(None)` and exits. The primary process passes its guard and main window label to `App::on_activation`. Its callback runs on the UI thread and receives shared app state, the target view and the activation. Keep state in Rust so a page can fetch an item received before navigation finishes. [Starter host](../examples/starter/src/main.rs) · [Rust activation API](../crates/sdk/src/activation.rs)

Each user profile, application identity and Windows session has its own queue. Keep the profile in the current user’s private app-data directory. The primary acknowledges an item after its callback succeeds. If the process exits during delivery, Windows releases the lock and a later launch can replay a pending item. Make operations such as opening a document idempotent. Ordinary launches bring the main window forward subject to Windows foreground-focus rules.

File associations and protocol launch parameters use the packaged desktop app extensions provided by Windows. [Microsoft’s desktop integration guide](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/desktop-to-uwp-extensions)
