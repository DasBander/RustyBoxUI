//! Check the actual runtime DLLs and an executable's bound asset package.
use super::Result;
use std::{env, path::Path};
use webview2_com::{
    CoTaskMemPWSTR, Microsoft::Web::WebView2::Win32::GetAvailableCoreWebView2BrowserVersionString,
};
use windows::{
    Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1},
    core::{PCWSTR, PWSTR},
};

pub(super) fn run(target: Option<&Path>) -> Result<()> {
    let root = match target {
        Some(p) => p.canonicalize().map_err(|e| e.to_string())?,
        None => {
            let cwd = env::current_dir().map_err(|e| e.to_string())?;
            let exe = env::current_exe().map_err(|e| e.to_string())?;
            [
                cwd.join("out"),
                cwd.join("target/release"),
                cwd,
                exe.parent().unwrap().to_owned(),
            ]
            .into_iter()
            .find(|p| p.join("rustybox_ui.dll").is_file())
            .ok_or("Pass the extracted SDK or an application executable")?
        }
    };
    let folder = if root.is_file() {
        root.parent().ok_or("Invalid host location")?
    } else {
        &root
    };
    let mut errors = Vec::new();
    let mut report = |label: &str, result: Result<String>| match result {
        Ok(text) => println!("{label}: {text}"),
        Err(error) => {
            eprintln!("{label}: {error}");
            errors.push(label.to_owned());
        }
    };
    for (file, symbol) in [
        ("rustybox_ui.dll", b"rb_abi_version\0".as_slice()),
        ("rustybox_graphics.dll", b"rbg_abi_version\0".as_slice()),
    ] {
        let path = folder.join(file);
        let result = (|| {
            let abi = unsafe {
                let dll = libloading::Library::new(&path).map_err(|e| e.to_string())?;
                dll.get::<unsafe extern "C" fn() -> u32>(symbol)
                    .map_err(|e| e.to_string())?()
            };
            if abi != 1 {
                return Err(format!("Unsupported ABI {abi}"));
            }
            Ok(format!("{} (ABI {abi})", path.display()))
        })();
        report(file, result);
    }
    let mut version = PWSTR::null();
    report(
        "WebView2",
        unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut version) }
            .map(|()| CoTaskMemPWSTR::from(version).to_string())
            .map_err(|e| e.to_string()),
    );
    let adapters = (|| -> windows::core::Result<String> {
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
        let mut names = Vec::new();
        while let Ok(adapter) = unsafe { factory.EnumAdapters1(names.len() as u32) } {
            let desc = unsafe { adapter.GetDesc1()? };
            let len = desc
                .Description
                .iter()
                .position(|v| *v == 0)
                .unwrap_or(desc.Description.len());
            names.push(String::from_utf16_lossy(&desc.Description[..len]));
        }
        Ok(names.join(", "))
    })();
    report("DXGI adapters", adapters.map_err(|e| e.to_string()));
    if root.is_file() {
        report(
            "Host resources",
            rustybox_assets::verify_host(&root).map(|n| {
                n.map_or_else(
                    || "Development host (loose resources)".into(),
                    |s| format!("{s} (authenticated index and entry page)"),
                )
            }),
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("Failed checks: {}", errors.join(", ")))
    }
}
