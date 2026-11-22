#![windows_subsystem = "windows"]
#[rustfmt::skip]
mod commands;
mod image;
use commands::*;
use image::{ImageFile, Pixels};
use rustybox_sdk::{Result, View, Window};
use std::path::{Path, PathBuf};

struct Entry {
    asset: Asset,
    path: PathBuf,
}
struct Atlas {
    entries: Vec<Entry>,
    selected: Option<(String, Pixels)>,
}
impl Atlas {
    fn new(assets: &Path) -> Result<Self> {
        let mut entries = Vec::new();
        for (id, title, location, credit) in [
            (
                "lena-delta",
                "Lena River Delta",
                "Sakha, Russia",
                "USGS EROS · Landsat 7",
            ),
            (
                "namib",
                "Namib Sand Sea",
                "Namibia",
                "NASA Earth Observatory · USGS",
            ),
            (
                "vatnajokull",
                "Vatnajökull",
                "Iceland",
                "USGS EROS · Landsat 7",
            ),
        ] {
            let file_name = format!("{id}.jpg");
            let url = format!("plates/{file_name}");
            let path = assets.join(&url);
            let file = ImageFile::open(&path)?;
            entries.push(Entry {
                path,
                asset: Asset {
                    id: id.into(),
                    title: title.into(),
                    location: location.into(),
                    credit: credit.into(),
                    thumbnail: Some(url),
                    file_name,
                    width: file.width,
                    height: file.height,
                    bytes: file.bytes,
                },
            });
        }
        Ok(Self {
            entries,
            selected: None,
        })
    }
    fn import(&mut self, view: &View, path: PathBuf) -> Result<Asset> {
        if let Some(entry) = self.entries.iter().find(|e| e.path == path) {
            return self.select_asset(
                view,
                Selection {
                    id: entry.asset.id.clone(),
                },
            );
        }
        if self.entries.len() >= 128 {
            return Err("This session holds up to 128 images".into());
        }
        let file = ImageFile::open(&path)?;
        let pixels = file.decode()?;
        pixels.display(view)?;
        let id = format!("local-{}", self.entries.len());
        let asset = Asset {
            id: id.clone(),
            title: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            file_name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            location: "Local import".into(),
            credit: "Imported from this computer".into(),
            thumbnail: None,
            width: file.width,
            height: file.height,
            bytes: file.bytes,
        };
        self.selected = Some((id, pixels));
        self.entries.push(Entry {
            asset: asset.clone(),
            path,
        });
        Ok(asset)
    }
}
impl AppCommands for Atlas {
    fn catalog(&mut self, _: &View, _: Empty) -> Result<Vec<Asset>> {
        Ok(self.entries.iter().map(|e| e.asset.clone()).collect())
    }
    fn select_asset(&mut self, view: &View, selection: Selection) -> Result<Asset> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.asset.id == selection.id)
            .ok_or("Unknown asset")?;
        if let Some((id, pixels)) = &self.selected
            && *id == selection.id
        {
            pixels.display(view)?;
        } else {
            let pixels = ImageFile::open(&entry.path)?.decode()?;
            pixels.display(view)?;
            self.selected = Some((selection.id, pixels));
        }
        Ok(entry.asset.clone())
    }
    fn import_image(&mut self, view: &View, _: Empty) -> Result<Option<Asset>> {
        let Some(path) = image::pick(view.hwnd())? else {
            return Ok(None);
        };
        self.import(view, path).map(Some)
    }
}
fn arg(flag: &str) -> Option<PathBuf> {
    let mut args = std::env::args_os();
    while let Some(a) = args.next() {
        if a == flag {
            return args.next().map(Into::into);
        }
    }
    None
}
fn run() -> Result<()> {
    rustybox_sdk::enable_per_monitor_dpi();
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let assets = arg("--assets").unwrap_or_else(|| exe.with_file_name("atlas-ui"));
    let profile = arg("--profile").unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("RustyBoxUI/Atlas")
    });
    let mut config = Window::new("Atlas — Texture workspace", assets.clone(), profile)?;
    config.composed = true;
    config.hidden = std::env::args_os().any(|arg| arg == "--hidden");
    let mut view = config.open_with_origin(rustybox_sdk::AssetOrigin::Reserved)?;
    let mut app = Atlas::new(&assets)?;
    let handlers = register::<Atlas>();
    while view.poll()? {
        while let Some(event) = view.next_event()? {
            if handlers.dispatch(&mut app, &view, &event)? {
                continue;
            }
            match event["event"].as_str() {
                Some("graphics_restored") => {
                    // Decoded pixels survive device loss. The replacement device needs a new texture.
                    if let Some((_, pixels)) = &app.selected {
                        pixels.display(&view)?;
                    }
                }
                Some("navigation_failed" | "process_failed" | "graphics_failed") => {
                    return Err(event.to_string());
                }
                _ => (),
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
