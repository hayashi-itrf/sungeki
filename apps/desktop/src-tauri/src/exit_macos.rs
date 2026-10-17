//! Cocoa's Quit menu / Dock Quit bypass Tauri's ExitRequested event.
//! Add the documented applicationShouldTerminate: delegate callback so those
//! paths use the same asynchronous confirmation as the window close button.
use objc2::{
    MainThreadMarker,
    runtime::{AnyClass, AnyObject, Sel},
    sel,
};
use objc2_app_kit::NSApplication;
use std::{
    ffi::{c_char, c_void},
    sync::OnceLock,
};

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
unsafe extern "C" {
    fn class_addMethod(
        class: *const AnyClass,
        name: Sel,
        implementation: *const c_void,
        types: *const c_char,
    ) -> bool;
}

pub fn install(app: &tauri::AppHandle) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("終了処理の登録はメインスレッドで必要です")?;
    let application = NSApplication::sharedApplication(mtm);
    let delegate = application
        .delegate()
        .ok_or("アプリの終了通知先がありません")?;
    let object: &AnyObject = (*delegate).as_ref();
    let class = object.class();
    let selector = sel!(applicationShouldTerminate:);
    // The pinned Tao delegate has no termination decision callback. Do not
    // overwrite an existing implementation if a future runtime adds one.
    if class.instance_method(selector).is_some() {
        return Err("終了通知の実装が変更されています。macOS終了処理の再確認が必要です".into());
    }
    APP.set(app.clone()).map_err(|_| "終了処理は登録済みです")?;
    // SAFETY: NSObject delegate ABI: NSUInteger(id, SEL, NSApplication*).
    // Q is NSUInteger on the supported 64-bit macOS targets; the callback and
    // AppHandle remain valid for the application's entire lifetime.
    let added = unsafe {
        class_addMethod(
            class,
            selector,
            should_terminate as *const c_void,
            c"Q@:@".as_ptr(),
        )
    };
    if !added {
        return Err("macOSの終了確認を登録できませんでした".into());
    }
    Ok(())
}
extern "C" fn should_terminate(
    _delegate: *mut AnyObject,
    _selector: Sel,
    _application: *mut AnyObject,
) -> usize {
    if crate::exit::can_terminate() {
        return 1;
    } // NSTerminateNow
    if let Some(app) = APP.get() {
        crate::exit::request_quit(app);
    }
    0 // NSTerminateCancel; after cleanup AppHandle::exit completes termination.
}
