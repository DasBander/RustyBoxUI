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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageRequest {
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Workspace {
    pub role: String,
    pub folder: Option<String>,
    pub selected: String,
    pub count: u32,
    pub running: bool,
    pub skipped: u32,
    pub generation: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderRequest {
    pub id: String,
    pub viewport: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Thumbnail {
    pub id: String,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ids {
    pub ids: Vec<String>,
}

pub trait AppCommands: Sized + 'static {
    fn catalog(&mut self, view: &View, args: Empty) -> Result<Vec<Asset>>;
    fn select_asset(&mut self, view: &View, args: Selection) -> Result<Asset>;
    fn import_image(&mut self, view: &View, args: Empty) -> Result<Option<Asset>>;
    fn workspace(&mut self, view: &View, args: Empty) -> Result<Workspace>;
    fn page(&mut self, view: &View, args: PageRequest) -> Result<Vec<Asset>>;
    fn thumbnails(&mut self, view: &View, args: Ids) -> Result<Vec<Thumbnail>>;
    fn render_asset(&mut self, view: &View, args: RenderRequest) -> Result<Asset>;
}

pub fn register<S: AppCommands>() -> Commands<S> {
    let mut commands = Commands::default();
    commands.register("host.catalog", S::catalog);
    commands.register("host.selectAsset", S::select_asset);
    commands.register("host.importImage", S::import_image);
    commands.register("host.workspace", S::workspace);
    commands.register("host.page", S::page);
    commands.register("host.thumbnails", S::thumbnails);
    commands.register("host.renderAsset", S::render_asset);
    commands
}
