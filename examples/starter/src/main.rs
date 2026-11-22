#![windows_subsystem = "windows"]
#[rustfmt::skip]
mod commands;
use commands::*;
use rustybox_sdk::activation::{Activation, SingleInstance, Target};
use rustybox_sdk::{Result, View, Window};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

const ID: &str = env!("RUSTYBOX_APP_ID");

#[derive(Default)]
struct App {
    opened: Vec<OpenedItem>,
}

fn read_note(path: &Path) -> std::io::Result<String> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other("Choose a regular text file"));
    }
    let mut bytes = Vec::new();
    file.take(4097).read_to_end(&mut bytes)?;
    let more = bytes.len() > 4096;
    bytes.truncate(4096);
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if more {
        text.push('…');
    }
    Ok(text)
}
impl App {
    fn activate(&mut self, view: &View, launch: Activation) -> Result<()> {
        if launch.targets.is_empty() {
            return Ok(());
        }
        for target in launch.targets {
            let item = match target {
                Target::File(path) => OpenedItem {
                    title: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    text: read_note(&path)
                        .unwrap_or_else(|e| format!("Could not open this note: {e}")),
                    url: String::new(),
                },
                Target::Url(url) => OpenedItem {
                    title: "App link".into(),
                    text: String::new(),
                    url,
                },
            };
            self.opened.push(item);
        }
        self.opened.drain(..self.opened.len().saturating_sub(6));
        // A newly created page can read the same state after its event listener is ready.
        let _ = view.post(&serde_json::json!({"v":1,"event":"activation-changed","data":{}}));
        Ok(())
    }
}
impl AppCommands for App {
    fn opened_items(&mut self, _view: &View, _: Empty) -> Result<Vec<OpenedItem>> {
        Ok(self.opened.clone())
    }
    fn greet(&mut self, _view: &View, input: Greeting) -> Result<Message> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err("Enter a name".into());
        }
        Ok(Message {
            text: format!("Hello, {name}."),
        })
    }
}
fn run() -> Result<()> {
    rustybox_sdk::enable_per_monitor_dpi();
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut profile =
        PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("Missing local app data directory")?)
            .join("RustyBoxUI")
            .join(if ID == "RustyBoxUI.Starter" {
                "Starter".into()
            } else {
                format!("app-{ID}")
            });
    let mut hidden = false;
    let mut targets = Vec::new();
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--profile" {
            profile = PathBuf::from(args.next().ok_or("Missing profile path")?);
        } else if arg == "--hidden" {
            hidden = true;
        } else if arg == "--" {
            targets.extend(args);
            break;
        } else {
            targets.push(arg);
        }
    }
    let file_types: Vec<_> = env!("RUSTYBOX_FILE_TYPES")
        .split(',')
        .filter(|v| !v.is_empty())
        .collect();
    let protocols: Vec<_> = env!("RUSTYBOX_PROTOCOLS")
        .split(',')
        .filter(|v| !v.is_empty())
        .collect();
    let launch = Activation::from_arguments(targets, &file_types, &protocols)?;
    let Some(instance) = SingleInstance::claim(&profile, ID, &launch)? else {
        return Ok(());
    };
    let mut config = Window::new(
        "RustyBox starter",
        executable.with_file_name("starter-ui"),
        profile,
    )?;
    config.size = (980, 680);
    config.hidden = hidden;
    let windows = rustybox_sdk::Windows::default();
    windows.open("main", config);
    let mut app = rustybox_sdk::App::new(App::default(), register::<App>(), windows);
    app.on_activation(instance, "main", App::activate);
    app.run()
}
fn main() {
    if let Err(error) = run() {
        rustybox_sdk::show_error(&error);
        std::process::exit(1);
    }
}
