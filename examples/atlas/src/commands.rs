// Generated from commands.json by generate-commands.mjs.
use rustybox_sdk::{Commands, Result, View};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Empty {
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    pub title: String,
    pub file_name: String,
    pub location: String,
    pub credit: String,
    pub thumbnail: Option<String>,
    pub width: u32,
    pub height: u32,
    pub bytes: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    pub id: String,
}

pub trait AppCommands: Sized + 'static {
    fn catalog(&mut self, view: &View, args: Empty) -> Result<Vec<Asset>>;
    fn select_asset(&mut self, view: &View, args: Selection) -> Result<Asset>;
    fn import_image(&mut self, view: &View, args: Empty) -> Result<Option<Asset>>;
}

pub fn register<S: AppCommands>() -> Commands<S> {
    let mut commands = Commands::default();
    commands.register("host.catalog", S::catalog);
    commands.register("host.selectAsset", S::select_asset);
    commands.register("host.importImage", S::import_image);
    commands
}
