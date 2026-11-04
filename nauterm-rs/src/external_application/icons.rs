//! Application icons are loaded with the association catalog on a worker.
//! Presentation only reads this cache; it never invokes AppKit or disk I/O.
use super::*;
use gpui_kit::Image;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

type Icons = HashMap<String, Option<Arc<Image>>>;
fn cache() -> &'static Mutex<Icons> {
    static ICONS: OnceLock<Mutex<Icons>> = OnceLock::new();
    ICONS.get_or_init(Mutex::default)
}
pub fn cached(editor: &Editor) -> Option<Arc<Image>> {
    cache().lock().unwrap().get(&editor.id()).cloned().flatten()
}
#[cfg(target_os = "windows")]
pub(super) fn remember(editor: &Editor, bytes: Option<Vec<u8>>) {
    let image = bytes.map(|bytes| Arc::new(Image::from_bytes(gpui_kit::ImageFormat::Png, bytes)));
    cache().lock().unwrap().insert(editor.id(), image);
}
pub fn preload(editor: &Editor) {
    let id = editor.id();
    if cache().lock().unwrap().contains_key(&id) {
        return;
    }
    #[cfg(target_os = "macos")]
    let image = {
        let path = match editor {
            Editor::Application { path, .. } => Some(path.clone()),
            Editor::Command { executable, .. } => resolve_executable(executable)
                .ok()
                .map(|path| path.canonicalize().unwrap_or(path)),
        };
        path.and_then(|path| {
            let bundle = path
                .ancestors()
                .find(|path| {
                    path.extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
                })
                .unwrap_or(&path);
            macos::icon(bundle)
        })
        .filter(|bytes| bytes.len() <= 2 * 1024 * 1024)
        .map(|bytes| Arc::new(Image::from_bytes(gpui_kit::ImageFormat::Png, bytes)))
    };
    #[cfg(target_os = "linux")]
    let image = match editor {
        Editor::Application { path, .. } => linux::icon(path),
        Editor::Command { executable, .. } => linux::command_icon(executable),
    }
    .map(|(format, bytes)| Arc::new(Image::from_bytes(format, bytes)));
    #[cfg(target_os = "windows")]
    let image = windows::icon(editor)
        .map(|bytes| Arc::new(Image::from_bytes(gpui_kit::ImageFormat::Png, bytes)));
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let image: Option<Arc<Image>> = None;
    cache().lock().unwrap().insert(id, image);
}
