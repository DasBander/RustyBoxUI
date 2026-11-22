//! Local asset mapping, browser settings and navigation callbacks.
use super::{Session, err};
use rustybox_core::SELF_TEST;
use serde_json::json;
use std::rc::Rc;
use webview2_com::{
    CoTaskMemPWSTR, ContainsFullScreenElementChangedEventHandler,
    Microsoft::Web::WebView2::Win32::*, NavigationCompletedEventHandler,
    NavigationStartingEventHandler, NewWindowRequestedEventHandler,
    PermissionRequestedEventHandler, ProcessFailedEventHandler, WebMessageReceivedEventHandler,
};
use windows::core::{HSTRING, Interface, PWSTR};

pub(super) enum WebAssets {
    Directory(std::path::PathBuf),
    Packed(Rc<rustybox_assets::Package>),
}
impl WebAssets {
    pub(super) fn open(path: &std::path::Path) -> Result<Self, String> {
        Ok(match rustybox_assets::Assets::open(path)? {
            rustybox_assets::Assets::Directory(root) => Self::Directory(root),
            rustybox_assets::Assets::Packed(pack) => Self::Packed(Rc::new(pack)),
        })
    }
}
impl Session {
    pub(super) unsafe fn configure(
        self: &Rc<Self>,
        assets: &WebAssets,
        flags: u32,
    ) -> Result<(), String> {
        unsafe {
            let web = self.webview.borrow().as_ref().unwrap().clone();
            let controller = self.controller.borrow().as_ref().unwrap().clone();
            let settings = web.Settings().map_err(err)?;
            settings.SetAreDevToolsEnabled(self.debug).map_err(err)?;
            settings
                .SetAreDefaultContextMenusEnabled(self.debug)
                .map_err(err)?;
            settings.SetIsStatusBarEnabled(false).map_err(err)?;
            settings.SetAreHostObjectsAllowed(false).map_err(err)?;
            settings.SetIsZoomControlEnabled(false).map_err(err)?;
            if let Ok(regions) = settings.cast::<ICoreWebView2Settings9>() {
                regions
                    .SetIsNonClientRegionSupportEnabled(true)
                    .map_err(err)?;
            }
            let bg: ICoreWebView2Controller2 = controller.cast().map_err(err)?;
            bg.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                A: if self.composition.is_some() { 0 } else { 255 },
                R: 0,
                G: 0,
                B: 0,
            })
            .map_err(err)?;
            let origin = self.origin;
            match assets {
                WebAssets::Packed(package) => {
                    super::resources::mount(&web, origin, package.clone()).map_err(err)?
                }
                WebAssets::Directory(assets) => {
                    let mapped: ICoreWebView2_3 = web.cast().map_err(err)?;
                    // Windows extended-length paths are valid for Rust FS but not WebView2 folder mapping.
                    let path = assets.to_string_lossy();
                    let path = path.strip_prefix("\\\\?\\").unwrap_or(&path);
                    mapped
                        .SetVirtualHostNameToFolderMapping(
                            &HSTRING::from(origin.host()),
                            &HSTRING::from(path),
                            COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_DENY_CORS,
                        )
                        .map_err(err)?;
                }
            }
            let weak = Rc::downgrade(self);
            let mut token = 0;
            web.add_ContainsFullScreenElementChanged(
                &ContainsFullScreenElementChangedEventHandler::create(Box::new(
                    move |sender, _| {
                        if let (Some(session), Some(web)) = (weak.upgrade(), sender) {
                            let mut active = windows::core::BOOL::default();
                            web.ContainsFullScreenElement(&mut active)?;
                            session
                                .request_fullscreen(&session.page_fullscreen, active.as_bool())?;
                        }
                        Ok(())
                    },
                )),
                &mut token,
            )
            .map_err(err)?;
            let weak = Rc::downgrade(self);
            web.add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                    if let (Some(session), Some(args)) = (weak.upgrade(), args) {
                        let mut source = PWSTR::null();
                        args.Source(&mut source)?;
                        if !session
                            .origin
                            .trusts(&CoTaskMemPWSTR::from(source).to_string())
                        {
                            return Ok(());
                        }
                        let mut raw = PWSTR::null();
                        args.WebMessageAsJson(&mut raw)?;
                        let raw = CoTaskMemPWSTR::from(raw).to_string();
                        let outcome =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                session.dispatch(&raw)
                            }));
                        if outcome.is_err() {
                            session.emit(
                                json!({"event":"error","message":"Command handler panicked"}),
                            );
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(err)?;
            web.add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let mut uri = PWSTR::null();
                        args.Uri(&mut uri)?;
                        if !origin.trusts(&CoTaskMemPWSTR::from(uri).to_string()) {
                            args.SetCancel(true)?;
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(err)?;
            web.add_FrameNavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let mut uri = PWSTR::null();
                        args.Uri(&mut uri)?;
                        if !origin.trusts(&CoTaskMemPWSTR::from(uri).to_string()) {
                            args.SetCancel(true)?;
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(err)?;
            web.add_NewWindowRequested(
                &NewWindowRequestedEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        args.SetHandled(true)?;
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(err)?;
            web.add_PermissionRequested(
                &PermissionRequestedEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?;
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(err)?;
            let weak = Rc::downgrade(self);
            web.add_NavigationCompleted(
                &NavigationCompletedEventHandler::create(Box::new(move |_, args| {
                    if let Some(session) = weak.upgrade() {
                        let mut success = windows::core::BOOL(0);
                        if let Some(args) = args {
                            args.IsSuccess(&mut success)?;
                        }
                        let event = if success.as_bool() {
                            "ready"
                        } else {
                            "navigation_failed"
                        };
                        session.emit(json!({ "event": event }));
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(err)?;
            let weak = Rc::downgrade(self);
            web.add_ProcessFailed(
                &ProcessFailedEventHandler::create(Box::new(move |_, _| {
                    if let Some(session) = weak.upgrade() {
                        session.emit(json!({
                            "event": "process_failed",
                            "message": "WebView2 process failed; recreate this view",
                        }));
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(err)?;
            let suffix = if flags & SELF_TEST != 0 {
                "?selftest=1"
            } else {
                ""
            };
            let url = format!("{}/index.html{suffix}", origin.base());
            web.Navigate(&HSTRING::from(url)).map_err(err)?;
            controller.SetIsVisible(true).map_err(err)?;
        }
        Ok(())
    }
}
