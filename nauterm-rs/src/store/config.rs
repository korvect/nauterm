//! Preferences use Nauterm's versioned config format. Resource catalogs never
//! belong here. Patch supported leaves so settings from the other client and
//! future versions survive an RS save.
use crate::{
    editor_config::Editor,
    model::AppState,
    terminal_appearance::{Palette, ThemeId},
};
use anyhow::{Context, Result, ensure};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

pub struct Document(Value);

impl Document {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let value: Value = serde_json::from_slice(bytes)?;
        ensure!(value.is_object(), "Config must contain a JSON object");
        ensure!(
            value["schemaVersion"] == 1,
            "Unsupported config schema version"
        );
        Ok(Self(value))
    }

    pub fn new() -> Result<Self> {
        let defaults = if cfg!(target_os = "macos") {
            include_bytes!("../../../assets/config/default-macos.json") as &[u8]
        } else if cfg!(windows) {
            include_bytes!("../../../assets/config/default-windows.json") as &[u8]
        } else {
            include_bytes!("../../../assets/config/default-linux.json") as &[u8]
        };
        Self::parse(defaults)
    }

    fn read<T: DeserializeOwned>(&self, path: &str, target: &mut T) -> Result<()> {
        if let Some(value) = self.0.pointer(path).filter(|value| !value.is_null()) {
            *target = serde_json::from_value(value.clone())
                .with_context(|| format!("Invalid config value at {path}"))?;
        }
        Ok(())
    }

    pub fn apply(&self, state: &mut AppState) -> Result<()> {
        self.read(
            "/ai/includeTerminalSelection",
            &mut state.ai.include_selection,
        )?;
        self.read(
            "/ai/includeRecentTerminalOutput",
            &mut state.ai.include_recent_output,
        )?;
        self.read("/ai/presetsUrl", &mut state.ai.presets_url)?;
        state.general = self.general_preferences()?;
        self.read("/appearance/theme", &mut state.theme)?;
        self.read("/appearance/hostIcon", &mut state.host_icons)?;
        self.read("/shortcuts", &mut state.shortcuts)?;
        self.read("/sync/mergeStrategy", &mut state.sync.merge_strategy)?;
        self.read("/sync/backupCount", &mut state.sync.backup_count)?;
        self.read("/sync/automatic", &mut state.sync.automatic)?;
        self.read("/sync/interval", &mut state.sync.interval)?;
        state.sync.interval = state.sync.interval.clamp(
            crate::sync::Preferences::MIN_INTERVAL,
            crate::sync::Preferences::MAX_INTERVAL,
        );
        self.read("/workspace/restoreBehavior", &mut state.restore_behavior)?;
        let preferences = &mut state.terminal_preferences;
        self.read("/terminal/emulationEngine", &mut preferences.engine)?;
        self.read("/terminal/emulation", &mut preferences.emulation)?;
        if let Some(value) = self.0.pointer("/terminal/shellPath") {
            preferences.shell_path = serde_json::from_value::<Option<String>>(value.clone())
                .context("Invalid config value at /terminal/shellPath")?
                .filter(|path| !path.trim().is_empty());
        }
        let mut keepalive_millis = preferences.keepalive_millis();
        self.read("/ssh/keepaliveInterval", &mut keepalive_millis)?;
        preferences.set_keepalive_millis(keepalive_millis);
        let terminal = &mut state.terminal;
        if let Some(id) = self
            .0
            .pointer("/terminal/appearance/theme/id")
            .and_then(Value::as_str)
        {
            terminal.theme = if id == "default" {
                ThemeId::NysaLight
            } else {
                ThemeId::from_id(id).unwrap_or(ThemeId::NysaLight)
            };
        }
        if let Some(custom) = self
            .0
            .pointer("/terminal/appearance/theme/custom")
            .filter(|value| !value.is_null())
        {
            terminal.custom_palette = Some(read_palette(custom, terminal.palette())?);
        } else {
            terminal.custom_palette = None;
        }
        self.read("/terminal/appearance/font/family", &mut terminal.family)?;
        if let Some(value) = self.0.pointer("/terminal/appearance/font/cjkFamily") {
            terminal.cjk_family = serde_json::from_value::<Option<String>>(value.clone())
                .context("Invalid config value at /terminal/appearance/font/cjkFamily")?
                .map(|family| family.trim().to_owned())
                .filter(|family| !family.is_empty());
        }
        self.read("/terminal/appearance/font/size", &mut terminal.size)?;
        self.read(
            "/terminal/appearance/font/enableLigatures",
            &mut terminal.enable_ligatures,
        )?;
        self.read(
            "/terminal/appearance/font/lineHeight",
            &mut terminal.line_height,
        )?;
        self.read(
            "/terminal/appearance/font/letterSpacing",
            &mut terminal.letter_spacing,
        )?;
        self.read("/terminal/appearance/font/weight", &mut terminal.weight)?;
        self.read(
            "/terminal/appearance/font/boldWeight",
            &mut terminal.bold_weight,
        )?;
        self.read(
            "/terminal/appearance/cursor/blink",
            &mut terminal.cursor_blink,
        )?;
        self.read(
            "/terminal/appearance/cursor/shape",
            &mut terminal.cursor_shape,
        )?;
        self.read("/terminal/appearance/scrollbar", &mut terminal.scrollbar)?;
        self.read(
            "/terminal/appearance/padding/left",
            &mut terminal.padding.left,
        )?;
        self.read(
            "/terminal/appearance/padding/top",
            &mut terminal.padding.top,
        )?;
        self.read(
            "/terminal/appearance/padding/right",
            &mut terminal.padding.right,
        )?;
        self.read(
            "/terminal/appearance/padding/bottom",
            &mut terminal.padding.bottom,
        )?;
        self.read(
            "/terminal/behavior/scrollbackLines",
            &mut terminal.scrollback_lines,
        )?;
        self.read(
            "/terminal/behavior/copyOnSelect",
            &mut terminal.copy_on_select,
        )?;
        self.read("/terminal/behavior/bell/sound", &mut terminal.bell.sound)?;
        terminal.ssh_prediction = match self
            .0
            .pointer("/ssh/predictionMode")
            .and_then(Value::as_str)
        {
            Some("always") => crate::terminal_prediction::Mode::Always,
            Some("never") => crate::terminal_prediction::Mode::Never,
            _ => crate::terminal_prediction::Mode::Adaptive,
        };
        self.read("/terminal/behavior/bell/visual", &mut terminal.bell.visual)?;
        self.read(
            "/terminal/behavior/bell/tabIndicator",
            &mut terminal.bell.tab_indicator,
        )?;
        self.read(
            "/terminal/behavior/selectCommandBlockOnClick",
            &mut terminal.select_command_block_on_click,
        )?;
        self.read(
            "/terminal/behavior/wordBoundaries",
            &mut terminal.word_boundaries,
        )?;
        self.read(
            "/terminal/behavior/pointer/commandClickOpensFilenameOrUrl",
            &mut terminal.command_click_opens_filename_or_url,
        )?;
        self.read(
            "/terminal/behavior/pointer/optionClickMovesCursor",
            &mut terminal.option_click_moves_cursor,
        )?;
        self.read(
            "/terminal/input/reportMouseEvents",
            &mut terminal.report_mouse_events,
        )?;
        self.read(
            "/terminal/input/useOptionAsMetaKey",
            &mut terminal.use_option_as_meta_key,
        )?;
        self.read(
            "/terminal/input/navigationKeysScrollOutsideInteractiveApps",
            &mut terminal.navigation_keys_scroll,
        )?;
        self.read(
            "/terminal/features/autocomplete",
            &mut terminal.composer_autocomplete,
        )?;
        self.read(
            "/terminal/features/composer",
            &mut terminal.composer_enabled,
        )?;
        self.read("/terminal/features/multiTab", &mut terminal.multiple_tabs)?;
        self.read(
            "/rs/composer/maxSuggestions",
            &mut terminal.composer_max_suggestions,
        )?;
        self.read("/sftp/showTab", &mut state.sftp.show_tab)?;
        self.read("/sftp/showHiddenFiles", &mut state.sftp.show_hidden_files)?;
        self.read(
            "/sftp/concurrentTasks",
            &mut state.sftp.limits.concurrent_tasks,
        )?;
        self.read(
            "/sftp/transferThreads",
            &mut state.sftp.limits.transfer_threads,
        )?;
        self.read(
            "/rs/sftp/initialLocalDirectory",
            &mut state.sftp.initial_local_directory,
        )?;
        self.read("/sftp/sshEditor", &mut state.sftp_editors.ssh_editor)?;
        self.read(
            "/sftp/textFileExtensions",
            &mut state.sftp_editors.text_extensions,
        )?;
        if let Some(editor) = self
            .0
            .pointer("/rs/sftp/applicationEditor")
            .filter(|value| !value.is_null())
        {
            state.sftp_editors.external_editor = Some(serde_json::from_value(editor.clone())?);
        } else if let Some(editor) = self
            .0
            .pointer("/sftp/externalEditor")
            .filter(|value| !value.is_null())
        {
            let mut editor = editor.clone();
            let executable = editor["executable"]
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .context("Invalid external editor executable")?
                .trim()
                .to_owned();
            let object = editor.as_object_mut().context("Invalid external editor")?;
            object.insert("kind".into(), json!("command"));
            if object
                .get("label")
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                object.insert("label".into(), json!(executable));
            }
            object.entry("arguments").or_insert_with(|| json!([]));
            state.sftp_editors.external_editor = Some(serde_json::from_value(editor)?);
        } else {
            state.sftp_editors.external_editor = None;
        }
        self.read("/recording/enabled", &mut state.recordings.enabled)?;
        self.read("/recording/captureEnabled", &mut state.recordings.capture)?;
        self.read(
            "/recording/retentionDays",
            &mut state.recordings.retention_days,
        )?;
        self.read(
            "/recording/maxSessionBytes",
            &mut state.recordings.max_session_bytes,
        )?;
        self.read(
            "/recording/maxTotalBytes",
            &mut state.recordings.max_total_bytes,
        )?;
        state.terminal.validate()?;
        state.terminal_preferences.validate()?;
        state.sftp.validate()?;
        state.sftp_editors.validate()?;
        state.recordings.validate()?;
        Ok(())
    }

    pub fn general_preferences(&self) -> Result<crate::general_preferences::Preferences> {
        let mut preferences = crate::general_preferences::Preferences {
            language: crate::localization::Language::from_config(
                self.0
                    .pointer("/appearance/language")
                    .and_then(Value::as_str),
            ),
            ..Default::default()
        };
        self.read("/window/width", &mut preferences.window.width)?;
        self.read("/window/height", &mut preferences.window.height)?;
        self.read("/tabs/confirmOnClose", &mut preferences.confirm_on_close)?;
        self.read("/workspace/enabled", &mut preferences.workspace_enabled)?;
        preferences.window.width = preferences
            .window
            .width
            .max(crate::general_preferences::WindowSize::MIN_WIDTH);
        preferences.window.height = preferences
            .window
            .height
            .max(crate::general_preferences::WindowSize::MIN_HEIGHT);
        preferences.validate()?;
        Ok(preferences)
    }

    fn put(&mut self, path: &str, value: impl Serialize) -> Result<()> {
        let mut parts = path.trim_start_matches('/').split('/').peekable();
        let mut node = &mut self.0;
        while let Some(part) = parts.next() {
            if node.is_null() {
                *node = json!({});
            }
            let object = node.as_object_mut().context("Invalid config section")?;
            if parts.peek().is_none() {
                object.insert(part.into(), serde_json::to_value(value)?);
                break;
            }
            node = object.entry(part).or_insert_with(|| json!({}));
        }
        Ok(())
    }

    pub fn encode(mut self, state: &AppState) -> Result<Vec<u8>> {
        state.validate()?;
        self.put("/sync/mergeStrategy", state.sync.merge_strategy)?;
        self.put("/sync/backupCount", state.sync.backup_count)?;
        self.put("/sync/automatic", state.sync.automatic)?;
        self.put("/sync/interval", state.sync.interval)?;
        self.put("/ai/includeTerminalSelection", state.ai.include_selection)?;
        self.put(
            "/ai/includeRecentTerminalOutput",
            state.ai.include_recent_output,
        )?;
        self.put("/ai/presetsUrl", &state.ai.presets_url)?;
        self.put("/appearance/theme", state.app_theme())?;
        self.put("/shortcuts", &state.shortcuts)?;
        self.put("/window/width", state.general.window.width)?;
        self.put("/window/height", state.general.window.height)?;
        self.put("/tabs/confirmOnClose", state.general.confirm_on_close)?;
        self.put("/workspace/enabled", state.general.workspace_enabled)?;
        self.put("/workspace/restoreBehavior", state.restore_behavior)?;
        let t = &state.terminal;
        // Preserve unavailable theme IDs/custom metadata when an unrelated
        // setting is changed. A deliberate theme change replaces them.
        let mut previous = AppState::default();
        self.apply(&mut previous)?;
        if self
            .0
            .pointer("/appearance/language")
            .and_then(Value::as_str)
            .is_none_or(|value| matches!(value, "system" | "en" | "zh" | "zh-CN"))
            || previous.general.language != state.general.language
        {
            self.put("/appearance/language", state.general.language)?;
        }
        self.put(
            "/terminal/emulationEngine",
            state.terminal_preferences.engine,
        )?;
        self.put("/terminal/emulation", state.terminal_preferences.emulation)?;
        self.put(
            "/terminal/shellPath",
            &state.terminal_preferences.shell_path,
        )?;
        self.put(
            "/ssh/keepaliveInterval",
            state.terminal_preferences.keepalive_millis(),
        )?;
        let known_icon_mode = self
            .0
            .pointer("/appearance/hostIcon")
            .and_then(Value::as_str)
            .is_none_or(|mode| matches!(mode, "default" | "defaultIcon" | "osBadge" | "osIcon"));
        if known_icon_mode || previous.host_icons != state.host_icons {
            self.put("/appearance/hostIcon", state.host_icons)?;
        }
        if previous.terminal.theme != t.theme
            || previous.terminal.custom_palette != t.custom_palette
        {
            self.put("/terminal/appearance/theme/id", t.theme.id())?;
            self.put(
                "/terminal/appearance/theme/custom",
                t.custom_palette.map(write_palette),
            )?;
        }
        self.put("/terminal/appearance/font/family", &t.family)?;
        self.put("/terminal/appearance/font/cjkFamily", &t.cjk_family)?;
        self.put(
            "/terminal/appearance/font/enableLigatures",
            t.enable_ligatures,
        )?;
        self.put("/terminal/appearance/font/size", t.size)?;
        self.put("/terminal/appearance/font/lineHeight", t.line_height)?;
        self.put("/terminal/appearance/font/letterSpacing", t.letter_spacing)?;
        self.put("/terminal/appearance/font/weight", t.weight)?;
        self.put("/terminal/appearance/font/boldWeight", t.bold_weight)?;
        self.put("/terminal/appearance/cursor/blink", t.cursor_blink)?;
        self.put("/terminal/appearance/cursor/shape", t.cursor_shape)?;
        self.put("/terminal/appearance/scrollbar", t.scrollbar)?;
        self.put("/terminal/appearance/padding/left", t.padding.left)?;
        self.put("/terminal/appearance/padding/top", t.padding.top)?;
        self.put("/terminal/appearance/padding/right", t.padding.right)?;
        self.put("/terminal/appearance/padding/bottom", t.padding.bottom)?;
        self.put("/terminal/behavior/scrollbackLines", t.scrollback_lines)?;
        self.put("/terminal/features/composer", t.composer_enabled)?;
        self.put("/terminal/features/multiTab", t.multiple_tabs)?;
        self.put(
            "/terminal/input/useOptionAsMetaKey",
            t.use_option_as_meta_key,
        )?;
        self.put(
            "/terminal/input/navigationKeysScrollOutsideInteractiveApps",
            t.navigation_keys_scroll,
        )?;
        self.put("/terminal/behavior/copyOnSelect", t.copy_on_select)?;
        self.put("/terminal/behavior/bell/sound", t.bell.sound)?;
        if self
            .0
            .pointer("/ssh/predictionMode")
            .and_then(Value::as_str)
            .is_none_or(|mode| matches!(mode, "adaptive" | "always" | "never"))
            || previous.terminal.ssh_prediction != t.ssh_prediction
        {
            self.put("/ssh/predictionMode", t.ssh_prediction)?;
        }
        self.put("/terminal/behavior/bell/visual", t.bell.visual)?;
        self.put("/terminal/behavior/bell/tabIndicator", t.bell.tab_indicator)?;
        self.put(
            "/terminal/behavior/selectCommandBlockOnClick",
            t.select_command_block_on_click,
        )?;
        self.put("/terminal/behavior/wordBoundaries", &t.word_boundaries)?;
        self.put(
            "/terminal/behavior/pointer/commandClickOpensFilenameOrUrl",
            t.command_click_opens_filename_or_url,
        )?;
        self.put(
            "/terminal/behavior/pointer/optionClickMovesCursor",
            t.option_click_moves_cursor,
        )?;
        self.put("/terminal/input/reportMouseEvents", t.report_mouse_events)?;
        self.put("/terminal/features/autocomplete", t.composer_autocomplete)?;
        self.put("/rs/composer/maxSuggestions", t.composer_max_suggestions)?;
        self.put("/sftp/showTab", state.sftp.show_tab)?;
        self.put("/sftp/showHiddenFiles", state.sftp.show_hidden_files)?;
        self.put("/sftp/concurrentTasks", state.sftp.limits.concurrent_tasks)?;
        self.put("/sftp/transferThreads", state.sftp.limits.transfer_threads)?;
        self.put(
            "/rs/sftp/initialLocalDirectory",
            &state.sftp.initial_local_directory,
        )?;
        self.put("/sftp/sshEditor", &state.sftp_editors.ssh_editor)?;
        self.put(
            "/sftp/textFileExtensions",
            &state.sftp_editors.text_extensions,
        )?;
        let (editor, application) = match &state.sftp_editors.external_editor {
            Some(editor @ Editor::Application { path, label, .. }) => (
                json!({"label":label,"executable":"/usr/bin/open","arguments":["-W","-a",path,"--args"]}),
                serde_json::to_value(editor)?,
            ),
            Some(editor @ Editor::Command { .. }) => {
                let mut value = serde_json::to_value(editor)?;
                value.as_object_mut().unwrap().remove("kind");
                (value, Value::Null)
            }
            None => (Value::Null, Value::Null),
        };
        self.put("/sftp/externalEditor", editor)?;
        self.put("/rs/sftp/applicationEditor", application)?;
        self.put("/recording/enabled", state.recordings.enabled)?;
        self.put("/recording/captureEnabled", state.recordings.capture)?;
        self.put("/recording/retentionDays", state.recordings.retention_days)?;
        self.put(
            "/recording/maxSessionBytes",
            state.recordings.max_session_bytes,
        )?;
        self.put("/recording/maxTotalBytes", state.recordings.max_total_bytes)?;
        Ok(serde_json::to_vec_pretty(&self.0)?)
    }
}

const ANSI: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];
fn read_palette(value: &Value, mut palette: Palette) -> Result<Palette> {
    ensure!(value.is_object(), "Invalid custom terminal theme");
    let color = |name: &str, target: &mut u32| -> Result<()> {
        if let Some(value) = value.pointer(name).filter(|value| !value.is_null()) {
            let text = value
                .as_str()
                .context("Invalid theme color")?
                .trim_start_matches('#');
            ensure!(matches!(text.len(), 6 | 8), "Invalid theme color");
            *target = u32::from_str_radix(text, 16)? & 0xffffff;
        }
        Ok(())
    };
    color("/foreground", &mut palette.base.foreground)?;
    color("/background", &mut palette.base.background)?;
    color("/cursor", &mut palette.base.cursor)?;
    color("/accent", &mut palette.accent)?;
    color("/cursorText", &mut palette.cursor_text)?;
    color("/selectionBackground", &mut palette.selection)?;
    color("/selectionText", &mut palette.selection_text)?;
    for (index, name) in ANSI.iter().enumerate() {
        color(&format!("/normal/{name}"), &mut palette.ansi[index])?;
        color(&format!("/bright/{name}"), &mut palette.ansi[index + 8])?;
    }
    if let Some(kind) = value["type"].as_str() {
        ensure!(matches!(kind, "light" | "dark"), "Invalid theme type");
        palette.dark = kind == "dark";
    }
    palette.validate()?;
    Ok(palette)
}
fn write_palette(palette: Palette) -> Value {
    let hex = |value: u32| format!("#{value:06x}");
    let mut value = json!({
        "name":"Custom", "type": if palette.dark {"dark"} else {"light"},
        "foreground":hex(palette.base.foreground), "background":hex(palette.base.background),
        "cursor":hex(palette.base.cursor), "cursorText":hex(palette.cursor_text),
        "accent":hex(palette.accent), "selectionBackground":hex(palette.selection),
        "selectionText":hex(palette.selection_text), "normal":{}, "bright":{}
    });
    for (index, name) in ANSI.iter().enumerate() {
        value["normal"][name] = json!(hex(palette.ansi[index]));
        value["bright"][name] = json!(hex(palette.ansi[index + 8]));
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ssh_prediction_uses_original_config_and_preserves_future_modes() -> Result<()> {
        use crate::terminal_prediction::Mode;
        for (value, expected) in [
            ("adaptive", Mode::Adaptive),
            ("always", Mode::Always),
            ("never", Mode::Never),
            ("future", Mode::Adaptive),
        ] {
            let document = Document::parse(&serde_json::to_vec(
                &json!({"schemaVersion":1,"ssh":{"predictionMode":value,"future":42}}),
            )?)?;
            let mut state = AppState::default();
            document.apply(&mut state)?;
            assert_eq!(state.terminal.ssh_prediction, expected);
            let bytes = document.encode(&state)?;
            let encoded: Value = serde_json::from_slice(&bytes)?;
            assert_eq!(encoded["ssh"]["predictionMode"], value);
            assert_eq!(encoded["ssh"]["future"], 42);
            state.terminal.ssh_prediction = Mode::Always;
            let mut restored = AppState::default();
            Document::parse(&Document::parse(&bytes)?.encode(&state)?)?.apply(&mut restored)?;
            assert_eq!(restored.terminal.ssh_prediction, Mode::Always);
        }
        Ok(())
    }
    #[test]
    fn terminal_bell_roundtrip_preserves_reference_defaults_and_future_fields() -> Result<()> {
        let source = json!({"schemaVersion":1,"terminal":{"behavior":{"bell":{"visual":true,"tabIndicator":false,"future":42}}}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        document.apply(&mut state)?;
        assert!(state.terminal.bell.sound);
        assert!(state.terminal.bell.visual);
        assert!(!state.terminal.bell.tab_indicator);
        state.terminal.bell.sound = false;
        let bytes = document.encode(&state)?;
        let encoded: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(encoded["terminal"]["behavior"]["bell"]["future"], 42);
        assert_eq!(encoded["terminal"]["behavior"]["bell"]["sound"], false);
        let mut restored = AppState::default();
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert_eq!(restored.terminal.bell, state.terminal.bell);
        Ok(())
    }
    #[test]
    fn terminal_padding_preserves_fractional_edges_and_future_fields() -> Result<()> {
        let source = json!({"schemaVersion":1,"terminal":{"appearance":{"padding":{"left":1.5,"top":2.5,"right":3.5,"bottom":4.5,"future":42}}}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        document.apply(&mut state)?;
        assert_eq!(
            state.terminal.padding,
            crate::terminal_padding::Padding::parse("2.5, 3.5, 4.5, 1.5")?
        );
        state.terminal.padding.left = 9.25;
        let bytes = document.encode(&state)?;
        let encoded: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(encoded["terminal"]["appearance"]["padding"]["future"], 42);
        let mut restored = AppState::default();
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert_eq!(restored.terminal, state.terminal);
        let null_padding =
            Document::parse(br#"{"schemaVersion":1,"terminal":{"appearance":{"padding":null}}}"#)?;
        let mut state = AppState::default();
        null_padding.apply(&mut state)?;
        assert_eq!(state.terminal.padding, Default::default());
        let bytes = null_padding.encode(&state)?;
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes)?["terminal"]["appearance"]["padding"]["left"],
            0.
        );
        Ok(())
    }
    #[test]
    fn terminal_keyboard_preferences_roundtrip_in_original_input_section() -> Result<()> {
        let source = json!({"schemaVersion":1,"terminal":{"input":{"useOptionAsMetaKey":false,"navigationKeysScrollOutsideInteractiveApps":false,"future":42}}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        document.apply(&mut state)?;
        assert!(!state.terminal.use_option_as_meta_key);
        assert!(!state.terminal.navigation_keys_scroll);
        let bytes = document.encode(&state)?;
        let encoded: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(encoded["terminal"]["input"]["future"], 42);
        let mut restored = AppState::default();
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert_eq!(restored.terminal, state.terminal);
        Ok(())
    }
    #[test]
    fn workspace_and_terminal_feature_flags_roundtrip_without_exposing_autocomplete() -> Result<()>
    {
        let source = json!({"schemaVersion":1,"workspace":{"enabled":false,"future":42},"terminal":{"features":{"composer":false,"multiTab":true,"autocomplete":true,"future":7}}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        document.apply(&mut state)?;
        assert!(!state.general.workspace_enabled);
        assert!(!state.terminal.composer_enabled);
        assert!(state.terminal.multiple_tabs);
        assert!(!state.terminal.composer_autocomplete_enabled());
        state.terminal.multiple_tabs = false;
        let bytes = document.encode(&state)?;
        let encoded: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(encoded["workspace"]["future"], 42);
        assert_eq!(encoded["terminal"]["features"]["future"], 7);
        assert_eq!(encoded["terminal"]["features"]["autocomplete"], true);
        let mut restored = AppState::default();
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert_eq!(restored.general, state.general);
        assert_eq!(restored.terminal, state.terminal);
        Ok(())
    }
    #[test]
    fn general_window_and_close_preferences_roundtrip_without_replacing_other_window_fields()
    -> Result<()> {
        let source = json!({"schemaVersion":1,"window":{"width":1024.5,"height":768,"x":55,"y":66,"future":42},"tabs":{"confirmOnClose":false,"future":true}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let preferences = document.general_preferences()?;
        assert_eq!(preferences.window.width, 1024.5);
        assert_eq!(preferences.window.height, 768.);
        assert!(!preferences.confirm_on_close);
        let mut state = AppState::default();
        document.apply(&mut state)?;
        assert_eq!(state.general, preferences);
        assert_eq!(state.restore_behavior, crate::model::RestoreBehavior::Ask);
        state.general.window.width = 1280.;
        let bytes = document.encode(&state)?;
        let encoded: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(encoded["window"]["x"], 55);
        assert_eq!(encoded["window"]["y"], 66);
        assert_eq!(encoded["window"]["future"], 42);
        assert_eq!(encoded["tabs"]["future"], true);
        let mut restored = AppState::default();
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert_eq!(restored.general, state.general);
        Ok(())
    }
    #[test]
    fn configured_window_dimensions_respect_the_shared_minimum() -> Result<()> {
        let document =
            Document::parse(br#"{"schemaVersion":1,"window":{"width":100,"height":0}}"#)?;
        let preferences = document.general_preferences()?;
        assert_eq!(
            preferences.window,
            crate::general_preferences::WindowSize {
                width: 750.,
                height: 440.
            }
        );
        assert!(preferences.confirm_on_close);
        Ok(())
    }
    #[test]
    fn terminal_session_config_roundtrips_original_names_units_and_nullable_shell() -> Result<()> {
        use crate::terminal_preferences::{Emulation, Engine};
        let shell = if cfg!(windows) {
            r"C:\test\shell.exe"
        } else {
            "/test/shell"
        };
        let source = json!({"schemaVersion":1,"terminal":{"shellPath":shell,"emulationEngine":"alacritty","emulation":"xterm-16color","future":true},"ssh":{"keepaliveInterval":45_000,"predictionMode":"adaptive","future":42}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        document.apply(&mut state)?;
        assert_eq!(state.terminal_preferences.engine, Engine::Alacritty);
        assert_eq!(
            state.terminal_preferences.emulation,
            Emulation::Xterm16Color
        );
        assert_eq!(state.terminal_preferences.ssh_keepalive_seconds, 45);
        assert_eq!(
            state.terminal_preferences.shell_path.as_deref(),
            Some(shell)
        );
        // Remote VT options must not inherit the local executable path.
        assert!(state.terminal_preferences.options().shell_path.is_none());
        state.terminal_preferences.ssh_keepalive_seconds = 0;
        state.terminal_preferences.engine = Engine::Ghostty;
        state.terminal_preferences.emulation = Emulation::Xterm;
        state.terminal_preferences.shell_path = None;
        let bytes = document.encode(&state)?;
        let encoded: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(encoded["ssh"]["keepaliveInterval"], 0);
        assert_eq!(encoded["ssh"]["predictionMode"], "adaptive");
        assert_eq!(encoded["ssh"]["future"], 42);
        assert_eq!(encoded["terminal"]["future"], true);
        assert_eq!(encoded["terminal"]["emulation"], "xterm");
        assert!(encoded["terminal"]["shellPath"].is_null());
        let mut restored = AppState::default();
        restored.terminal_preferences.shell_path = Some(shell.into());
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert_eq!(restored.terminal_preferences, state.terminal_preferences);
        restored.terminal_preferences.ssh_keepalive_seconds = u32::MAX;
        let bytes = Document::parse(&bytes)?.encode(&restored)?;
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes)?["ssh"]["keepaliveInterval"],
            u64::from(u32::MAX) * 1000
        );
        Ok(())
    }
    #[test]
    fn platform_config_defaults_select_ghostty_xterm_and_twenty_second_keepalive() -> Result<()> {
        for bytes in [
            include_bytes!("../../../assets/config/default-macos.json") as &[u8],
            include_bytes!("../../../assets/config/default-windows.json"),
            include_bytes!("../../../assets/config/default-linux.json"),
        ] {
            let mut state = AppState::default();
            Document::parse(bytes)?.apply(&mut state)?;
            assert_eq!(
                state.terminal_preferences,
                crate::terminal_preferences::Preferences::default()
            );
            assert_eq!(
                state.general,
                crate::general_preferences::Preferences::default()
            );
        }
        Ok(())
    }
    #[test]
    fn terminal_runtime_preferences_roundtrip_in_reference_config_format() -> Result<()> {
        let source = json!({"schemaVersion":1,"terminal":{"appearance":{"cursor":{"shape":"hollowBlock","blink":false,"future":42},"scrollbar":false},"behavior":{"scrollbackLines":12345}}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        document.apply(&mut state)?;
        assert_eq!(
            state.terminal.cursor_shape,
            crate::terminal_appearance::CursorShape::HollowBlock
        );
        assert!(!state.terminal.cursor_blink && !state.terminal.scrollbar);
        assert_eq!(state.terminal.scrollback_lines, 12345);
        state.terminal.scrollback_lines = 0;
        state.terminal.cursor_shape = crate::terminal_appearance::CursorShape::Beam;
        let bytes = document.encode(&state)?;
        let encoded: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(encoded["terminal"]["appearance"]["cursor"]["future"], 42);
        let mut restored = AppState::default();
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert_eq!(restored.terminal, state.terminal);
        Ok(())
    }
    #[test]
    fn sync_config_roundtrips_scheduler_and_preserves_future_fields() -> Result<()> {
        let source = json!({"schemaVersion":1, "sync":{"mergeStrategy":"remote_wins", "backupCount":34,
            "automatic":true,"interval":900000,"future":{"retained":true}}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        document.apply(&mut state)?;
        assert_eq!(state.sync.merge_strategy, crate::sync::Strategy::RemoteWins);
        assert_eq!(state.sync.backup_count, 34);
        assert!(state.sync.automatic);
        assert_eq!(state.sync.interval, 900000);
        state.sync.backup_count = 0;
        state.sync.automatic = false;
        state.sync.set_minutes("37")?;
        let encoded: Value = serde_json::from_slice(&document.encode(&state)?)?;
        assert_eq!(encoded["sync"]["backupCount"], 0);
        assert_eq!(encoded["sync"]["automatic"], false);
        assert_eq!(encoded["sync"]["interval"], 37 * 60000);
        assert_eq!(encoded["sync"]["future"], source["sync"]["future"]);
        Ok(())
    }
    #[test]
    fn sync_intervals_from_original_config_are_clamped_before_runtime_use() -> Result<()> {
        for (source, expected) in [
            (1, crate::sync::Preferences::MIN_INTERVAL),
            (u64::MAX, crate::sync::Preferences::MAX_INTERVAL),
        ] {
            let value = json!({"schemaVersion":1,"sync":{"automatic":true,"interval":source}});
            let mut state = AppState::default();
            Document::parse(&serde_json::to_vec(&value)?)?.apply(&mut state)?;
            assert_eq!(state.sync.interval, expected);
            state.sync.validate()?;
        }
        Ok(())
    }
    #[test]
    fn shortcuts_roundtrip_disabled_and_custom_bindings_without_losing_future_fields() -> Result<()>
    {
        let source = json!({"schemaVersion":1,"shortcuts":{"quickConnect":"command+option+k","copy":"","tabSwitches":["ctrl+0"],"futureCommand":{"value":7}}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        document.apply(&mut state)?;
        assert_eq!(
            state.shortcuts.raw(crate::shortcuts::Command::QuickConnect),
            "cmd+alt+k"
        );
        assert_eq!(state.shortcuts.raw(crate::shortcuts::Command::Copy), "");
        state
            .shortcuts
            .set(crate::shortcuts::Command::NextTab, "cmd+alt+right");
        let bytes = document.encode(&state)?;
        let encoded: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            encoded["shortcuts"]["futureCommand"],
            source["shortcuts"]["futureCommand"]
        );
        let mut restored = AppState::default();
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert_eq!(restored.shortcuts, state.shortcuts);
        assert_eq!(
            restored.shortcuts.raw(crate::shortcuts::Command::Tab(1)),
            "cmd+2"
        );
        Ok(())
    }
    #[test]
    fn cjk_font_roundtrips_auto_and_uninstalled_names_without_losing_font_metadata() -> Result<()> {
        for value in [Value::Null, json!(""), json!(" Example CJK ")] {
            let source = json!({"schemaVersion":1,"terminal":{"appearance":{"font":{
                "cjkFamily":value,"futureFontOption":{"enabled":true}
            }}}});
            let document = Document::parse(&serde_json::to_vec(&source)?)?;
            let mut state = AppState::default();
            state.terminal.cjk_family = Some("Previous CJK".into());
            document.apply(&mut state)?;
            assert_eq!(
                state.terminal.cjk_family.as_deref(),
                value
                    .as_str()
                    .filter(|name| !name.is_empty())
                    .map(str::trim)
            );
            let bytes = document.encode(&state)?;
            let encoded: Value = serde_json::from_slice(&bytes)?;
            assert_eq!(
                encoded["terminal"]["appearance"]["font"]["futureFontOption"],
                source["terminal"]["appearance"]["font"]["futureFontOption"]
            );
            let mut restored = AppState::default();
            Document::parse(&bytes)?.apply(&mut restored)?;
            assert_eq!(restored.terminal.cjk_family, state.terminal.cjk_family);
        }
        for invalid in [json!(3), json!("bad\nfont")] {
            let source =
                json!({"schemaVersion":1,"terminal":{"appearance":{"font":{"cjkFamily":invalid}}}});
            let document = Document::parse(&serde_json::to_vec(&source)?)?;
            let mut state = AppState::default();
            assert!(
                document
                    .apply(&mut state)
                    .and_then(|_| state.validate())
                    .is_err()
            );
        }
        Ok(())
    }
    #[test]
    fn font_ligatures_use_the_original_config_leaf_and_can_be_disabled_again() -> Result<()> {
        let source = json!({"schemaVersion":1,"terminal":{"appearance":{"font":{
            "enableLigatures":true,"futureFontOption":{"level":2}
        }}}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        assert!(!state.terminal.enable_ligatures);
        document.apply(&mut state)?;
        assert!(state.terminal.enable_ligatures);
        state.terminal.enable_ligatures = false;
        let bytes = document.encode(&state)?;
        let encoded: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            encoded["terminal"]["appearance"]["font"]["enableLigatures"],
            false
        );
        assert_eq!(
            encoded["terminal"]["appearance"]["font"]["futureFontOption"]["level"],
            2
        );
        let mut restored = AppState::default();
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert!(!restored.terminal.enable_ligatures);
        Ok(())
    }
    #[test]
    fn host_icon_mode_roundtrips_and_preserves_a_future_mode_until_changed() -> Result<()> {
        let source =
            json!({"schemaVersion":1,"appearance":{"hostIcon":"futureMode","customLeaf":42}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        document.apply(&mut state)?;
        assert_eq!(state.host_icons, crate::avatar::Mode::OsBadge);
        let bytes = document.encode(&state)?;
        let preserved: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(preserved["appearance"]["hostIcon"], "futureMode");
        state.host_icons = crate::avatar::Mode::OsIcon;
        let bytes = Document::parse(&bytes)?.encode(&state)?;
        let mut restored = AppState::default();
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert_eq!(restored.host_icons, state.host_icons);
        let encoded: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(encoded["appearance"]["hostIcon"], "osIcon");
        assert_eq!(encoded["appearance"]["customLeaf"], 42);
        Ok(())
    }
    #[test]
    fn language_config_uses_the_original_leaf_and_preserves_future_values() -> Result<()> {
        use crate::localization::Language;
        for (source, expected, saved) in [
            ("en", Language::English, "en"),
            ("zh-CN", Language::Chinese, "zh-CN"),
            ("zh", Language::Chinese, "zh-CN"),
            ("system", Language::System, "system"),
            ("future-language", Language::System, "future-language"),
        ] {
            let value = json!({"schemaVersion":1,"appearance":{"language":source,"future":42}});
            let document = Document::parse(&serde_json::to_vec(&value)?)?;
            let mut state = AppState::default();
            document.apply(&mut state)?;
            assert_eq!(state.general.language, expected);
            let bytes = document.encode(&state)?;
            let encoded: Value = serde_json::from_slice(&bytes)?;
            assert_eq!(encoded["appearance"]["language"], saved);
            assert_eq!(encoded["appearance"]["future"], 42);
            state.general.language = Language::Chinese;
            let bytes = Document::parse(&bytes)?.encode(&state)?;
            assert_eq!(
                serde_json::from_slice::<Value>(&bytes)?["appearance"]["language"],
                "zh-CN"
            );
        }
        Ok(())
    }
    #[test]
    fn nested_config_round_trip_preserves_unimplemented_settings_and_excludes_resources()
    -> Result<()> {
        let source = json!({"schemaVersion":1,"appearance":{"language":"zh_CN"},"terminal":{"appearance":{"font":{"cjkFamily":"Test CJK"}},"behavior":{"scrollbackLines":23456}},"future":{"value":1}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        document.apply(&mut state)?;
        state.terminal.change_color(0, 0x123456)?;
        state.sftp.show_hidden_files = true;
        let bytes = document.encode(&state)?;
        let encoded: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            encoded["terminal"]["appearance"]["font"]["cjkFamily"],
            "Test CJK"
        );
        assert_eq!(encoded["terminal"]["behavior"]["scrollbackLines"], 23456);
        assert_eq!(encoded["future"], source["future"]);
        assert!(encoded.get("hosts").is_none() && encoded.get("workspaces").is_none());
        let mut restored = AppState::default();
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert_eq!(restored.terminal, state.terminal);
        assert_eq!(restored.sftp, state.sftp);
        Ok(())
    }
    #[test]
    fn ai_preferences_roundtrip_without_provider_credentials_or_catalogs() -> Result<()> {
        let source = json!({"schemaVersion":1,"ai":{"includeTerminalSelection":false,"includeRecentTerminalOutput":true,
            "presetsUrl":"https://example.invalid/presets.json","future":{"preserved":true}}});
        let document = Document::parse(&serde_json::to_vec(&source)?)?;
        let mut state = AppState::default();
        document.apply(&mut state)?;
        assert!(!state.ai.include_selection);
        assert_eq!(state.ai.presets_url, "https://example.invalid/presets.json");
        state.ai.include_recent_output = false;
        let bytes = document.encode(&state)?;
        let value: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(value["ai"]["future"], source["ai"]["future"]);
        assert!(value["ai"].get("apiKey").is_none());
        assert!(value["ai"].get("providers").is_none());
        let mut restored = AppState::default();
        Document::parse(&bytes)?.apply(&mut restored)?;
        assert_eq!(restored.ai, state.ai);
        Ok(())
    }
}
