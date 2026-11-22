#![windows_subsystem = "windows"]
#[rustfmt::skip]
mod commands;
use commands::*;
use rustybox_sdk::{Result, View, Window};
use std::path::PathBuf;

struct App;
impl AppCommands for App {
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
    let profile =
        PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("Missing local app data directory")?)
            .join("RustyBoxUI/Starter");
    let mut config = Window::new(
        "RustyBox starter",
        executable.with_file_name("starter-ui"),
        profile,
    )?;
    config.size = (980, 680);
    let mut view = config.open_with_origin(rustybox_sdk::AssetOrigin::Reserved)?;
    let commands = register::<App>();
    let mut app = App;
    while view.poll()? {
        while let Some(event) = view.next_event()? {
            commands.dispatch(&mut app, &view, &event)?;
        }
        view.wait(200)?;
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        rustybox_sdk::show_error(&error);
        std::process::exit(1);
    }
}
