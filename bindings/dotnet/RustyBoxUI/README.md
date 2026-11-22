# RustyBoxUI for .NET

Embed an HTML interface in a Windows x64 .NET 10 application. Install the Microsoft Edge WebView2 Evergreen Runtime on the target computer. The package includes the native UI and graphics DLLs and a JavaScript bridge at `ui/sdk/rustybox.js`.

On a running WinForms or WPF UI thread, await `RustyBoxUI.View.OpenAsync(parentHwnd, title, assetsDirectory, profileDirectory)`. The asset directory must contain `index.html`. Register typed `host.*` commands with `view.Register<TRequest, TReply>()`. In the UI loop, call `view.Pump()` to deliver requests and `view.Resize(width, height)` when the container changes size. Dispose the view on its creating thread before the dispatcher exits.

The Windows SDK includes a complete consumer project under `examples/dotnet-host`.
