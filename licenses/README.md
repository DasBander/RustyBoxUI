# Third-party software

RustyBoxUI uses the WebView2 Rust bindings and statically links Microsoft's x64 WebView2 SDK loader. The Evergreen WebView2 Runtime is installed on the user's machine. The SDK and generated apps include `DEPENDENCIES.txt` and the license files supplied with the locked Cargo packages.

| Component | Included notice | Upstream source |
| --- | --- | --- |
| `webview2-com` 0.39.1 and `webview2-com-sys` 0.39.1 | [MIT license](webview2-rs-MIT.txt) | [webview2-rs source revision](https://github.com/wravery/webview2-rs/tree/edc2caf886175ccaebe86078c9cfe1ae2a187328) |
| `webview2-com-macros` 0.8.1 | [MIT license](webview2-rs-MIT.txt) | [Macro source revision](https://github.com/wravery/webview2-rs/tree/dffa41a8a46d3f5565eefbff2de57d38d399f158) |
| Microsoft.Web.WebView2 SDK 1.0.3800.47 | [SDK license](WebView2-SDK-LICENSE.txt) and [third-party notices](WebView2-SDK-NOTICE.txt) | [Microsoft's NuGet package](https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.3800.47) |
