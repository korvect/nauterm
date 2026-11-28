//! Uses the same AppKit association/chooser/opening APIs as the reference.
//! Only chooser/open requests need the UI thread; catalog queries use a worker.
use super::*;
use async_channel::{Receiver, bounded};
use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSModalResponseCancel, NSModalResponseOK, NSOpenPanel, NSRunningApplication,
    NSWorkspace, NSWorkspaceOpenConfiguration,
};
use objc2_foundation::{NSArray, NSBundle, NSError, NSFileManager, NSString, NSURL};

// CoreServices' Create/Copy APIs return +1 objects. CFString/CFArray are
// toll-free bridged to NSString/NSArray. Signatures/roles match the local SDK's
// LaunchServices UTType.h, LSInfo.h and LSConstants.h and the reference bridge.
#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    static kUTTagClassFilenameExtension: *const NSString;
    fn UTTypeCreatePreferredIdentifierForTag(
        class: *const NSString,
        tag: *const NSString,
        conforming: *const NSString,
    ) -> *mut NSString;
    fn LSCopyAllRoleHandlersForContentType(
        content_type: *const NSString,
        role: u32,
    ) -> *mut NSArray<NSString>;
    fn LSCopyDefaultRoleHandlerForContentType(
        content_type: *const NSString,
        role: u32,
    ) -> *mut NSString;
}
fn preferred_type(extension: &str) -> Option<Retained<NSString>> {
    let extension = NSString::from_str(extension);
    // SAFETY: SDK-owned tag-class constant and a live NSString; Create transfers
    // its nullable +1 CFString result to the toll-free Objective-C owner.
    unsafe {
        Retained::from_raw(UTTypeCreatePreferredIdentifierForTag(
            kUTTagClassFilenameExtension,
            &*extension,
            std::ptr::null(),
        ))
    }
}

fn application(url: &NSURL, default: Option<&NSURL>) -> Option<Application> {
    let bundle = NSBundle::bundleWithURL(url)?;
    let identifier = bundle.bundleIdentifier()?.to_string();
    let path = url.to_file_path()?;
    let path_text = url.path()?;
    let display = NSFileManager::defaultManager().displayNameAtPath(&path_text);
    let name = Path::new(&display.to_string())
        .file_stem()?
        .to_string_lossy()
        .into_owned();
    Some(Application {
        name,
        identifier,
        path,
        is_default: default == Some(url),
    })
}
pub(super) fn list(path: &Path) -> Result<Vec<Application>> {
    objc2::rc::autoreleasepool(|_| {
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            return Ok(vec![]);
        };
        let Some(content_type) = preferred_type(&extension.to_lowercase()) else {
            return Ok(vec![]);
        };
        let workspace = NSWorkspace::sharedWorkspace();
        // SAFETY: Live type identifier, SDK UInt32 kLSRolesAll. Both Copy results
        // are nullable +1 toll-free Foundation objects, released by Retained.
        let (handlers, default) = unsafe {
            (
                Retained::from_raw(LSCopyAllRoleHandlersForContentType(
                    &*content_type,
                    u32::MAX,
                )),
                Retained::from_raw(LSCopyDefaultRoleHandlerForContentType(
                    &*content_type,
                    u32::MAX,
                )),
            )
        };
        let apps = handlers
            .iter()
            .flat_map(|handlers| handlers.iter())
            .filter_map(|identifier| {
                let url = workspace.URLForApplicationWithBundleIdentifier(&identifier)?;
                let mut app = application(&url, None)?;
                app.is_default = default
                    .as_ref()
                    .is_some_and(|default| default.to_string() == identifier.to_string());
                Some(app)
            })
            .collect();
        let own = NSBundle::mainBundle()
            .bundleIdentifier()
            .map(|id| id.to_string());
        Ok(normalized(apps, own.as_deref()))
    })
}

pub struct Choice {
    receiver: Receiver<Result<Option<Application>>>,
    panel: Retained<NSOpenPanel>,
    finished: std::rc::Rc<std::cell::Cell<bool>>,
}
impl Choice {
    pub async fn receive(&self) -> Result<Option<Application>> {
        let result = self
            .receiver
            .recv()
            .await
            .context("The application chooser closed unexpectedly.");
        result?
    }
}
impl Drop for Choice {
    fn drop(&mut self) {
        if self.finished.get() {
            return;
        }
        // Cancels a pending sheet too when its originating pane/task disappears.
        self.panel.orderOut(None);
        // SAFETY: No sender is required for AppKit's standard cancel action.
        unsafe { self.panel.cancel(None) };
    }
}
pub fn choose(_: &gpui_kit::Window) -> Result<Choice> {
    let mtm = MainThreadMarker::new().context("The application chooser requires the UI thread.")?;
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setDirectoryURL(NSURL::from_directory_path("/Applications").as_deref());
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(false);
    panel.setAllowsOtherFileTypes(false);
    #[allow(deprecated)] // Matches the reference; supported on its deployment range.
    panel.setAllowedFileTypes(Some(&NSArray::from_retained_slice(&[NSString::from_str(
        "app",
    )])));
    let (sender, receiver) = bounded(1);
    let chosen = panel.clone();
    let finished = std::rc::Rc::new(std::cell::Cell::new(false));
    let completed = finished.clone();
    let callback = RcBlock::new(move |response| {
        completed.set(true);
        chosen.orderOut(None);
        let result = if response == NSModalResponseOK {
            chosen
                .URL()
                .and_then(|url| application(&url, None))
                .context("The selected application has no usable bundle identifier.")
                .map(Some)
        } else if response == NSModalResponseCancel {
            Ok(None)
        } else {
            Err(anyhow::anyhow!(
                "The application chooser could not be displayed."
            ))
        };
        let _ = sender.try_send(result);
    });
    if let Some(window) = NSApplication::sharedApplication(mtm).keyWindow() {
        panel.beginSheetModalForWindow_completionHandler(&window, &callback);
    } else {
        panel.beginWithCompletionHandler(&callback);
    }
    Ok(Choice {
        receiver,
        panel,
        finished,
    })
}

pub fn open(path: &Path, selected: Option<&Application>) -> Result<Receiver<Result<()>>> {
    ensure!(
        MainThreadMarker::new().is_some(),
        "Opening files requires the UI thread."
    );
    let file = NSURL::from_file_path(path).context("Invalid local file path.")?;
    let workspace = NSWorkspace::sharedWorkspace();
    let (sender, receiver) = bounded(1);
    let callback = RcBlock::new(move |_: *mut NSRunningApplication, error: *mut NSError| {
        let result = if error.is_null() {
            Ok(())
        } else {
            // SAFETY: AppKit owns this nullable NSError for the callback duration.
            Err(anyhow::anyhow!(
                unsafe { &*error }.localizedDescription().to_string()
            ))
        };
        let _ = sender.try_send(result);
    });
    let configuration = NSWorkspaceOpenConfiguration::new();
    if let Some(selected) = selected {
        let application = workspace
            .URLForApplicationWithBundleIdentifier(&NSString::from_str(&selected.identifier))
            .context("The selected application is no longer installed.")?;
        ensure!(
            application.to_file_path().as_ref() == Some(&selected.path),
            "The selected application moved or changed. Choose it again."
        );
        workspace.openURLs_withApplicationAtURL_configuration_completionHandler(
            &NSArray::from_retained_slice(&[file]),
            &application,
            &configuration,
            Some(&callback),
        );
    } else {
        workspace.openURL_configuration_completionHandler(&file, &configuration, Some(&callback));
    }
    Ok(receiver)
}

/// Read a small native app icon without launching the application or drawing UI.
pub(super) fn icon(path: &Path) -> Option<Vec<u8>> {
    use objc2::AnyThread;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace,
        NSGraphicsContext, NSImageInterpolation,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
    objc2::rc::autoreleasepool(|_| {
        let image = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path.to_str()?));
        // Render the NSImage, rather than exporting an arbitrary backing rep.
        // This is the reference's 32pt icon at 2x, resampled before the UI scales
        // it down to its 18pt slot. The bitmap/context belong to this worker.
        // SAFETY: null planes request AppKit-owned storage; the static color
        // space and explicit 64x64, 8-bit RGBA layout are valid for this bitmap.
        let bitmap = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(), std::ptr::null_mut(), 64, 64, 8, 4,
                true, false, NSDeviceRGBColorSpace, 0, 0,
            )
        }?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&bitmap)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        context.setImageInterpolation(NSImageInterpolation::High);
        image.drawInRect_fromRect_operation_fraction(
            NSRect::new(NSPoint::new(0., 0.), NSSize::new(64., 64.)),
            NSRect::new(NSPoint::new(0., 0.), image.size()),
            NSCompositingOperation::Copy,
            1.,
        );
        NSGraphicsContext::restoreGraphicsState_class();
        // SAFETY: The SDK requires a dictionary of representation properties;
        // an empty, immutable dictionary is valid for lossless PNG encoding.
        unsafe {
            bitmap.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }
        .map(|data| data.to_vec())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filename_type_lookup_does_not_require_a_downloaded_file_or_launch_an_app() {
        let content_type = preferred_type("txt").unwrap();
        assert!(!content_type.to_string().is_empty());
        let empty = preferred_type("nauterm-rs-synthetic-unknown-type");
        assert!(
            empty
                .as_ref()
                .is_none_or(|value| !value.to_string().is_empty())
        );
    }

    #[test]
    fn application_metadata_comes_from_a_synthetic_bundle_without_launching_it() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("独立 Editor.app");
        fs::create_dir_all(path.join("Contents"))?;
        fs::write(path.join("Contents/Info.plist"), br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>CFBundleIdentifier</key><string>com.example.nauterm.rs.synthetic-editor</string>
<key>CFBundleName</key><string>Synthetic Editor</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#)?;
        let url = NSURL::from_directory_path(&path).unwrap();
        let selected = application(&url, None).unwrap();
        assert_eq!(selected.name, "独立 Editor");
        assert_eq!(
            selected.identifier,
            "com.example.nauterm.rs.synthetic-editor"
        );
        assert_eq!(selected.path, path);
        assert!(!selected.is_default);
        assert!(application(&url, Some(&url)).unwrap().is_default);
        Ok(())
    }
}
