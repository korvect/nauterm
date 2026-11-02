//! Sparkle's standard macOS updater, loaded only from this app's framework bundle.
use anyhow::{Context, Result, ensure};
use objc2::{
    MainThreadMarker, msg_send,
    rc::{Allocated, Retained},
    runtime::{AnyClass, AnyObject},
};
use objc2_foundation::{NSBundle, NSString};
use std::cell::RefCell;

struct Updater {
    _framework: Retained<NSBundle>,
    controller: Retained<AnyObject>,
}
thread_local! {
    static UPDATER: RefCell<Option<Updater>> = const { RefCell::new(None) };
}

fn setting(name: &str) -> Option<String> {
    NSBundle::mainBundle()
        .objectForInfoDictionaryKey(&NSString::from_str(name))?
        .downcast::<NSString>()
        .ok()
        .map(|value| value.to_string())
        .filter(|value| !value.trim().is_empty())
}

pub fn configuration() -> Result<()> {
    ensure!(
        setting("SUFeedURL").is_some() && setting("SUPublicEDKey").is_some(),
        "This build does not include a Sparkle update feed and public key."
    );
    Ok(())
}

pub fn init() -> Result<()> {
    ensure!(
        MainThreadMarker::new().is_some(),
        "Sparkle requires the main thread."
    );
    configuration()?;
    UPDATER.with(|slot| {
        if slot.borrow().is_some() {
            return Ok(());
        }
        let path = NSBundle::mainBundle()
            .privateFrameworksPath()
            .context("This application has no Frameworks directory.")?;
        let framework =
            NSBundle::bundleWithPath(&NSString::from_str(&format!("{path}/Sparkle.framework")))
                .context("Sparkle.framework is missing from this application.")?;
        // SAFETY: Load only the framework embedded and signed by our packager.
        ensure!(
            unsafe { framework.load() },
            "Could not load Sparkle.framework."
        );
        let class = AnyClass::get(c"SPUStandardUpdaterController")
            .context("Sparkle's standard updater is unavailable.")?;
        // SAFETY: These selectors and nil delegates match Sparkle 2.9.4's
        // SPUStandardUpdaterController.h. The retained controller is main-thread
        // confined and updates NSBundle.mainBundle using Sparkle's own UI.
        let controller: Retained<AnyObject> = unsafe {
            let allocated: Allocated<AnyObject> = msg_send![class, alloc];
            msg_send![allocated, initWithStartingUpdater: true,
                updaterDelegate: std::ptr::null::<AnyObject>(),
                userDriverDelegate: std::ptr::null::<AnyObject>()]
        };
        *slot.borrow_mut() = Some(Updater {
            _framework: framework,
            controller,
        });
        Ok(())
    })
}

/// Call on the foreground executor, outside a GPUI App/Window update: AppKit
/// can synchronously dispatch notifications while presenting its updater UI.
pub fn check_for_updates() -> Result<()> {
    init()?;
    UPDATER.with(|slot| {
        let slot = slot.borrow();
        let controller = &slot.as_ref().unwrap().controller;
        // SAFETY: Controller/updater selectors match the pinned Sparkle headers;
        // both retained objects are live and this function is main-thread only.
        unsafe {
            let updater: Retained<AnyObject> = msg_send![controller, updater];
            let ready: bool = msg_send![&updater, canCheckForUpdates];
            if ready {
                let _: () = msg_send![controller, checkForUpdates: std::ptr::null::<AnyObject>()];
            }
        }
    });
    Ok(())
}
