#![windows_subsystem = "windows"]

use rustybox_sdk::{AssetOrigin, Commands, Result, View, Window};
use serde_json::{Value, json};
use std::path::PathBuf;
use windows::Win32::UI::{
    HiDpi::GetDpiForSystem,
    WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN},
};

fn argument(flag: &str) -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == flag {
            return args.next().map(Into::into);
        }
    }
    None
}

fn run() -> Result<()> {
    rustybox_sdk::enable_per_monitor_dpi();
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let assets = argument("--assets").unwrap_or_else(|| executable.with_file_name("ui"));
    let profile = match argument("--data-dir") {
        Some(path) => path,
        None => PathBuf::from(
            std::env::var_os("LOCALAPPDATA").ok_or("Missing local app data directory")?,
        )
        .join("RustyBoxUI/showcase"),
    };
    let mut window = Window::new("RustyBox Launcher", assets, profile)?;
    // Preserve a desktop margin on smaller displays. The SDK accepts logical pixels.
    let dpi = unsafe { GetDpiForSystem() }.max(96);
    let logical = |metric, margin: i32, minimum: i32| {
        (unsafe { GetSystemMetrics(metric) } - margin).max(minimum) as u32 * 96 / dpi
    };
    window.size = (
        1520.min(logical(SM_CXSCREEN, 80, 960)),
        1120.min(logical(SM_CYSCREEN, 100, 640)),
    );
    window.hidden = std::env::args_os().any(|arg| arg == "--hidden");
    let mut view = window.open_with_origin(AssetOrigin::Reserved)?;
    let mut commands = Commands::default();
    commands.register("host.ping", |_: &mut (), _: &View, message: Value| {
        Ok(json!({"pong": true, "host": "Rust dynamic DLL host", "received": message}))
    });
    while view.poll()? {
        while let Some(event) = view.next_event()? {
            commands.dispatch(&mut (), &view, &event)?;
            if matches!(
                event["event"].as_str(),
                Some("navigation_failed" | "process_failed")
            ) {
                return Err(event.to_string());
            }
        }
        view.wait(100)?;
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        rustybox_sdk::show_error(&error);
        std::process::exit(1);
    }
}
