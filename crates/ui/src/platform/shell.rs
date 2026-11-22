//! Notification-area lifetime, taskbar progress and menu callbacks for standalone windows.
use super::{Session, err};
use serde::Deserialize;
use serde_json::{Value, json};
use windows::{
    Win32::{
        Foundation::*,
        System::Com::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    },
    core::{HSTRING, w},
};

const TRAY_MESSAGE: u32 = WM_APP + 78;
#[derive(Clone, Deserialize)]
struct Item {
    id: String,
    label: String,
    #[serde(default = "yes")]
    enabled: bool,
}
fn yes() -> bool {
    true
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tray {
    tooltip: String,
    #[serde(default)]
    menu: Vec<Item>,
    #[serde(default)]
    keep_running: bool,
}
#[derive(Default)]
pub(super) struct Shell {
    tray: Option<Tray>,
    taskbar: Option<ITaskbarList3>,
    explorer_created: u32,
}
fn copy_text<const N: usize>(out: &mut [u16; N], value: &str) {
    let mut pos = 0;
    for ch in value.chars().take_while(|ch| *ch != '\0') {
        if pos + ch.len_utf16() >= N {
            break;
        }
        pos += ch.encode_utf16(&mut out[pos..]).len();
    }
}
fn icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    }
}
fn set_icon(hwnd: HWND, tray: &Tray, operation: NOTIFY_ICON_MESSAGE) -> Result<(), String> {
    let mut icon = icon_data(hwnd);
    icon.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    icon.uCallbackMessage = TRAY_MESSAGE;
    let handle = unsafe { SendMessageW(hwnd, WM_GETICON, Some(WPARAM(ICON_SMALL as usize)), None) };
    icon.hIcon = if handle.0 != 0 {
        HICON(handle.0 as *mut _)
    } else {
        unsafe { LoadIconW(None, IDI_APPLICATION) }.map_err(err)?
    };
    copy_text(&mut icon.szTip, &tray.tooltip);
    if unsafe { Shell_NotifyIconW(operation, &icon) }.as_bool() {
        Ok(())
    } else {
        Err("Windows notification area is unavailable".into())
    }
}
pub(super) fn show(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(
            hwnd,
            if IsIconic(hwnd).as_bool() {
                SW_RESTORE
            } else {
                SW_SHOW
            },
        );
        let _ = SetForegroundWindow(hwnd);
    }
}
impl Shell {
    pub(super) fn remove(&mut self, hwnd: HWND) {
        if self.tray.take().is_some() {
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &icon_data(hwnd));
            }
        }
    }
    fn configure(&mut self, hwnd: HWND, params: Value) -> Result<(), String> {
        if params.get("enabled") == Some(&Value::Bool(false)) {
            self.remove(hwnd);
            return Ok(());
        }
        let tray: Tray = serde_json::from_value(params).map_err(err)?;
        if tray.menu.len() > 64
            || tray
                .menu
                .iter()
                .any(|v| v.id.is_empty() || v.label.is_empty() || v.label.contains('\0'))
        {
            return Err("Invalid tray menu".into());
        }
        set_icon(
            hwnd,
            &tray,
            if self.tray.is_some() {
                NIM_MODIFY
            } else {
                NIM_ADD
            },
        )?;
        self.explorer_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        self.tray = Some(tray);
        Ok(())
    }
    fn progress(&mut self, hwnd: HWND, params: Value) -> Result<(), String> {
        let state = match params["state"].as_str().unwrap_or("normal") {
            "none" => TBPF_NOPROGRESS,
            "normal" => TBPF_NORMAL,
            "paused" => TBPF_PAUSED,
            "error" => TBPF_ERROR,
            "indeterminate" => TBPF_INDETERMINATE,
            _ => return Err("Unknown progress state".into()),
        };
        unsafe {
            if self.taskbar.is_none() {
                let taskbar: ITaskbarList3 =
                    CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).map_err(err)?;
                taskbar.HrInit().map_err(err)?;
                self.taskbar = Some(taskbar);
            }
            let taskbar = self.taskbar.as_ref().unwrap();
            if matches!(state, TBPF_NORMAL | TBPF_PAUSED | TBPF_ERROR) {
                let total = params["total"]
                    .as_u64()
                    .filter(|n| *n > 0)
                    .ok_or("Expected positive total")?;
                let completed = params["completed"]
                    .as_u64()
                    .filter(|n| *n <= total)
                    .ok_or("Invalid progress")?;
                taskbar
                    .SetProgressValue(hwnd, completed, total)
                    .map_err(err)?;
            }
            taskbar.SetProgressState(hwnd, state).map_err(err)
        }
    }
}
impl Session {
    pub(super) fn shell_command(&self, method: &str, params: Value) -> Result<Value, String> {
        let hwnd = self.hwnd;
        if unsafe { GetAncestor(hwnd, GA_ROOT) } != hwnd {
            return Err("The embedding host owns desktop integration".into());
        }
        match method {
            "shell.tray" => self.shell.borrow_mut().configure(hwnd, params)?,
            "shell.progress" => self.shell.borrow_mut().progress(hwnd, params)?,
            "shell.notify" => {
                if self.shell.borrow().tray.is_none() {
                    return Err("Configure the tray before posting a notification".into());
                }
                let mut icon = icon_data(hwnd);
                icon.uFlags = NIF_INFO;
                icon.dwInfoFlags = NIIF_INFO;
                copy_text(
                    &mut icon.szInfoTitle,
                    params["title"]
                        .as_str()
                        .ok_or("Expected notification title")?,
                );
                copy_text(
                    &mut icon.szInfo,
                    params["body"]
                        .as_str()
                        .ok_or("Expected notification body")?,
                );
                if !unsafe { Shell_NotifyIconW(NIM_MODIFY, &icon) }.as_bool() {
                    return Err("Windows could not deliver the notification".into());
                }
            }
            "shell.show" => show(hwnd),
            "shell.quit" => {
                self.shell.borrow_mut().remove(hwnd);
                unsafe {
                    PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)).map_err(err)?;
                }
            }
            _ => return Err("Unknown shell command".into()),
        }
        Ok(Value::Null)
    }
    pub(super) fn shell_message(&self, msg: u32, lparam: LPARAM) -> Option<LRESULT> {
        if msg == WM_CLOSE
            && self
                .shell
                .borrow()
                .tray
                .as_ref()
                .is_some_and(|tray| tray.keep_running)
        {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            return Some(LRESULT(0));
        }
        if msg >= 0xc000
            && self
                .shell
                .try_borrow()
                .is_ok_and(|state| msg == state.explorer_created)
        {
            let mut shell = self.shell.borrow_mut();
            shell.taskbar = None;
            if let Some(tray) = &shell.tray {
                let _ = set_icon(self.hwnd, tray, NIM_ADD);
            }
        }
        if msg != TRAY_MESSAGE {
            return None;
        }
        match lparam.0 as u32 {
            WM_LBUTTONUP | NIN_BALLOONUSERCLICK => {
                show(self.hwnd);
                self.shell_event("show");
            }
            WM_RBUTTONUP | WM_CONTEXTMENU => {
                let items = self
                    .shell
                    .borrow()
                    .tray
                    .as_ref()
                    .map(|t| t.menu.clone())
                    .unwrap_or_default();
                if let Ok(Some(action)) = tray_menu(self.hwnd, &items) {
                    match action.as_str() {
                        "show" => show(self.hwnd),
                        "quit" => {
                            let _ = self.shell_command("shell.quit", Value::Null);
                        }
                        _ => self.shell_event(&action),
                    }
                }
            }
            _ => (),
        }
        Some(LRESULT(0))
    }
    fn shell_event(&self, id: &str) {
        let _ = self.post(json!({"v":1,"event":"shell-action","data":{"id":id}}));
        self.emit(json!({"event":"shell_action","id":id}));
    }
}
fn tray_menu(hwnd: HWND, items: &[Item]) -> windows::core::Result<Option<String>> {
    unsafe {
        let menu = CreatePopupMenu()?;
        let result = (|| {
            AppendMenuW(menu, MF_STRING, 1, w!("Open"))?;
            for (index, item) in items.iter().enumerate() {
                let flags = if item.enabled {
                    MF_STRING
                } else {
                    MF_STRING | MF_GRAYED
                };
                AppendMenuW(menu, flags, index + 3, &HSTRING::from(&item.label))?;
            }
            AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
            AppendMenuW(menu, MF_STRING, 2, w!("Quit"))?;
            let mut point = POINT::default();
            GetCursorPos(&mut point)?;
            let _ = SetForegroundWindow(hwnd);
            let command = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                Some(0),
                hwnd,
                None,
            )
            .0;
            let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            Ok(match command {
                0 => None,
                1 => Some("show".into()),
                2 => Some("quit".into()),
                n => items.get((n - 3) as usize).map(|item| item.id.clone()),
            })
        })();
        let _ = DestroyMenu(menu);
        result
    }
}
