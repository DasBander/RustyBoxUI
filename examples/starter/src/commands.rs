// Generated from commands.json by generate-commands.mjs.
use rustybox_sdk::{Commands, Result, View};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Greeting {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Message {
    pub text: String,
}

pub trait AppCommands: Sized + 'static {
    fn greet(&mut self, view: &View, args: Greeting) -> Result<Message>;
}

pub fn register<S: AppCommands>() -> Commands<S> {
    let mut commands = Commands::default();
    commands.register("host.greet", S::greet);
    commands
}
