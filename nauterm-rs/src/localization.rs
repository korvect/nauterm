//! Explicit UI-copy translation. Never apply this to terminal output, host names,
//! paths, commands, provider/model IDs or arbitrary editable/user-owned strings.
use gpui_kit::{App, Global, IntoElement, RenderOnce, SharedString, Window};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    sync::OnceLock,
};

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
fn system_locale() -> &'static str {
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
        crate::app_menu::refresh_language(cx);
        cx.refresh_windows();
    }
}
pub fn text(cx: &App, source: &str) -> SharedString {
    text_for(language(cx), source)
}
/// Resolve product copy when rendering so composed forms follow language changes.
/// Pass user-owned names, paths and terminal content directly to their text nodes.
#[derive(IntoElement)]
pub struct Text {
    source: SharedString,
    is_message: bool,
}
pub fn view(source: impl Into<SharedString>) -> Text {
    Text {
        source: source.into(),
        is_message: false,
    }
}
pub fn message_view(source: impl Into<SharedString>) -> Text {
    Text {
        source: source.into(),
        is_message: true,
    }
}
impl RenderOnce for Text {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.is_message {
            message(cx, &self.source)
        } else {
            label(cx, &self.source)
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
    catalog()
        .translate(language.resolve(system_locale()), source)
        .into()
}
pub fn message(cx: &App, source: &str) -> SharedString {
    message_for(language(cx), source)
}
pub fn message_for(language: Language, source: &str) -> SharedString {
    let locale = language.resolve(system_locale());
    let catalog = catalog();
    let translated = catalog.translate(locale, source);
    if translated != source {
        return translated.into();
    }

    let mut output = String::with_capacity(source.len());
    let mut changed = false;
    for line in source.split_inclusive('\n') {
        let (content, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |content| (content, "\n"));
        let translated_line = catalog.translate(locale, content);
        if translated_line != content {
            output.push_str(&translated_line);
            changed = true;
        } else {
            for (index, part) in content.split(": ").enumerate() {
                if index > 0 {
                    output.push_str(": ");
                }
                let translated_part = catalog.translate(locale, part);
                changed |= translated_part != part;
                output.push_str(&translated_part);
            }
        }
        output.push_str(newline);
    }
    if changed { output } else { source.to_owned() }.into()
}
pub fn format_for(language: Language, source: &str, args: &[(&str, &str)]) -> SharedString {
    interpolate(&text_for(language, source), args).into()
}
fn interpolate(template: &str, args: &[(&str, &str)]) -> String {
    interpolate_with(template, |key| {
        args.iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| *value)
    })
}
fn interpolate_with<'a>(template: &str, value: impl Fn(&str) -> Option<&'a str>) -> String {
    let mut output = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}').map(|end| start + end) else {
            break;
        };
        output.push_str(&rest[..start]);
        let key = &rest[start + 1..end];
        if let Some(value) = value(key) {
            output.push_str(value);
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
    by_english: HashMap<String, String>,
    by_chinese: HashMap<String, String>,
    patterns: Vec<(Regex, String)>,
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
        let english = messages(&english);
        let mut by_english = HashMap::new();
        let mut by_chinese = HashMap::new();
        let patterns = chinese
            .get("_patterns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|entry| {
                (
                    Regex::new(entry["pattern"].as_str().expect("translation pattern"))
                        .expect("supported translation regex"),
                    entry["replacement"]
                        .as_str()
                        .expect("translation replacement")
                        .to_owned(),
                )
            })
            .collect();
        let chinese = messages(&chinese);
        for (key, source) in &english {
            if let Some(target) = chinese.get(key) {
                by_english
                    .entry(source.to_ascii_lowercase())
                    .or_insert_with(|| target.clone());
                by_chinese
                    .entry(target.clone())
                    .or_insert_with(|| source.clone());
            }
        }
        Catalog {
            english,
            chinese,
            by_english,
            by_chinese,
            patterns,
        }
    })
}
impl Catalog {
    fn translate(&self, locale: Locale, source: &str) -> String {
        if locale == Locale::English {
            return self
                .english
                .get(source)
                .or_else(|| self.by_chinese.get(source))
                .map_or(source, String::as_str)
                .to_owned();
        }
        if let Some(value) = self
            .chinese
            .get(source)
            .or_else(|| self.by_english.get(&source.to_ascii_lowercase()))
        {
            return value.clone();
        }
        for (pattern, replacement) in &self.patterns {
            if let Some(captures) = pattern.captures(source) {
                return interpolate_with(replacement, |key| {
                    key.parse::<usize>()
                        .ok()
                        .filter(|index| *index > 0 && *index < captures.len())
                        .map(|index| captures.get(index).map_or("", |capture| capture.as_str()))
                });
            }
        }
        self.english
            .get(source)
            .map_or(source, String::as_str)
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn localization_shared_catalogs_have_the_same_keys_and_usable_patterns() {
        let catalog = catalog();
        assert_eq!(
            catalog.english.keys().collect::<Vec<_>>(),
            catalog.chinese.keys().collect::<Vec<_>>()
        );
        assert!(!catalog.patterns.is_empty());
        assert_eq!(
            catalog.translate(Locale::English, "settings.general.language.system"),
            "System Default"
        );
        assert_eq!(
            catalog.translate(Locale::Chinese, "language.simplifiedChinese.autonym"),
            "简体中文"
        );
        assert_eq!(
            catalog.translate(Locale::Chinese, "HOSTS"),
            catalog.chinese["workspace.sidebar.hosts.label"]
        );
        assert_eq!(
            catalog.translate(Locale::Chinese, "unknown-static-copy"),
            "unknown-static-copy"
        );
        assert_eq!(
            catalog.translate(
                Locale::English,
                &catalog.translate(Locale::Chinese, "Search providers")
            ),
            "Search providers"
        );
    }
    #[test]
    fn localization_patterns_retain_dynamic_names_in_ui_sentences() {
        assert_eq!(
            catalog().translate(
                Locale::Chinese,
                "Delete \"prod.example\"? This cannot be undone."
            ),
            "删除“prod.example”？此操作无法撤销。"
        );
        assert_eq!(
            catalog().translate(Locale::Chinese, "Open in My custom Workspace"),
            "在 My custom 工作区中打开"
        );
        assert_eq!(
            catalog().translate(Locale::Chinese, "printf '%s' Host"),
            "printf '%s' Host"
        );
        assert_eq!(
            interpolate("{query}: {count}", &[("query", "{count}"), ("count", "2")]),
            "{count}: 2"
        );
    }
    #[test]
    fn localization_preserves_captured_braces_names_and_multiline_errors() {
        assert_eq!(
            text_for(
                Language::Chinese,
                "Key export stopped after creating /tmp/{2}. The remaining files were not published."
            ),
            "创建 /tmp/{2} 后密钥导出已停止，其余文件未发布。"
        );
        assert_eq!(
            text_for(
                Language::Chinese,
                "Could not recover /tmp/{2}: permission denied"
            ),
            "无法恢复 /tmp/{2}：permission denied"
        );
        assert_eq!(
            text_for(Language::Chinese, "settings.terminal.fontWeight.light"),
            "细体"
        );
        assert_eq!(
            text_for(Language::Chinese, "settings.general.applicationTheme.light"),
            "浅色"
        );
        assert_eq!(text_for(Language::Chinese, "Black"), "黑色");
        assert_eq!(
            text_for(Language::Chinese, "settings.terminal.fontWeight.black"),
            "极粗"
        );
        assert_eq!(text_for(Language::Chinese, "18 days"), "18 天");
    }
    #[test]
    fn dynamic_messages_translate_known_lines_and_context_fragments() {
        assert_eq!(
            message_for(
                Language::Chinese,
                "Delete \"prod.example\"? This cannot be undone."
            ),
            "删除“prod.example”？此操作无法撤销。"
        );
        assert_eq!(
            message_for(
                Language::Chinese,
                "provider request failed: Delete \"prod.example\"? This cannot be undone.\nunknown detail"
            ),
            "provider request failed: 删除“prod.example”？此操作无法撤销。\nunknown detail"
        );
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
        let key = Regex::new(r#""((?:rs|settings|common|workspace|ai|sftp|language)\.[A-Za-z0-9]+(?:\.[A-Za-z0-9]+)+)""#)
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
        for (key, english) in catalog.english.range("rs.".to_owned()..) {
            if !key.starts_with("rs.") {
                break;
            }
            assert_ne!(&catalog.chinese[key], english, "untranslated {key}");
        }
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
                let translated = catalog().translate(Locale::Chinese, source);
                assert_ne!(
                    translated,
                    source,
                    "{}: untranslated {source:?}",
                    file.display()
                );
                checked += 1;
            }
        }
        assert!(checked > 100);
    }
    #[test]
    fn localization_required_field_markers_follow_the_translated_caption() {
        assert_eq!(label_for(Language::Chinese, "Name *"), "名称 *");
        assert_eq!(label_for(Language::English, "Name *"), "Name *");
        assert_eq!(
            label_for(Language::Chinese, "unknown caption *"),
            "unknown caption *"
        );
    }
}
