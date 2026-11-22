mod patch;
mod scene;

use rustybox_sdk::{AssetOrigin, Commands, Result, View, Window};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use windows::Win32::UI::Input::XboxController::*;

type Setup = fn(&mut State, &mut Commands<State>, &Path, &rustybox_assets::Assets) -> Result<()>;

pub struct App {
    key: &'static str,
    title: &'static str,
    shader: Option<&'static str>,
    setup: Option<Setup>,
}

// Each executable links the assets and host commands from its own definition.
pub const RACING: App = App {
    key: "racing",
    title: "Nightshift — Red Mesa Circuit",
    shader: Some("racing.hlsl"),
    setup: None,
};
pub const TALUS: App = App {
    key: "talus",
    title: "Talus — Canyon Flight",
    shader: None,
    setup: None,
};
pub const MATERIAL: App = App {
    key: "material",
    title: "Foundry — Material editor",
    shader: Some("material.hlsl"),
    setup: None,
};
pub const AUDIO: App = App {
    key: "audio",
    title: "Splice Room — Wave editor",
    shader: None,
    setup: None,
};
pub const COUCH: App = App {
    key: "couch",
    title: "Couch Library",
    shader: None,
    setup: Some(|_, commands, _, _| {
        commands.register("host.gamepad", State::gamepad);
        commands.register("host.launchRace", State::launch_race);
        Ok(())
    }),
};
pub const PATCHER: App = App {
    key: "patcher",
    title: "Relay — Game updates",
    shader: None,
    setup: Some(|state, commands, profile, assets| {
        state.patcher = Some(patch::Worker::new(profile.join("Relay-1.4"), assets)?);
        commands.register("host.patch", State::patch);
        Ok(())
    }),
};
fn argument(name: &str) -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(value) = args.next() {
        if value == name {
            return args.next().map(Into::into);
        }
    }
    None
}
struct State {
    shader: Option<(String, String)>,
    scene: Option<scene::Scene>,
    frame: Option<scene::Frame>,
    patcher: Option<patch::Worker>,
}
impl State {
    fn draw(&mut self, view: &View, frame: scene::Frame) -> Result<()> {
        let device = view.device()?;
        if self.scene.as_ref().is_none_or(|s| !s.matches(&device)) {
            let (vs, ps) = self.shader.as_ref().ok_or("No scene")?;
            self.scene = Some(scene::Scene::new(device, vs, ps)?);
        }
        self.scene.as_mut().unwrap().draw(view, &frame)?;
        self.frame = Some(frame);
        Ok(())
    }
    fn gamepad(&mut self, _: &View, _: Value) -> Result<Value> {
        let mut state = XINPUT_STATE::default();
        for slot in 0..XUSER_MAX_COUNT {
            if unsafe { XInputGetState(slot, &mut state) } != 0 {
                continue;
            }
            let pad = state.Gamepad;
            let mut keys = vec![];
            for (flag, key) in [
                (XINPUT_GAMEPAD_A, "open"),
                (XINPUT_GAMEPAD_B, "back"),
                (XINPUT_GAMEPAD_LEFT_SHOULDER, "previous"),
                (XINPUT_GAMEPAD_RIGHT_SHOULDER, "next"),
                (XINPUT_GAMEPAD_DPAD_UP, "up"),
                (XINPUT_GAMEPAD_DPAD_DOWN, "down"),
                (XINPUT_GAMEPAD_DPAD_LEFT, "left"),
                (XINPUT_GAMEPAD_DPAD_RIGHT, "right"),
            ] {
                if pad.wButtons.contains(flag) {
                    keys.push(key);
                }
            }
            if pad.sThumbLX < -16_000 {
                keys.push("left");
            }
            if pad.sThumbLX > 16_000 {
                keys.push("right");
            }
            if pad.sThumbLY < -16_000 {
                keys.push("down");
            }
            if pad.sThumbLY > 16_000 {
                keys.push("up");
            }
            return Ok(json!({"connected":true,"keys":keys}));
        }
        Ok(json!({"connected":false,"keys":[]}))
    }
    fn launch_race(&mut self, _: &View, _: Value) -> Result<()> {
        let exe = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("rustybox-racing.exe");
        Command::new(exe)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    fn patch(&mut self, _: &View, args: PatchRequest) -> Result<patch::Status> {
        let worker = self.patcher.as_mut().ok_or("Patcher unavailable")?;
        match args.action.as_str() {
            "status" => (),
            "start" => worker.send(patch::Action::Start)?,
            "pause" => worker.send(patch::Action::Pause)?,
            "verify" => worker.send(patch::Action::Verify)?,
            _ => return Err("Unknown patch action".into()),
        }
        Ok(worker.read())
    }
}
#[derive(Deserialize)]
struct PatchRequest {
    action: String,
}

fn run(app: App, hidden: bool) -> Result<()> {
    rustybox_sdk::enable_per_monitor_dpi();
    let App {
        key,
        title,
        shader,
        setup,
    } = app;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let assets = argument("--assets").unwrap_or_else(|| exe.with_file_name(format!("{key}-ui")));
    let profile = match argument("--profile") {
        Some(profile) => profile,
        None => PathBuf::from(
            std::env::var_os("LOCALAPPDATA").ok_or("Missing local app data directory")?,
        )
        .join("RustyBoxUI")
        .join(key),
    };
    let content = rustybox_assets::Assets::open(&assets)?;
    let shader_text = |name: &str| -> Result<String> {
        String::from_utf8(
            content
                .read(&format!("_native/{name}"))?
                .ok_or("Missing scene shader")?,
        )
        .map_err(|e| e.to_string())
    };
    let mut config = Window::new(title, assets, profile.clone())?;
    config.size = (1280, 800);
    config.hidden = hidden;
    config.composed = shader.is_some();
    let mut view = config.open_with_origin(AssetOrigin::Reserved)?;
    let mut state = State {
        shader: match shader {
            Some(name) => Some((shader_text("fullscreen.hlsl")?, shader_text(name)?)),
            None => None,
        },
        scene: None,
        frame: None,
        patcher: None,
    };
    let mut commands = Commands::default();
    if shader.is_some() {
        commands.register("host.scene", State::draw);
    }
    if let Some(setup) = setup {
        setup(&mut state, &mut commands, &profile, &content)?;
    }
    while view.poll()? {
        while let Some(event) = view.next_event()? {
            if commands.dispatch(&mut state, &view, &event)? {
                continue;
            }
            match event["event"].as_str() {
                Some("graphics_lost") => state.scene = None,
                Some("graphics_restored") => {
                    if let Some(frame) = state.frame.clone() {
                        state.draw(&view, frame)?;
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
pub fn launch(app: App) {
    let hidden = std::env::args_os().any(|a| a == "--hidden");
    if let Err(error) = run(app, hidden) {
        if hidden {
            eprintln!("{error}");
        } else {
            rustybox_sdk::show_error(&error);
        }
        std::process::exit(1);
    }
}
