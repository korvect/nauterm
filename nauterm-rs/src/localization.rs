//! Explicit UI-copy translation. Never apply this to terminal output, host names,
//! paths, commands, provider/model IDs or arbitrary editable/user-owned strings.
use gpui_kit::{App, Global, IntoElement, RenderOnce, SharedString, Window};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN", alias = "zh")]
    Chinese,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Locale {
    English,
    Chinese,
}
impl Language {
    pub const ALL: [Self; 3] = [Self::System, Self::English, Self::Chinese];
    pub fn from_config(value: Option<&str>) -> Self {
        match value {
            Some("en") => Self::English,
            Some("zh" | "zh-CN") => Self::Chinese,
            _ => Self::System,
        }
    }
    fn resolve(self, system: &str) -> Locale {
        match self {
            Self::English => Locale::English,
            Self::Chinese => Locale::Chinese,
            Self::System
                if system
                    .split(['-', '_', '.', '@'])
                    .next()
                    .is_some_and(|code| code.eq_ignore_ascii_case("zh")) =>
            {
                Locale::Chinese
            }
            Self::System => Locale::English,
        }
    }
    pub fn label(self, display: Self) -> SharedString {
        match self {
            Self::System => text_for(display, "settings.general.language.system"),
            Self::English => "English".into(),
            Self::Chinese => "简体中文".into(),
        }
    }
}
pub(crate) fn system_locale() -> &'static str {
    static LOCALE: OnceLock<String> = OnceLock::new();
    LOCALE.get_or_init(|| sys_locale::get_locale().unwrap_or_else(|| "en".into()))
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Active(Language);
impl Global for Active {}
pub fn install(language: Language, cx: &mut App) {
    if cx
        .try_global::<Active>()
        .is_none_or(|active| active.0 != language)
    {
        cx.set_global(Active(language));
        cx.refresh_windows();
    }
}
pub fn text(cx: &App, source: &str) -> SharedString {
    text_for(language(cx), source)
}
/// Resolve product copy when rendering so composed forms follow language changes.
/// Pass user-owned names, paths and terminal content directly to their text nodes.
#[derive(IntoElement)]
pub struct Text(TextSource);

enum TextSource {
    Label(SharedString),
    Message(SharedString),
    Translated(SharedString),
}
pub fn view(source: impl Into<SharedString>) -> Text {
    Text(TextSource::Label(source.into()))
}
pub fn message_view(source: impl Into<SharedString>) -> Text {
    Text(TextSource::Message(source.into()))
}

/// Display product copy that was already translated by the caller.
pub(crate) fn translated_view(text: SharedString) -> Text {
    Text(TextSource::Translated(text))
}
impl RenderOnce for Text {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let text = match self.0 {
            TextSource::Label(source) => label(cx, &source),
            TextSource::Message(source) => message(cx, &source),
            TextSource::Translated(text) => text,
        };
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            crate::typography::ui_label_text(text, _window)
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            text.into_any_element()
        }
    }
}
pub fn label(cx: &App, source: &str) -> SharedString {
    label_for(language(cx), source)
}
fn label_for(language: Language, source: &str) -> SharedString {
    let translated = text_for(language, source);
    if translated == source
        && let Some(caption) = source.strip_suffix(" *")
    {
        return format!("{} *", text_for(language, caption)).into();
    }
    translated
}
pub fn language(cx: &App) -> Language {
    cx.try_global::<Active>()
        .map_or(Language::System, |active| active.0)
}
pub fn text_for(language: Language, source: &str) -> SharedString {
    format_for(language, source, &[])
}
pub fn message(cx: &App, source: &str) -> SharedString {
    text_for(language(cx), source)
}
pub fn message_for(language: Language, source: &str) -> SharedString {
    text_for(language, source)
}
pub fn format_for(language: Language, key: &str, args: &[(&str, &str)]) -> SharedString {
    let template = catalog().lookup(language.resolve(system_locale()), key);
    let app_name = template.map(|_| crate::APPLICATION_NAME);
    interpolate(template.unwrap_or(key), args, app_name).into()
}
pub fn format(cx: &App, key: &str, args: &[(&str, &str)]) -> SharedString {
    format_for(language(cx), key, args)
}
fn interpolate(template: &str, args: &[(&str, &str)], app_name: Option<&str>) -> String {
    if args.is_empty() && (app_name.is_none() || !template.contains("{appName}")) {
        return template.to_owned();
    }
    let mut output = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}').map(|end| start + end) else {
            break;
        };
        output.push_str(&rest[..start]);
        let key = &rest[start + 1..end];
        if let Some((_, value)) = args.iter().find(|(name, _)| *name == key) {
            output.push_str(value);
        } else if key == "appName"
            && let Some(app_name) = app_name
        {
            output.push_str(app_name);
        } else {
            output.push_str(&rest[start..=end]);
        }
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}

struct Catalog {
    english: BTreeMap<String, String>,
    chinese: BTreeMap<String, String>,
}
fn messages(value: &Value) -> BTreeMap<String, String> {
    value
        .as_object()
        .expect("localization resource object")
        .iter()
        .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_owned())))
        .collect()
}
fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let english: Value = serde_json::from_str(include_str!("../../assets/i18n/en.json"))
            .expect("English localization resource");
        let chinese: Value = serde_json::from_str(include_str!("../../assets/i18n/zh_CN.json"))
            .expect("Chinese localization resource");
        Catalog {
            english: messages(&english),
            chinese: messages(&chinese),
        }
    })
}
impl Catalog {
    fn lookup(&self, locale: Locale, key: &str) -> Option<&str> {
        match locale {
            Locale::English => self.english.get(key),
            Locale::Chinese => self.chinese.get(key).or_else(|| self.english.get(key)),
        }
        .map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    #[test]
    fn localization_preferences_match_original_config_and_system_fallbacks() {
        for (config, expected) in [
            ("en", Language::English),
            ("zh", Language::Chinese),
            ("zh-CN", Language::Chinese),
            ("system", Language::System),
            ("future", Language::System),
        ] {
            assert_eq!(Language::from_config(Some(config)), expected);
        }
        for locale in ["zh-CN", "zh_TW.UTF-8", "ZH-hans", "zh@variant"] {
            assert_eq!(Language::System.resolve(locale), Locale::Chinese);
        }
        for locale in ["en_US", "de-DE", "", "zho"] {
            assert_eq!(Language::System.resolve(locale), Locale::English);
        }
        assert_eq!(Language::English.resolve("zh-CN"), Locale::English);
        assert_eq!(Language::Chinese.resolve("en-US"), Locale::Chinese);
        assert_eq!(serde_json::to_value(Language::Chinese).unwrap(), "zh-CN");
    }
    #[test]
    fn forwarding_types_use_the_shared_english_and_chinese_labels() {
        use crate::port_forward::Kind;
        for (kind, english, chinese) in [
            (Kind::Local, "Local", "本地"),
            (Kind::Remote, "Remote", "远程"),
            (Kind::Dynamic, "Dynamic", "动态"),
        ] {
            assert_eq!(text_for(Language::English, kind.label_key()), english);
            assert_eq!(text_for(Language::Chinese, kind.label_key()), chinese);
        }
    }

    #[test]
    fn localization_catalogs_resolve_keys_only() {
        assert_eq!(
            catalog().english.keys().collect::<Vec<_>>(),
            catalog().chinese.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            text_for(Language::English, "settings.general.language.system"),
            "System Default"
        );
        assert_eq!(
            text_for(Language::Chinese, "language.simplifiedChinese.autonym"),
            "简体中文"
        );
        assert_eq!(text_for(Language::Chinese, "HOSTS"), "HOSTS");
        assert_eq!(text_for(Language::English, "主机"), "主机");
        let english_command = "Delete \"prod.example\"? This cannot be undone.";
        assert_eq!(
            text_for(Language::Chinese, english_command),
            english_command
        );
    }

    #[test]
    fn formatted_copy_keeps_inserted_names_and_braces_literal() {
        assert_eq!(
            format_for(
                Language::Chinese,
                "workspace.contextMenu.host.openInWorkspace",
                &[("workspace", "My {workspace}")],
            ),
            "在 My {workspace} 工作区中打开"
        );
        assert_eq!(
            interpolate(
                "{query}: {count}",
                &[("query", "{count}"), ("count", "2")],
                None
            ),
            "{count}: 2"
        );
    }
    #[test]
    fn application_name_and_host_are_values_in_keyed_copy() {
        use super::{Language, format_for};
        assert_eq!(
            text_for(Language::Chinese, "workspace.restore.unexpectedExit.title"),
            "Nauterm RS 意外关闭"
        );
        assert_eq!(
            format_for(
                Language::English,
                "settings.general.defaultUrlHandler.isDefault",
                &[("scheme", "SSH")],
            ),
            "Nauterm RS is the default application for SSH."
        );
        assert_eq!(
            format_for(
                Language::Chinese,
                "workspace.knownHosts.authenticity.description",
                &[("host", "server{appName}")],
            ),
            "Nauterm RS 的 known_hosts 中尚无 server{appName} 的身份信息。你可以仅在本次连接中信任它，也可以保存指纹后重新连接。"
        );
        assert_eq!(
            text_for(Language::Chinese, "Diagnostic {appName}"),
            "Diagnostic {appName}"
        );
        assert_eq!(
            format_for(
                Language::English,
                "settings.pages.about.title",
                &[("appName", "Nauterm RS")]
            ),
            "About Nauterm RS"
        );
        assert_eq!(
            format_for(
                Language::Chinese,
                "settings.pages.about.title",
                &[("appName", "Nauterm")]
            ),
            "关于 Nauterm"
        );
        assert_eq!(
            format_for(
                Language::Chinese,
                "sftp.conflict.itemExists.description",
                &[("name", "{name}.txt")]
            ),
            "目标位置已存在名为“{name}.txt”的项目。"
        );
    }
    #[test]
    fn message_keys_translate_and_diagnostics_stay_verbatim() {
        assert_eq!(
            message_for(Language::Chinese, "common.label.failed"),
            "失败"
        );
        for source in [
            "Error: Failed\nFailed to export key: /tmp/{2}",
            "Could not recover /tmp/{2}: permission denied",
            "Key export stopped after creating /tmp/{2}. The remaining files were not published.",
            "18 days",
            "Black",
        ] {
            assert_eq!(message_for(Language::Chinese, source), source);
            assert_eq!(text_for(Language::Chinese, source), source);
        }
    }
    #[test]
    fn localization_keys_referenced_by_rs_sources_exist_and_are_translated() {
        fn visit(path: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(path).expect("source directory") {
                let path = entry.expect("source entry").path();
                if path.is_dir() {
                    visit(&path, files);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    files.push(path);
                }
            }
        }
        let mut files = Vec::new();
        visit(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let key = Regex::new(r#""((?:rs|settings|common|workspace|ai|sftp|language|connection|terminal)\.[A-Za-z0-9]+(?:\.[A-Za-z0-9]+)+)""#)
            .expect("key pattern");
        let catalog = catalog();
        let mut checked = 0;
        for file in files {
            let source = std::fs::read_to_string(&file).expect("source file");
            for captures in key.captures_iter(&source) {
                let key = &captures[1];
                assert!(
                    catalog.english.contains_key(key),
                    "{}: missing English copy for {key}",
                    file.display()
                );
                assert!(
                    catalog.chinese.contains_key(key),
                    "{}: missing Chinese copy for {key}",
                    file.display()
                );
                checked += 1;
            }
        }
        assert!(checked > 100);
        assert!(
            catalog.english.keys().all(|key| !key.starts_with("rs.")),
            "localization keys should use feature namespaces"
        );
    }
    #[test]
    fn static_control_labels_tooltips_and_placeholders_are_translated() {
        fn visit(path: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(path).expect("source directory") {
                let path = entry.expect("source entry").path();
                if path.is_dir() {
                    visit(&path, files);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    files.push(path);
                }
            }
        }
        let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        visit(&source_root, &mut files);
        let literal = Regex::new(
            r#"\.(?:label|tooltip|placeholder|child|title|description|localized_accessibility_label|aria_label)\(\s*"((?:[^"\\]|\\.)*)""#,
        )
        .expect("control copy pattern");
        let intentionally_untranslated = [
            "·",
            "…",
            "$",
            "$ ls -la",
            "FIDO2",
            "MIT + Commons Clause",
            "Mosh",
            "Nauterm RS",
            "PIN",
            "SFTP",
            "SSH",
            "Telnet",
            "txt, md, json, yaml, sh, py",
            "sel",
        ];
        let mut checked = 0;
        let mut missing = Vec::new();
        for file in files {
            let source = std::fs::read_to_string(&file).expect("source file");
            let source = source
                .split("#[cfg(test)]\nmod tests")
                .next()
                .expect("source before tests");
            for captures in literal.captures_iter(source) {
                let source = &captures[1];
                if intentionally_untranslated.contains(&source) {
                    continue;
                }
                let translated = catalog().lookup(Locale::Chinese, source).unwrap_or(source);
                if translated == source {
                    missing.push(format!("{}: untranslated {source:?}", file.display()));
                }
                checked += 1;
            }
        }
        assert!(checked > 100);
        assert!(missing.is_empty(), "{}", missing.join("\n"));
    }
    #[test]
    fn localization_required_field_markers_follow_the_translated_caption() {
        assert_eq!(
            label_for(Language::Chinese, "common.label.name *"),
            "名称 *"
        );
        assert_eq!(
            label_for(Language::English, "common.label.name *"),
            "Name *"
        );
        assert_eq!(
            label_for(Language::Chinese, "unknown caption *"),
            "unknown caption *"
        );
    }
}
