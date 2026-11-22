//! WebView2 startup tickets. COM callbacks report completion to the owning STA.
use super::{
    SESSIONS, Session, err,
    window::{self, WindowOwner},
};
use crate::{composition::Composition, storage::Storage};
use rustybox_core::{
    AssetOrigin, COMPOSED, Config, DEBUG, HIDDEN, INVALID_HANDLE, OK, PENDING, RESERVED_ORIGIN,
};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    path::PathBuf,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use webview2_com::{
    CoTaskMemPWSTR, CreateCoreWebView2CompositionControllerCompletedHandler,
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    Microsoft::Web::WebView2::Win32::*,
};
use windows::{
    Win32::{Foundation::*, System::LibraryLoader::*, UI::WindowsAndMessaging::*},
    core::{HSTRING, Interface, PWSTR},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

enum Stage {
    Environment(ICoreWebView2Environment),
    Controller(
        ICoreWebView2Controller,
        Option<ICoreWebView2CompositionController>,
    ),
}
type StageResult = Result<Stage, String>;
struct Startup {
    assets: crate::platform::webview::WebAssets,
    storage: Option<Storage>,
    flags: u32,
    deadline: Instant,
    sender: mpsc::Sender<StageResult>,
    receiver: mpsc::Receiver<StageResult>,
    browser_version: String,
    waiting: bool,
    owner: Rc<WindowOwner>,
}
thread_local! {
    static STARTUPS: RefCell<HashMap<u64, Startup>> = RefCell::new(HashMap::new());
    static RETIRED: RefCell<Vec<(Startup, HMODULE)>> = const { RefCell::new(Vec::new()) };
}
pub fn begin(
    config: Config,
    title: String,
    assets: PathBuf,
    data: PathBuf,
    timeout: Duration,
) -> Result<u64, String> {
    let deadline = Instant::now() + timeout;
    let assets = super::webview::WebAssets::open(&assets)?;
    let mut storage = Storage::open(data.clone())?;
    let owner = window::create(config, &title)?;
    storage.set_wakeup(owner.hwnd);
    unsafe {
        windows::Win32::UI::Shell::DragAcceptFiles(owner.hwnd, true);
    }
    let (sender, receiver) = mpsc::channel();
    let tx = sender.clone();
    let user_data = HSTRING::from(data.join("webview").to_string_lossy().as_ref());
    unsafe {
        CreateCoreWebView2EnvironmentWithOptions(
            None,
            &user_data,
            None,
            &CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
                move |status, environment| {
                    let _ = tx.send(status.map_err(err).and_then(|_| {
                        environment
                            .map(Stage::Environment)
                            .ok_or("Missing environment".into())
                    }));
                    Ok(())
                },
            )),
        )
        .map_err(err)?;
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    STARTUPS.with(|map| {
        map.borrow_mut().insert(
            id,
            Startup {
                owner,
                assets,
                flags: config.flags,
                storage: Some(storage),
                deadline,
                sender,
                receiver,
                browser_version: String::new(),
                waiting: true,
            },
        )
    });
    Ok(id)
}
impl Startup {
    fn advance(&mut self) -> Result<Option<Rc<Session>>, String> {
        let stage = match self.receiver.try_recv() {
            Ok(stage) => {
                self.waiting = false;
                stage?
            }
            Err(mpsc::TryRecvError::Empty) => return Ok(None),
            Err(_) => return Err("WebView2 initialization channel closed".into()),
        };
        let hwnd = self.owner.hwnd;
        match stage {
            Stage::Environment(env) => unsafe {
                let mut version = PWSTR::null();
                env.BrowserVersionString(&mut version).map_err(err)?;
                self.browser_version = CoTaskMemPWSTR::from(version).to_string();
                let tx = self.sender.clone();
                if self.flags & COMPOSED != 0 {
                    let env: ICoreWebView2Environment3 = env.cast().map_err(err)?;
                    env.CreateCoreWebView2CompositionController(
                        hwnd,
                        &CreateCoreWebView2CompositionControllerCompletedHandler::create(Box::new(
                            move |status, controller| {
                                let result = status.map_err(err).and_then(|_| {
                                    let comp =
                                        controller.ok_or("Missing composition controller")?;
                                    Ok(Stage::Controller(comp.cast().map_err(err)?, Some(comp)))
                                });
                                let _ = tx.send(result);
                                Ok(())
                            },
                        )),
                    )
                    .map_err(err)?;
                } else {
                    env.CreateCoreWebView2Controller(
                        hwnd,
                        &CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
                            move |status, controller| {
                                let _ = tx.send(status.map_err(err).and_then(|_| {
                                    controller
                                        .map(|c| Stage::Controller(c, None))
                                        .ok_or("Missing controller".into())
                                }));
                                Ok(())
                            },
                        )),
                    )
                    .map_err(err)?;
                }
                self.waiting = true;
                Ok(None)
            },
            Stage::Controller(controller, composition) => unsafe {
                let composition = composition
                    .map(|controller| Composition::new(hwnd, controller))
                    .transpose()
                    .map_err(err)?;
                let webview = controller.CoreWebView2().map_err(err)?;
                let session = Rc::new(Session {
                    hwnd,
                    origin: if self.flags & RESERVED_ORIGIN != 0 {
                        AssetOrigin::Reserved
                    } else {
                        AssetOrigin::LegacyLocal
                    },
                    closed: Cell::new(false),
                    fullscreen: Cell::new(None),
                    host_fullscreen: Cell::new(false),
                    page_fullscreen: Cell::new(false),
                    controller: RefCell::new(Some(controller)),
                    webview: RefCell::new(Some(webview)),
                    events: RefCell::new(VecDeque::new()),
                    storage: self.storage.take().ok_or("Storage closed")?,
                    surfaces: RefCell::new(HashMap::new()),
                    composition,
                    started: Instant::now(),
                    last_frame: Cell::new(Instant::now()),
                    graphics_path: std::env::current_exe()
                        .map_err(err)?
                        .with_file_name("rustybox_graphics.dll"),
                    browser_version: self.browser_version.clone(),
                    debug: self.flags & DEBUG != 0,
                    _owner: self.owner.clone(),
                });
                session.configure(&self.assets, self.flags)?;
                Ok(Some(session))
            },
        }
    }
    fn retire(mut self) {
        // Keep the STA and callback code alive until WebView2 acknowledges an abandoned operation.
        if !self.waiting {
            return;
        }
        match self.receiver.try_recv() {
            Ok(Ok(Stage::Controller(controller, _))) => unsafe {
                let _ = controller.Close();
            },
            Err(mpsc::TryRecvError::Empty) => {
                self.storage.take();
                let mut module = HMODULE::default();
                unsafe {
                    GetModuleHandleExW(
                        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                        windows::core::PCWSTR(begin as *const () as _),
                        &mut module,
                    )
                    .expect("DLL containing callback");
                }
                RETIRED.with(|items| items.borrow_mut().push((self, module)));
            }
            _ => (),
        }
    }
}
pub fn collect() -> bool {
    RETIRED.with(|items| {
        let old = std::mem::take(&mut *items.borrow_mut());
        for (startup, module) in old {
            match startup.receiver.try_recv() {
                Err(mpsc::TryRecvError::Empty) => {
                    items.borrow_mut().push((startup, module));
                    continue;
                }
                Ok(Ok(Stage::Controller(controller, _))) => unsafe {
                    let _ = controller.Close();
                },
                _ => (),
            }
            drop(startup);
            unsafe {
                let _ = FreeLibrary(module);
            }
        }
        items.borrow().is_empty()
    })
}
pub fn cancel(id: u64) -> bool {
    let startup = STARTUPS.with(|map| map.borrow_mut().remove(&id));
    if let Some(startup) = startup {
        startup.retire();
        true
    } else {
        false
    }
}
pub fn finish(id: u64) -> Result<i32, String> {
    let Some(mut startup) = STARTUPS.with(|map| map.borrow_mut().remove(&id)) else {
        return Ok(INVALID_HANDLE);
    };
    if Instant::now() >= startup.deadline {
        startup.retire();
        return Err("WebView2 initialization deadline exceeded".into());
    }
    let Some(session) = startup.advance()? else {
        STARTUPS.with(|map| map.borrow_mut().insert(id, startup));
        return Ok(PENDING);
    };
    unsafe {
        SetWindowLongPtrW(session.hwnd, GWLP_USERDATA, id as isize);
    }
    session.fit();
    SESSIONS.with(|map| map.borrow_mut().insert(id, session.clone()));
    if startup.flags & HIDDEN == 0 {
        unsafe {
            let _ = ShowWindow(session.hwnd, SW_SHOW);
        }
    }
    session.emit(json!({"event":"created","handle":id}));
    Ok(OK)
}
