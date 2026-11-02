//! Nauterm's portable shortcut format: `cmd` means the platform primary
//! modifier. Terminal editing adds Shift on Windows/Linux, preserving Ctrl-C.
use gpui_kit::{
    App, Global, InputPreference, KeybindingKeystroke, Keystroke, KeystrokeEvent, Modifiers,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, sync::LazyLock};

static DEFAULTS: LazyLock<Config> = LazyLock::new(|| {
    let source = if cfg!(target_os = "macos") {
        include_str!("../../assets/config/default-macos.json")
    } else if cfg!(windows) {
        include_str!("../../assets/config/default-windows.json")
    } else {
        include_str!("../../assets/config/default-linux.json")
    };
    let value: Value = serde_json::from_str(source).expect("valid bundled config");
    serde_json::from_value(value["shortcuts"].clone()).expect("valid bundled shortcuts")
});

/// Character input and the focused overlay own their keys before the workspace
/// or terminal. In particular, Ctrl+Alt used by AltGr is not an app shortcut.
pub fn accepts_event(event: &KeystrokeEvent) -> bool {
    event.input_preference == InputPreference::KeyBindings
        && !event.context_stack.iter().any(|context| {
            context.contains("NautermOverlay")
                || context.contains("NautermModal")
                || context.contains("NautermMenu")
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Workspace,
    Collection,
    Terminal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    QuickConnect,
    Palette,
    OpenSettings,
    Edit,
    Duplicate,
    Ssh,
    Sftp,
    PreviousTab,
    NextTab,
    CloseTab,
    Tab(u8),
    Copy,
    Paste,
    SelectAll,
    Search,
    SplitRight,
    SplitDown,
    NewLocalTerminal,
}
impl Command {
    pub const ALL: [Self; 26] = [
        Self::QuickConnect,
        Self::Palette,
        Self::OpenSettings,
        Self::Edit,
        Self::Duplicate,
        Self::Ssh,
        Self::Sftp,
        Self::PreviousTab,
        Self::NextTab,
        Self::CloseTab,
        Self::Tab(0),
        Self::Tab(1),
        Self::Tab(2),
        Self::Tab(3),
        Self::Tab(4),
        Self::Tab(5),
        Self::Tab(6),
        Self::Tab(7),
        Self::Tab(8),
        Self::Copy,
        Self::Paste,
        Self::SelectAll,
        Self::Search,
        Self::SplitRight,
        Self::SplitDown,
        Self::NewLocalTerminal,
    ];
    pub fn scope(self) -> Scope {
        match self {
            Self::Edit | Self::Duplicate => Scope::Collection,
            Self::Copy | Self::Paste | Self::SelectAll | Self::Search => Scope::Terminal,
            _ => Scope::Workspace,
        }
    }
    pub fn label(self) -> String {
        match self {
            Self::QuickConnect => "Quick Connect",
            Self::Palette => "Command Palette",
            Self::OpenSettings => "Open Settings",
            Self::Edit => "Edit Selected Item",
            Self::Duplicate => "Duplicate Selected Item",
            Self::Ssh => "Switch to SSH",
            Self::Sftp => "Switch to SFTP",
            Self::PreviousTab => "Previous Tab",
            Self::NextTab => "Next Tab",
            Self::CloseTab => "Close Tab",
            Self::Tab(index) => return format!("Select Tab {}", index + 1),
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
            Self::Search => "Search",
            Self::SplitRight => "Split Right",
            Self::SplitDown => "Split Down",
            Self::NewLocalTerminal => "New Local Terminal",
        }
        .into()
    }
    fn key(self) -> &'static str {
        match self {
            Self::QuickConnect => "quickConnect",
            Self::Palette => "commandPalette",
            Self::OpenSettings => "openSettings",
            Self::Edit => "editWorkspaceItem",
            Self::Duplicate => "duplicateWorkspaceItem",
            Self::Ssh => "switchToSsh",
            Self::Sftp => "switchToSftp",
            Self::PreviousTab => "previousTab",
            Self::NextTab => "nextTab",
            Self::CloseTab => "closeTab",
            Self::Tab(_) => "tabSwitches",
            Self::Copy => "copy",
            Self::Paste => "paste",
            Self::SelectAll => "selectAll",
            Self::Search => "search",
            Self::SplitRight => "splitRight",
            Self::SplitDown => "splitDown",
            Self::NewLocalTerminal => "newLocalTerminal",
        }
    }
    pub fn default_binding(self) -> String {
        if let Some(value) = DEFAULTS.value(self).and_then(Value::as_str) {
            return normalize(value);
        }
        match self {
            Self::QuickConnect => "cmd+t",
            Self::Palette => "cmd+shift+p",
            Self::OpenSettings => "cmd+,",
            Self::Edit => "cmd+e",
            Self::Duplicate => "cmd+d",
            Self::Ssh => "cmd+shift+t",
            Self::Sftp => "cmd+shift+s",
            Self::PreviousTab => "cmd+shift+[",
            Self::NextTab => "cmd+shift+]",
            Self::CloseTab => "cmd+w",
            Self::Tab(index) => return format!("cmd+{}", index + 1),
            Self::Copy => "cmd+c",
            Self::Paste => "cmd+v",
            Self::SelectAll => "cmd+a",
            Self::Search => "cmd+f",
            Self::SplitRight => "cmd+alt+right",
            Self::SplitDown => "cmd+alt+down",
            Self::NewLocalTerminal => "cmd+l",
        }
        .into()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Config(BTreeMap<String, Value>);
impl Global for Config {}
impl Config {
    fn value(&self, command: Command) -> Option<&Value> {
        let value = self.0.get(command.key());
        match command {
            Command::Tab(index) => value
                .and_then(Value::as_array)
                .and_then(|values| values.get(index as usize)),
            _ => value,
        }
    }
    pub fn raw(&self, command: Command) -> String {
        normalize(
            self.value(command)
                .and_then(Value::as_str)
                .unwrap_or(&command.default_binding()),
        )
    }
    pub fn set(&mut self, command: Command, binding: &str) {
        let binding = Value::String(normalize(binding));
        if let Command::Tab(index) = command {
            let mut values: Vec<_> = (0..9)
                .map(|index| Value::String(self.raw(Command::Tab(index))))
                .collect();
            if let Some(value) = values.get_mut(index as usize) {
                *value = binding;
            }
            if let Some(extra) = self
                .0
                .get(command.key())
                .and_then(Value::as_array)
                .and_then(|values| values.get(9..))
            {
                values.extend_from_slice(extra);
            }
            self.0.insert(command.key().into(), Value::Array(values));
        } else {
            self.0.insert(command.key().into(), binding);
        }
    }
    pub fn reset(&mut self) {
        for command in Command::ALL {
            self.set(command, &command.default_binding());
        }
    }
    pub fn matching(&self, scope: Scope, stroke: &Keystroke) -> Option<Command> {
        self.matching_on(scope, stroke, cfg!(target_os = "macos"))
    }
    fn matching_on(&self, scope: Scope, stroke: &Keystroke, macos: bool) -> Option<Command> {
        Command::ALL
            .into_iter()
            .filter(|command| command.scope() == scope)
            .find(|command| {
                Binding::parse(&self.raw(*command))
                    .is_some_and(|binding| binding.matches(stroke, macos, scope == Scope::Terminal))
            })
    }
    pub fn label(&self, command: Command) -> String {
        Binding::parse(&self.raw(command))
            .map(|binding| {
                binding.label(
                    cfg!(target_os = "macos"),
                    command.scope() == Scope::Terminal,
                )
            })
            .unwrap_or_else(|| {
                if self.raw(command).is_empty() {
                    "Disabled".into()
                } else {
                    "Unsupported shortcut".into()
                }
            })
    }
    pub fn menu(
        &self,
        item: crate::components::menu::MenuItem,
        command: Command,
    ) -> crate::components::menu::MenuItem {
        match Binding::parse(&self.raw(command)) {
            Some(binding) => {
                let key = KeybindingKeystroke::from_keystroke(binding.stroke(
                    cfg!(target_os = "macos"),
                    command.scope() == Scope::Terminal,
                ));
                item.accelerator(key)
            }
            None => item,
        }
    }
}
pub fn current(cx: &App) -> Config {
    cx.try_global::<Config>().cloned().unwrap_or_default()
}

pub fn normalize(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .replace("command", "cmd")
        .replace("control", "cmd")
        .replace("ctrl", "cmd")
        .replace("option", "alt")
        .split('+')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("+")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    key: String,
    alt: bool,
    shift: bool,
}
impl Binding {
    pub fn parse(value: &str) -> Option<Self> {
        let normalized = normalize(value);
        let parts: Vec<_> = normalized.split('+').collect();
        let (key, modifiers) = parts.split_last()?;
        if !modifiers
            .iter()
            .all(|modifier| matches!(*modifier, "cmd" | "alt" | "shift"))
        {
            return None;
        }
        Some(Self {
            key: key_name(key)?,
            alt: modifiers.contains(&"alt"),
            shift: modifiers.contains(&"shift"),
        })
    }
    pub fn record(stroke: &Keystroke, terminal: bool) -> Option<String> {
        let macos = cfg!(target_os = "macos");
        if stroke.modifiers.platform != macos || stroke.modifiers.control == macos {
            return None;
        }
        if terminal && !macos && !stroke.modifiers.shift {
            return None;
        }
        let key = key_name(&stroke.key)?;
        let mut parts = vec!["cmd".into()];
        if stroke.modifiers.alt {
            parts.push("alt".into());
        }
        if stroke.modifiers.shift && (macos || !terminal) {
            parts.push("shift".into());
        }
        parts.push(key);
        Some(parts.join("+"))
    }
    pub(crate) fn stroke(&self, macos: bool, terminal: bool) -> Keystroke {
        Keystroke {
            key: self.key.clone(),
            key_char: None,
            modifiers: Modifiers {
                platform: macos,
                control: !macos,
                alt: self.alt,
                shift: self.shift || (!macos && terminal),
                ..Default::default()
            },
        }
    }
    fn matches(&self, stroke: &Keystroke, macos: bool, terminal: bool) -> bool {
        let expected = self.stroke(macos, terminal);
        let mut modifiers = stroke.modifiers;
        modifiers.function = false;
        modifiers == expected.modifiers
            && key_name(&stroke.key).as_deref() == Some(self.key.as_str())
    }
    fn label(&self, macos: bool, terminal: bool) -> String {
        let stroke = self.stroke(macos, terminal);
        let mut parts = vec![if macos { "Cmd".into() } else { "Ctrl".into() }];
        if stroke.modifiers.alt {
            parts.push(if macos { "Option".into() } else { "Alt".into() });
        }
        if stroke.modifiers.shift {
            parts.push("Shift".into());
        }
        parts.push(match self.key.as_str() {
            "left" => "←".into(),
            "right" => "→".into(),
            "up" => "↑".into(),
            "down" => "↓".into(),
            "pageup" => "Page Up".into(),
            "pagedown" => "Page Down".into(),
            _ => self.key.to_uppercase(),
        });
        parts.join(" + ")
    }
}
fn key_name(key: &str) -> Option<String> {
    let lower = key.to_lowercase();
    let key = match lower.as_str() {
        "{" => "[",
        "}" => "]",
        "+" => "=",
        "_" => "-",
        ":" => ";",
        "\"" => "'",
        "<" => ",",
        ">" => ".",
        "?" => "/",
        "|" => "\\",
        "~" => "`",
        "!" => "1",
        "@" => "2",
        "#" => "3",
        "$" => "4",
        "%" => "5",
        "^" => "6",
        "&" => "7",
        "*" => "8",
        "(" => "9",
        ")" => "0",
        value => value,
    };
    (key.len() == 1 && (key.as_bytes()[0].is_ascii_alphanumeric() || "[]/\\;'.,-=`".contains(key))
        || matches!(
            key,
            "space"
                | "enter"
                | "tab"
                | "backspace"
                | "delete"
                | "left"
                | "right"
                | "up"
                | "down"
                | "home"
                | "end"
                | "pageup"
                | "pagedown"
        )
        || key
            .strip_prefix('f')
            .and_then(|number| number.parse::<u8>().ok())
            .is_some_and(|number| (1..=12).contains(&number)))
    .then(|| key.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_input_and_overlay_keys_never_enter_workspace_or_terminal_dispatch() {
        let mut event = KeystrokeEvent {
            keystroke: Keystroke::parse("ctrl-alt-t").unwrap(),
            input_preference: InputPreference::CharacterInput,
            action: None,
            context_stack: vec![gpui_kit::KeyContext::parse("TerminalView").unwrap()],
        };
        assert!(!accepts_event(&event));
        event.input_preference = InputPreference::KeyBindings;
        assert!(accepts_event(&event));
        for overlay in ["NautermOverlay", "NautermModal", "NautermMenu"] {
            event
                .context_stack
                .push(gpui_kit::KeyContext::parse(overlay).unwrap());
            assert!(!accepts_event(&event), "{overlay} owns the key");
            event.context_stack.pop();
        }
        event
            .context_stack
            .push(gpui_kit::KeyContext::parse("Input").unwrap());
        assert!(
            accepts_event(&event),
            "configured editing still reaches the composer"
        );
    }

    #[test]
    fn platform_defaults_and_reset_share_the_bundled_config() {
        let mut config = Config::default();
        config.set(Command::CloseTab, "");
        config.set(Command::SplitRight, "cmd+alt+k");
        config.reset();
        for command in Command::ALL {
            if let Some(value) = DEFAULTS.value(command).and_then(Value::as_str) {
                assert_eq!(config.raw(command), normalize(value), "{command:?}");
            }
        }
        if !cfg!(target_os = "macos") {
            assert_eq!(config.raw(Command::CloseTab), "cmd+shift+w");
            assert_eq!(config.raw(Command::SplitRight), "cmd+shift+e");
            assert_eq!(config.raw(Command::SplitDown), "cmd+shift+o");
        }
    }

    #[test]
    fn bundled_bindings_dispatch_on_their_platform_without_conflicts() -> anyhow::Result<()> {
        for (source, macos) in [
            (include_str!("../../assets/config/default-macos.json"), true),
            (
                include_str!("../../assets/config/default-windows.json"),
                false,
            ),
            (
                include_str!("../../assets/config/default-linux.json"),
                false,
            ),
        ] {
            let value: Value = serde_json::from_str(source)?;
            let config: Config = serde_json::from_value(value["shortcuts"].clone())?;
            for command in Command::ALL {
                let binding = Binding::parse(&config.raw(command)).unwrap();
                let stroke = binding.stroke(macos, command.scope() == Scope::Terminal);
                assert_eq!(
                    config.matching_on(command.scope(), &stroke, macos),
                    Some(command),
                    "{command:?}, macos={macos}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn every_default_maps_to_its_command_on_both_platform_modifier_schemes() {
        let config = Config::default();
        for macos in [true, false] {
            for command in Command::ALL {
                let binding = Binding::parse(&config.raw(command)).unwrap();
                let stroke = binding.stroke(macos, command.scope() == Scope::Terminal);
                assert_eq!(
                    config.matching_on(command.scope(), &stroke, macos),
                    Some(command),
                    "{command:?}, macos={macos}"
                );
            }
        }
        let interrupt = Keystroke::parse("ctrl-c").unwrap();
        assert_eq!(config.matching_on(Scope::Terminal, &interrupt, false), None);
        let shell_edit = Keystroke::parse("ctrl-e").unwrap();
        assert_eq!(
            config.matching_on(Scope::Workspace, &shell_edit, false),
            None
        );
        assert_eq!(
            config.matching_on(Scope::Terminal, &shell_edit, false),
            None
        );
    }

    #[test]
    fn aliases_disabled_commands_and_shifted_keys_use_one_matching_rule() -> anyhow::Result<()> {
        let mut config: Config = serde_json::from_value(serde_json::json!({
            "quickConnect":" Control + Option + Shift + K ", "closeTab":"", "futureAction":{"preserved":true},
            "tabSwitches":["command+0", ""]
        }))?;
        assert_eq!(config.raw(Command::QuickConnect), "cmd+alt+shift+k");
        assert_eq!(config.raw(Command::CloseTab), "");
        assert_eq!(config.raw(Command::Tab(2)), "cmd+3");
        assert_eq!(
            config.matching_on(Scope::Workspace, &Keystroke::parse("cmd-w")?, true),
            None
        );
        let mut previous = Keystroke::parse("cmd-shift-[")?;
        previous.key = "{".into();
        assert_eq!(
            config.matching_on(Scope::Workspace, &previous, true),
            Some(Command::PreviousTab)
        );
        config.set(Command::QuickConnect, "cmd+shift+!");
        let mut first = Keystroke::parse("cmd-shift-1")?;
        first.key = "!".into();
        assert_eq!(
            config.matching_on(Scope::Workspace, &first, true),
            Some(Command::QuickConnect)
        );
        config.set(Command::Tab(8), "");
        config.reset();
        let value = serde_json::to_value(&config)?;
        assert_eq!(value["futureAction"]["preserved"], true);
        assert_eq!(value["tabSwitches"].as_array().unwrap().len(), 9);
        for command in Command::ALL {
            assert_eq!(config.raw(command), command.default_binding());
        }
        Ok(())
    }

    #[test]
    fn recorder_requires_the_primary_modifier_and_stores_portable_terminal_bindings() {
        let os = if cfg!(target_os = "macos") {
            gpui_kit::OperatingSystem::Mac
        } else if cfg!(target_os = "windows") {
            gpui_kit::OperatingSystem::Windows
        } else {
            gpui_kit::OperatingSystem::Linux
        };
        let primary = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };
        assert!(Binding::record(&Keystroke::parse("k").unwrap(), false).is_none());
        assert_eq!(
            Binding::record(
                &Keystroke::parse_with_os(&format!("{primary}-alt-shift-k"), os).unwrap(),
                false
            )
            .as_deref(),
            Some("cmd+alt+shift+k")
        );
        let recorded = Binding::record(
            &Keystroke::parse_with_os(&format!("{primary}-shift-v"), os).unwrap(),
            true,
        )
        .unwrap();
        assert_eq!(
            recorded,
            if cfg!(target_os = "macos") {
                "cmd+shift+v"
            } else {
                "cmd+v"
            }
        );
        if !cfg!(target_os = "macos") {
            assert!(Binding::record(&Keystroke::parse("ctrl-v").unwrap(), true).is_none());
        }
        assert!(Binding::parse("cmd+unsupported-key").is_none());
    }
}
