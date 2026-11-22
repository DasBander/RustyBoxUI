//! Typed host handlers and replies for the WebView message bridge.
use crate::{Result, View};
use rustybox_core::response;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::HashMap;

type Handler<S> = Box<dyn Fn(&mut S, &View, Value) -> Result<Value>>;
/// Maps each registered method to its Serde input and output types.
pub struct Commands<S> {
    handlers: HashMap<&'static str, Handler<S>>,
}
impl<S> Default for Commands<S> {
    fn default() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
}
impl<S: 'static> Commands<S> {
    pub fn register<A, R>(
        &mut self,
        method: &'static str,
        handler: fn(&mut S, &View, A) -> Result<R>,
    ) where
        A: DeserializeOwned + 'static,
        R: Serialize + 'static,
    {
        let wrapped = move |state: &mut S, view: &View, value: Value| {
            let args = serde_json::from_value(value)
                .map_err(|e| format!("Invalid parameters for {method}: {e}"))?;
            serde_json::to_value(handler(state, view, args)?).map_err(|e| e.to_string())
        };
        assert!(
            self.handlers.insert(method, Box::new(wrapped)).is_none(),
            "Duplicate command {method}"
        );
    }
    /// Routes only invocation events. Other runtime events remain available to the application.
    pub fn dispatch(&self, state: &mut S, view: &View, event: &Value) -> Result<bool> {
        if event["event"] != "invoke" {
            return Ok(false);
        }
        let request = &event["request"];
        let method = request["method"].as_str().ok_or("Missing command name")?;
        let id = request["id"].as_u64().ok_or("Missing request id")?;
        let session = request["session"]
            .as_str()
            .ok_or("Missing document session")?;
        let result = self
            .handlers
            .get(method)
            .ok_or_else(|| format!("Unknown command {method}"))
            .and_then(|h| h(state, view, request["params"].clone()));
        view.post(&response(id, session, result))?;
        Ok(true)
    }
}
