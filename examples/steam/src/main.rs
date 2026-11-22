#![windows_subsystem = "windows"]
#[rustfmt::skip]
mod commands;

use commands::{AppCommands, DemoPack, Selection, register};
use rustybox_sdk::{AssetOrigin, Result, View, Window};
use std::path::PathBuf;

struct Library {
    packs: Vec<DemoPack>,
}

impl AppCommands for Library {
    fn demo_pack(&mut self, _: &View, selection: Selection) -> Result<DemoPack> {
        self.packs
            .iter()
            .find(|pack| pack.id == selection.id)
            .cloned()
            .ok_or_else(|| "Unknown demo pack".into())
    }
}

fn arg(flag: &str) -> Option<PathBuf> {
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
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let assets = arg("--assets").unwrap_or_else(|| exe.with_file_name("steam-ui"));
    let profile = match arg("--profile") {
        Some(path) => path,
        None => PathBuf::from(
            std::env::var_os("LOCALAPPDATA").ok_or("Missing local app data directory")?,
        )
        .join("RustyBoxUI/SteamLibrary"),
    };
    let packs = rustybox_assets::Assets::open(&assets)?
        .read("packs.json")?
        .ok_or("Missing demo packs")?;
    let mut window = Window::new("RustyBox Library — Steam-inspired example", assets, profile)?;
    window.size = (1380, 884);
    window.hidden = std::env::args_os().any(|arg| arg == "--hidden");
    let mut library = Library {
        packs: serde_json::from_slice(&packs).map_err(|e| e.to_string())?,
    };
    let mut view = window.open_with_origin(AssetOrigin::Reserved)?;
    let commands = register::<Library>();
    while view.poll()? {
        while let Some(event) = view.next_event()? {
            if !commands.dispatch(&mut library, &view, &event)?
                && matches!(
                    event["event"].as_str(),
                    Some("navigation_failed" | "process_failed")
                )
            {
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
