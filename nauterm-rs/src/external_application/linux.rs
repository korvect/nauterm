//! GIO owns MIME associations and desktop-entry expansion. No shell command
//! parsing, GTK initialization or display-server connection is needed here.
use super::*;
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    os::unix::ffi::OsStrExt,
    ptr,
};
mod icons;
pub(super) use icons::load as icon;

#[repr(C)]
struct List {
    data: *mut c_void,
    next: *mut List,
    prev: *mut List,
}
#[repr(C)]
struct Error {
    domain: u32,
    code: c_int,
    message: *const c_char,
}
#[link(name = "gio-2.0")]
unsafe extern "C" {
    fn g_content_type_guess(
        name: *const c_char,
        data: *const u8,
        size: usize,
        uncertain: *mut c_int,
    ) -> *mut c_char;
    fn g_app_info_get_all_for_type(content_type: *const c_char) -> *mut List;
    fn g_app_info_get_all() -> *mut List;
    fn g_app_info_get_default_for_type(content_type: *const c_char, uris: c_int) -> *mut c_void;
    fn g_app_info_equal(first: *mut c_void, second: *mut c_void) -> c_int;
    fn g_app_info_get_display_name(app: *mut c_void) -> *const c_char;
    fn g_app_info_get_id(app: *mut c_void) -> *const c_char;
    fn g_app_info_get_executable(app: *mut c_void) -> *const c_char;
    fn g_desktop_app_info_get_type() -> usize;
    fn g_desktop_app_info_new_from_filename(filename: *const c_char) -> *mut c_void;
    fn g_desktop_app_info_get_filename(app: *mut c_void) -> *const c_char;
    fn g_desktop_app_info_get_string(app: *mut c_void, key: *const c_char) -> *mut c_char;
    fn g_file_new_for_path(path: *const c_char) -> *mut c_void;
    fn g_app_info_launch(
        app: *mut c_void,
        files: *mut List,
        context: *mut c_void,
        error: *mut *mut Error,
    ) -> c_int;
    fn g_settings_schema_source_get_default() -> *mut c_void;
    fn g_settings_schema_source_lookup(
        source: *mut c_void,
        name: *const c_char,
        recursive: c_int,
    ) -> *mut c_void;
    fn g_settings_schema_has_key(schema: *mut c_void, key: *const c_char) -> c_int;
    fn g_settings_schema_unref(schema: *mut c_void);
    fn g_settings_new_full(
        schema: *mut c_void,
        backend: *mut c_void,
        path: *const c_char,
    ) -> *mut c_void;
    fn g_settings_get_string(settings: *mut c_void, key: *const c_char) -> *mut c_char;
}
#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn g_object_unref(object: *mut c_void);
    fn g_type_check_instance_is_a(instance: *mut c_void, kind: usize) -> c_int;
}
#[link(name = "glib-2.0")]
unsafe extern "C" {
    fn g_free(memory: *mut c_void);
    fn g_list_free(list: *mut List);
    fn g_error_free(error: *mut Error);
}

struct Object(*mut c_void);
impl Drop for Object {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: All Object values own one GObject reference from a GIO
            // constructor; they stay on their creating worker thread.
            unsafe { g_object_unref(self.0) };
        }
    }
}
struct Apps(*mut List);
impl Drop for Apps {
    fn drop(&mut self) {
        // SAFETY: get_all_for_type transfers both the GList and each reference.
        unsafe {
            let mut item = self.0;
            while let Some(node) = item.as_ref() {
                g_object_unref(node.data);
                item = node.next;
            }
            g_list_free(self.0);
        }
    }
}
// All string pointers passed here come from GIO and are borrowed for this call.
unsafe fn text(value: *const c_char) -> Option<String> {
    if value.is_null() {
        None
    } else {
        // SAFETY: GIO guarantees a NUL-terminated string for every non-null value.
        Some(
            unsafe { CStr::from_ptr(value) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}
unsafe fn owned_text(value: *mut c_char) -> Option<String> {
    // SAFETY: The caller passes a nullable transfer-full GLib string.
    let result = unsafe { text(value) };
    unsafe { g_free(value.cast()) };
    result
}
fn filename(path: &Path) -> Result<CString> {
    Ok(CString::new(path.as_os_str().as_bytes())?)
}
fn desktop(path: &Path) -> Result<Object> {
    ensure!(
        path.is_absolute() && path.extension().is_some_and(|ext| ext == "desktop"),
        "Choose a desktop application launcher (.desktop)."
    );
    let name = filename(path)?;
    // SAFETY: Live C filename; constructor returns a nullable owned GObject.
    let object = Object(unsafe { g_desktop_app_info_new_from_filename(name.as_ptr()) });
    ensure!(
        !object.0.is_null(),
        "The application launcher is invalid or no longer installed."
    );
    Ok(object)
}
unsafe fn application(app: *mut c_void, default: *mut c_void) -> Option<Application> {
    // SAFETY: The caller retains this GAppInfo throughout this conversion. Only
    // cast desktop-backed instances before calling GDesktopAppInfo methods.
    unsafe {
        if g_type_check_instance_is_a(app, g_desktop_app_info_get_type()) == 0 {
            return None;
        }
        Some(Application {
            name: text(g_app_info_get_display_name(app))?,
            identifier: format!("desktop:{}", text(g_app_info_get_id(app))?),
            path: PathBuf::from(text(g_desktop_app_info_get_filename(app))?),
            is_default: !default.is_null() && g_app_info_equal(app, default) != 0,
        })
    }
}
// The caller retains the GList and its GAppInfo references throughout this call.
unsafe fn applications_from_list(mut item: *mut List, default: *mut c_void) -> Vec<Application> {
    let mut results = vec![];
    // SAFETY: The caller supplies a live borrowed GIO list; conversion copies
    // metadata before any of its references are released.
    unsafe {
        while let Some(node) = item.as_ref() {
            // NoDisplay only controls application menus. File handlers such as
            // Shotwell Viewer deliberately hide there but belong in Open With.
            if let Some(app) = application(node.data, default) {
                results.push(app);
            }
            item = node.next;
        }
    }
    normalized(results, Some("desktop:com.korvect.nauterm.rs.desktop"))
}

pub(super) fn list(path: &Path) -> Result<Vec<Application>> {
    let name = filename(path)?;
    // SAFETY: Filename-only MIME guessing doesn't read file contents. The MIME
    // string, list and default reference are released on every return path.
    unsafe {
        let mime = g_content_type_guess(name.as_ptr(), ptr::null(), 0, ptr::null_mut());
        if mime.is_null() {
            return Ok(vec![]);
        }
        let apps = Apps(g_app_info_get_all_for_type(mime));
        let default = Object(g_app_info_get_default_for_type(mime, 0));
        g_free(mime.cast());
        Ok(applications_from_list(apps.0, default.0))
    }
}
pub(super) fn from_path(path: &Path) -> Result<Application> {
    let app = desktop(path)?;
    // SAFETY: desktop() returns a live GDesktopAppInfo owned by app.
    unsafe { application(app.0, ptr::null_mut()) }
        .context("The application has no usable desktop identifier.")
}
pub(super) fn launch(path: &Path, application: &Path, identifier: &str) -> Result<()> {
    let app = desktop(application)?;
    // SAFETY: A live GDesktopAppInfo is kept until launch completes. A GFile is
    // passed as data; GIO, not a shell, expands the launcher's %f/%u fields.
    unsafe {
        let current =
            self::application(app.0, ptr::null_mut()).context("Invalid application launcher.")?;
        ensure!(
            current.identifier == identifier,
            "The selected application changed. Choose it again."
        );
        let path = filename(path)?;
        let file = Object(g_file_new_for_path(path.as_ptr()));
        let mut files = List {
            data: file.0,
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
        };
        let mut error = ptr::null_mut();
        let launched = g_app_info_launch(app.0, &mut files, ptr::null_mut(), &mut error);
        let detail = if let Some(error_ref) = error.as_ref() {
            let message = text(error_ref.message);
            g_error_free(error);
            message
        } else {
            None
        };
        ensure!(
            launched != 0,
            "{}",
            detail.unwrap_or_else(|| "Unable to start the selected application.".into())
        );
    }
    Ok(())
}
pub(super) fn icon_name(path: &Path) -> Option<String> {
    let app = desktop(path).ok()?;
    // SAFETY: This key lookup returns a nullable transfer-full GLib string.
    unsafe { owned_text(g_desktop_app_info_get_string(app.0, c"Icon".as_ptr())) }
}

// Saved command editors predate the desktop-application catalog. Look up their
// registered launcher for presentation only; never replace the saved command
// or use metadata returned by GIO as executable shell text.
pub(super) fn command_icon(executable: &Path) -> Option<(gpui_kit::ImageFormat, Vec<u8>)> {
    let executable = resolve_executable(executable).ok()?.canonicalize().ok()?;
    // SAFETY: This list owns all GAppInfo references until iteration ends.
    let apps = Apps(unsafe { g_app_info_get_all() });
    let mut item = apps.0;
    // SAFETY: Nodes and their GAppInfo references remain alive through apps.
    unsafe {
        while let Some(node) = item.as_ref() {
            if let Some(path) = launcher_for_executable(node.data, &executable)
                && let Some(image) = icon(&path)
            {
                return Some(image);
            }
            item = node.next;
        }
    }
    None
}

unsafe fn launcher_for_executable(app: *mut c_void, executable: &Path) -> Option<PathBuf> {
    // SAFETY: The caller retains a live GAppInfo; verify its concrete type
    // before accessing desktop-specific metadata. No process is started.
    unsafe {
        if g_type_check_instance_is_a(app, g_desktop_app_info_get_type()) == 0 {
            return None;
        }
        let name = g_app_info_get_executable(app);
        if name.is_null() {
            return None;
        }
        let name = std::ffi::OsStr::from_bytes(CStr::from_ptr(name).to_bytes());
        let candidate = resolve_executable(Path::new(name))
            .ok()?
            .canonicalize()
            .ok()?;
        same_installation(executable, &candidate)
            .then(|| text(g_desktop_app_info_get_filename(app)).map(PathBuf::from))
            .flatten()
    }
}

fn same_installation(command: &Path, application: &Path) -> bool {
    if command == application {
        return true;
    }
    // VS Code's public CLI is <installation>/bin/code, while its desktop
    // launcher runs <installation>/code. Both paths here are canonicalized.
    // Require that precise installation relationship, not just equal names
    // across unrelated directories. This affects icon lookup only.
    let Some(bin) = command
        .parent()
        .filter(|path| path.file_name().is_some_and(|name| name == "bin"))
    else {
        return false;
    };
    let Some((installation, name)) = bin.parent().zip(command.file_name()) else {
        return false;
    };
    installation
        .join(name)
        .canonicalize()
        .is_ok_and(|path| path == application)
}
fn icon_theme() -> Option<String> {
    // SAFETY: Probe the optional GNOME schema/key before constructing settings;
    // missing schemas must never make GSettings terminate a non-GNOME client.
    unsafe {
        let source = g_settings_schema_source_get_default();
        if source.is_null() {
            return None;
        }
        let schema =
            g_settings_schema_source_lookup(source, c"org.gnome.desktop.interface".as_ptr(), 1);
        if schema.is_null() {
            return None;
        }
        let settings = if g_settings_schema_has_key(schema, c"icon-theme".as_ptr()) != 0 {
            Some(Object(g_settings_new_full(
                schema,
                ptr::null_mut(),
                ptr::null(),
            )))
        } else {
            None
        };
        g_settings_schema_unref(schema);
        settings.and_then(|settings| {
            owned_text(g_settings_get_string(settings.0, c"icon-theme".as_ptr()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_wrapper_uses_its_own_installations_desktop_icon() -> Result<()> {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = tempfile::tempdir()?;
        let installation = temp.path().join("editor");
        fs::create_dir_all(installation.join("bin"))?;
        let cli = installation.join("bin/code");
        let binary = installation.join("code");
        for path in [&cli, &binary] {
            fs::write(path, "fixture; never executed")?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        }
        let link = temp.path().join("code");
        symlink(&cli, &link)?;
        let launcher = temp.path().join("fixture-code.desktop");
        fs::write(
            &launcher,
            format!(
                "[Desktop Entry]\nType=Application\nName=Visual Studio Code\nExec={} %F\nIcon=vscode\n",
                binary.display()
            ),
        )?;
        let app = desktop(&launcher)?;
        // SAFETY: app retains its GDesktopAppInfo throughout these lookups.
        unsafe {
            assert_eq!(
                launcher_for_executable(app.0, &link.canonicalize()?),
                Some(launcher)
            );
            assert_eq!(
                launcher_for_executable(app.0, &binary.canonicalize()?),
                Some(temp.path().join("fixture-code.desktop"))
            );
            let unrelated = temp.path().join("other/bin/code");
            fs::create_dir_all(unrelated.parent().unwrap())?;
            fs::write(&unrelated, "unrelated")?;
            assert_eq!(
                launcher_for_executable(app.0, &unrelated.canonicalize()?),
                None
            );
        }
        Ok(())
    }

    #[test]
    fn saved_command_icons_match_executables_instead_of_application_labels() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let launcher = temp.path().join("fixture.desktop");
        fs::write(
            &launcher,
            "[Desktop Entry]\nType=Application\nName=Different display name\nExec=/bin/true --fixture %f\nIcon=fixture-icon\n",
        )?;
        let app = desktop(&launcher)?;
        let matching = Path::new("/bin/true").canonicalize()?;
        let different = Path::new("/bin/false").canonicalize()?;
        // SAFETY: app owns the desktop GAppInfo through both metadata lookups.
        unsafe {
            assert_eq!(launcher_for_executable(app.0, &matching), Some(launcher));
            assert_eq!(launcher_for_executable(app.0, &different), None);
        }
        Ok(())
    }

    #[test]
    fn gio_launch_passes_literal_paths_and_uris_to_the_selected_application() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let directory = temp.path().join("launcher space 用户");
        fs::create_dir(&directory)?;
        let script = directory.join("capture.sh");
        let report = directory.join("arguments");
        fs::write(
            &script,
            "report=$1\nshift\nprintf '%s\\0' \"$@\" > \"$report.tmp\"\n/bin/mv -- \"$report.tmp\" \"$report\"\n",
        )?;
        // These are arguments to the fixture, never shell source. In particular
        // shell substitutions, field codes and a newline must survive intact.
        let file = directory.join("用户 & $(printf expanded) `printf expanded` %f ! ' \"\n.txt");
        fs::write(&file, "fixture")?;
        let uri = url::Url::from_file_path(&file).unwrap().to_string();
        for field in ["%f", "%F", "%u", "%U"] {
            let launcher = directory.join("fixture.desktop");
            fs::write(
                &launcher,
                format!(
                    "[Desktop Entry]\nType=Application\nName=Fixture\nExec=/bin/sh \"{}\" \"{}\" {field}\n",
                    script.display(),
                    report.display(),
                ),
            )?;
            let application = from_path(&launcher)?;
            launch(&file, &application.path, &application.identifier)?;
            // GIO launch is asynchronous. The fixture atomically publishes its
            // complete report; wait for this child, not an unrelated GUI app.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !report.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let received = fs::read(&report).with_context(|| format!("{field} launch report"))?;
            let literal_path = [file.as_os_str().as_bytes(), &[0]].concat();
            // The desktop-entry specification explicitly permits local paths
            // for %u/%U too. Accept only an exact path or its exact file URI.
            assert!(
                received == literal_path
                    || (field.eq_ignore_ascii_case("%u")
                        && received == [uri.as_bytes(), &[0]].concat()),
                "{field}: {received:?}"
            );
            fs::remove_file(&report)?;
        }
        Ok(())
    }

    #[test]
    fn missing_or_mismatched_launchers_fail_before_starting_a_process() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let launcher = temp.path().join("fixture.desktop");
        fs::write(
            &launcher,
            "[Desktop Entry]\nType=Application\nName=Fixture\nExec=/bin/true %f\n",
        )?;
        let application = from_path(&launcher)?;
        let file = temp.path().join("fixture.txt");
        fs::write(&file, "fixture")?;
        assert!(launch(&file, &launcher, "desktop:different.desktop").is_err());
        fs::remove_file(&launcher)?;
        assert!(launch(&file, &launcher, &application.identifier).is_err());
        Ok(())
    }

    #[test]
    fn associated_handlers_hidden_from_application_menus_remain_available() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("fixture-viewer.desktop");
        fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nName=Fixture Viewer\nExec=/bin/true %f\nNoDisplay=true\nMimeType=image/png;\n",
        )?;
        let object = desktop(&path)?;
        let mut node = List {
            data: object.0,
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
        };
        // SAFETY: Both the stack node and its owned GAppInfo remain live. No
        // system associations or environment variables are changed by this test.
        let entries = unsafe { applications_from_list(&mut node, object.0) };
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].identifier, "desktop:fixture-viewer.desktop");
        assert!(entries[0].is_default);
        Ok(())
    }

    #[test]
    fn desktop_catalog_entries_use_localized_metadata_and_keep_launcher_identity() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("fixture.desktop");
        fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nName=Fixture Editor\nExec=/bin/true %f\nIcon=fixture-editor\n",
        )?;
        let app = from_path(&path)?;
        assert_eq!(app.name, "Fixture Editor");
        assert_eq!(app.identifier, "desktop:fixture.desktop");
        assert_eq!(app.path, path);
        assert_eq!(icon_name(&path).as_deref(), Some("fixture-editor"));
        assert!(from_path(&temp.path().join("missing.desktop")).is_err());
        let invalid = temp.path().join("invalid.desktop");
        fs::write(
            &invalid,
            "[Desktop Entry]\nType=Link\nName=Not an app\nURL=https://example.invalid\n",
        )?;
        assert!(from_path(&invalid).is_err());
        Ok(())
    }
}
