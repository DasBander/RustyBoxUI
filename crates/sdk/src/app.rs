//! An optional event loop for applications with shared state and multiple native windows.
use crate::{
    AssetOrigin, Commands, Result, View, Window,
    activation::{Activation, SingleInstance},
};
use rustybox_core::response;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
pub struct Windows(Rc<RefCell<Vec<(String, Window)>>>);
impl Windows {
    /// Creation happens after the current command returns, outside a native callback.
    pub fn open(&self, label: impl Into<String>, window: Window) {
        self.0.borrow_mut().push((label.into(), window));
    }
}
/// An application command with shared state, per-window availability and an optional shortcut.
pub struct Action<S> {
    pub id: &'static str,
    pub title: &'static str,
    pub shortcut: &'static str,
    pub enabled: fn(&S, &View) -> bool,
    pub run: fn(&mut S, &View) -> Result<()>,
}
// Return None for application-defined methods so the generated router can handle them.
fn dispatch_action<S>(
    actions: &[Action<S>],
    state: &mut S,
    view: &View,
    request: &Value,
) -> Option<Result<Value>> {
    match request["method"].as_str()? {
        "host.actions" => {
            let entries = actions
                .iter()
                .map(|action| {
                    json!({
                        "id": action.id,
                        "title": action.title,
                        "shortcut": action.shortcut,
                        "enabled": (action.enabled)(state, view)
                    })
                })
                .collect();
            Some(Ok(Value::Array(entries)))
        }
        "host.performAction" => {
            let id = request["params"]["id"].as_str();
            let action = actions
                .iter()
                .find(|action| Some(action.id) == id && (action.enabled)(state, view));
            Some(match action {
                Some(action) => (action.run)(state, view).map(|()| Value::Null),
                None => Err("Action is unavailable".into()),
            })
        }
        _ => None,
    }
}

type ActivationHandler<S> = Box<dyn FnMut(&mut S, &View, Activation) -> Result<()>>;
struct Launches<S> {
    instance: SingleInstance,
    window_label: String,
    receive: ActivationHandler<S>,
}
type Event<S> = Box<dyn FnMut(&mut S, &View, &Value) -> Result<()>>;
type Idle<S> = Box<dyn FnMut(&mut S, &View) -> Result<()>>;
pub struct App<S> {
    state: S,
    commands: Commands<S>,
    windows: Windows,
    views: Vec<View>,
    actions: Vec<Action<S>>,
    event: Option<Event<S>>,
    idle: Option<Idle<S>>,
    launches: Option<Launches<S>>,
}
impl<S: 'static> App<S> {
    pub fn new(state: S, commands: Commands<S>, windows: Windows) -> Self {
        Self {
            state,
            commands,
            windows,
            views: vec![],
            actions: vec![],
            event: None,
            idle: None,
            launches: None,
        }
    }
    pub fn action(&mut self, action: Action<S>) {
        assert!(
            !self.actions.iter().any(|a| a.id == action.id),
            "Duplicate app action"
        );
        self.actions.push(action);
    }
    pub fn on_event(&mut self, handler: impl FnMut(&mut S, &View, &Value) -> Result<()> + 'static) {
        self.event = Some(Box::new(handler));
    }
    pub fn on_idle(&mut self, handler: impl FnMut(&mut S, &View) -> Result<()> + 'static) {
        self.idle = Some(Box::new(handler));
    }
    /// Dispatch launch requests on the UI thread to the named window.
    pub fn on_activation(
        &mut self,
        instance: SingleInstance,
        window_label: impl Into<String>,
        handler: impl FnMut(&mut S, &View, Activation) -> Result<()> + 'static,
    ) {
        self.launches = Some(Launches {
            instance,
            window_label: window_label.into(),
            receive: Box::new(handler),
        });
    }

    pub fn run(mut self) -> Result<()> {
        use std::time::{Duration, Instant};
        let mut next_launch = Instant::now();
        loop {
            let pending = std::mem::take(&mut *self.windows.0.borrow_mut());
            for (label, window) in pending {
                if let Some(existing) = self.views.iter().find(|v| v.label() == label) {
                    unsafe {
                        let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(
                            existing.hwnd(),
                        );
                    }
                    continue;
                }
                let mut view = window.open_with_origin(AssetOrigin::Reserved)?;
                view.label = label;
                self.views.push(view);
            }
            if self.views.is_empty() {
                return Ok(());
            }
            if let Some(launches) = &mut self.launches
                && Instant::now() >= next_launch
                && let Some(view) = self
                    .views
                    .iter()
                    .find(|view| view.label() == launches.window_label)
            {
                next_launch = Instant::now() + Duration::from_millis(180);
                launches.instance.dispatch(|launch| {
                    use windows::Win32::UI::WindowsAndMessaging::{
                        IsIconic, IsWindowVisible, SW_RESTORE, SetForegroundWindow, ShowWindow,
                    };
                    let hwnd = view.hwnd();
                    unsafe {
                        if IsWindowVisible(hwnd).as_bool() {
                            if IsIconic(hwnd).as_bool() {
                                let _ = ShowWindow(hwnd, SW_RESTORE);
                            }
                            let _ = SetForegroundWindow(hwnd);
                        }
                    }
                    (launches.receive)(&mut self.state, view, launch)
                })?;
            }
            let mut i = 0;
            while i < self.views.len() {
                let view = &mut self.views[i];
                if !view.poll()? {
                    self.views.remove(i);
                    continue;
                }
                while let Some(event) = view.next_event()? {
                    if event["event"] == "invoke" {
                        let req = &event["request"];
                        let result = dispatch_action(&self.actions, &mut self.state, view, req);
                        if let Some(result) = result {
                            view.post(&response(
                                req["id"].as_u64().ok_or("Missing command ID")?,
                                req["session"].as_str().unwrap_or(""),
                                result,
                            ))?;
                            continue;
                        }
                    }
                    if !self.commands.dispatch(&mut self.state, view, &event)?
                        && let Some(handler) = &mut self.event
                    {
                        handler(&mut self.state, view, &event)?;
                    }
                }
                if let Some(handler) = &mut self.idle {
                    handler(&mut self.state, view)?;
                }
                i += 1;
            }
            // The STA wait wakes on messages for all windows. An idle hook can drain worker progress.
            if self.windows.0.borrow().is_empty()
                && let Some(view) = self.views.first()
            {
                view.wait(if self.idle.is_some() || self.views.len() > 1 {
                    16
                } else {
                    200
                })?;
            }
        }
    }
}
