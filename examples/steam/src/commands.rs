// Generated from commands.json by generate-commands.mjs.
use rustybox_sdk::{Commands, Result, View};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Band {
    pub station: String,
    pub frequency: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DemoPack {
    pub id: String,
    pub channels: Vec<Band>,
}

pub trait AppCommands: Sized + 'static {
    fn demo_pack(&mut self, view: &View, args: Selection) -> Result<DemoPack>;
}

pub fn register<S: AppCommands>() -> Commands<S> {
    let mut commands = Commands::default();
    commands.register("host.demoPack", S::demo_pack);
    commands
}
