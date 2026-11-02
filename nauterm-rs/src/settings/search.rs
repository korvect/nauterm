//! Settings search indexes UI metadata only. It never reads resource/secret data.
use super::*;
use crate::components::ButtonCustomVariant;
use crate::typography::FontWeights;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Section {
    Appearance,
    Window,
    Behavior,
    Theme,
    Font,
    Cursor,
    Layout,
    Session,
    Input,
    Selection,
    Bell,
    Features,
    Recording,
    SftpWorkspace,
    Editors,
    Transfers,
    Files,
    Keyboard,
    Bindings,
    AiProviders,
    AiContext,
    Sync,
    About,
}
impl Section {
    const ALL: [Self; 23] = [
        Self::Appearance,
        Self::Window,
        Self::Behavior,
        Self::Theme,
        Self::Font,
        Self::Cursor,
        Self::Layout,
        Self::Session,
        Self::Input,
        Self::Selection,
        Self::Bell,
        Self::Features,
        Self::Recording,
        Self::SftpWorkspace,
        Self::Editors,
        Self::Transfers,
        Self::Files,
        Self::Keyboard,
        Self::Bindings,
        Self::AiProviders,
        Self::AiContext,
        Self::Sync,
        Self::About,
    ];
    pub fn page(self) -> Page {
        match self {
            Self::Appearance | Self::Window | Self::Behavior => Page::General,
            Self::SftpWorkspace | Self::Editors | Self::Transfers | Self::Files => Page::Sftp,
            Self::Keyboard | Self::Bindings => Page::Shortcuts,
            Self::AiProviders | Self::AiContext => Page::Ai,
            Self::Sync => Page::Sync,
            Self::About => Page::About,
            _ => Page::Terminal,
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Appearance => "APPEARANCE",
            Self::Window => "WINDOW",
            Self::Behavior => "BEHAVIOR",
            Self::Theme => "THEME",
            Self::Font => "FONT",
            Self::Cursor => "CURSOR",
            Self::Layout => "LAYOUT",
            Self::Session => "SESSION",
            Self::Input => "INPUT",
            Self::Selection => "SELECTION",
            Self::Bell => "BELL",
            Self::Features => "FEATURES",
            Self::Recording => "HISTORY & STORAGE",
            Self::SftpWorkspace => "WORKSPACE",
            Self::Editors => "EDITORS",
            Self::Transfers => "TRANSFERS",
            Self::Files => "FILES",
            Self::Keyboard => "KEYBOARD BEHAVIOR",
            Self::Bindings => "KEY BINDINGS",
            Self::AiProviders => "PROVIDERS",
            Self::AiContext => "TERMINAL CONTEXT",
            Self::Sync => "SYNC & BACKUP",
            Self::About => "ABOUT",
        }
    }
}

impl Page {
    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Terminal => "Terminal",
            Self::Sftp => "SFTP",
            Self::Ai => "AI Assistant",
            Self::Shortcuts => "Shortcuts",
            Self::Sync => "Sync & Backup",
            Self::About => "About",
        }
    }
}

struct Entry {
    section: Section,
    title: &'static str,
    description: &'static str,
    keywords: &'static str,
}
impl Entry {
    const fn new(
        section: Section,
        title: &'static str,
        description: &'static str,
        keywords: &'static str,
    ) -> Self {
        Self {
            section,
            title,
            description,
            keywords,
        }
    }
    fn matches(&self, terms: &[&str], language: crate::localization::Language) -> bool {
        use crate::localization::{Language, text_for};
        let page = self.section.page().label();
        let haystack = format!(
            "{} {} {} {} {} {} {}",
            text_for(Language::English, page),
            text_for(Language::English, self.title),
            text_for(Language::English, self.description),
            self.keywords,
            text_for(language, page),
            text_for(language, self.title),
            text_for(language, self.description),
        )
        .to_lowercase();
        terms.iter().all(|term| haystack.contains(term))
    }
}
use Section as S;
const ENTRIES: &[Entry] = &[
    Entry::new(
        S::Appearance,
        "settings.general.language.label",
        "settings.general.language.description",
        "language english chinese system locale 语言 英文 中文 系统",
    ),
    Entry::new(
        S::About,
        "rs.settings.pages.about.title",
        "rs.settings.search.about.description",
        "release source third party license update build version platform 关于 版本 更新 许可证 开源 构建 平台",
    ),
    Entry::new(
        S::Sync,
        "settings.pages.sync.title",
        "rs.settings.search.sync.description",
        "cloud github gist s3 webdav local restore backup merge encryption master key rotate forget automatic interval minutes schedule 同步 备份 恢复 云端 加密 密钥 清除 修改 自动 间隔 分钟 调度",
    ),
    Entry::new(
        S::AiProviders,
        "settings.label.aiProviders",
        "rs.settings.search.aiProviders.description",
        "openai anthropic google gemini ollama token temperature presets AI 提供商 模型 密钥 预设",
    ),
    Entry::new(
        S::AiContext,
        "settings.label.terminalContext",
        "settings.ai.terminalContext.summary",
        "ai assistant selected text history output 上下文 选区 输出",
    ),
    Entry::new(
        S::Appearance,
        "settings.general.applicationTheme.title",
        "settings.general.applicationTheme.summary",
        "appearance mode 主题 外观 深色 浅色",
    ),
    Entry::new(
        S::Appearance,
        "settings.general.hostIcon.label",
        "settings.general.hostIcon.summary",
        "operating system os badge 图标 主机",
    ),
    Entry::new(
        S::Window,
        "settings.label.windowSize",
        "settings.general.window.summary",
        "dimensions resize 窗口 宽度 高度",
    ),
    Entry::new(
        S::Behavior,
        "settings.general.workspacePage.label",
        "settings.general.workspacePage.summary",
        "tabs layout 工作区 概览",
    ),
    Entry::new(
        S::Behavior,
        "settings.general.workspaceRestore.label",
        "settings.general.workspaceRestore.summary",
        "ask always never recovery launch session 恢复 启动 工作区",
    ),
    Entry::new(
        S::Behavior,
        "settings.general.confirmOnClose.label",
        "settings.general.confirmOnClose.description",
        "quit exit confirmation 退出 关闭 确认",
    ),
    Entry::new(
        S::Theme,
        "settings.label.terminalTheme",
        "settings.terminal.theme.summary",
        "palette foreground background cursor selection ansi 主题 颜色 配色",
    ),
    Entry::new(
        S::Font,
        "settings.label.terminalFont",
        "rs.settings.search.terminalFont.description",
        "monospace normal bold typography 字体 字重 字号 行高 间距",
    ),
    Entry::new(
        S::Font,
        "settings.terminal.cjkFont.label",
        "rs.settings.terminal.cjkFont.description",
        "han glyph 中文 汉字 日文 韩文 字体",
    ),
    Entry::new(
        S::Font,
        "settings.terminal.ligatures.label",
        "settings.terminal.ligatures.description",
        "font -> => != 连字",
    ),
    Entry::new(
        S::Cursor,
        "common.label.cursor",
        "settings.terminal.cursor.summary",
        "block beam underline hollow caret 光标 闪烁",
    ),
    Entry::new(
        S::Layout,
        "settings.label.terminalPadding",
        "settings.terminal.padding.summary",
        "inset margin top right bottom left 内边距 留白",
    ),
    Entry::new(
        S::Layout,
        "rs.settings.search.scrollbar.title",
        "settings.terminal.scrollbar.description",
        "scroll history 滚动条",
    ),
    Entry::new(
        S::Session,
        "settings.terminal.shell.label",
        "settings.terminal.shell.description",
        "bash zsh fish command local 命令 解释器",
    ),
    Entry::new(
        S::Session,
        "settings.label.terminalEmulation",
        "rs.settings.search.emulation.description",
        "ghostty alacritty xterm vt 模拟 引擎",
    ),
    Entry::new(
        S::Session,
        "settings.terminal.scrollback.label",
        "rs.settings.terminal.scrollback.description",
        "buffer zero unlimited output 回滚 历史 行数",
    ),
    Entry::new(
        S::Session,
        "settings.terminal.sshKeepalive.label",
        "settings.terminal.sshKeepalive.description",
        "connection timeout ping 保活 心跳 间隔",
    ),
    Entry::new(
        S::Session,
        "settings.terminal.sshPrediction.label",
        "settings.terminal.sshPrediction.description",
        "adaptive always off latency prediction echo input 预测 回显 延迟 本地 自适应",
    ),
    Entry::new(
        S::Input,
        "settings.terminal.input.reportMouseEvents.label",
        "settings.terminal.input.reportMouseEvents.description",
        "click drag pointer vim wheel 鼠标 上报",
    ),
    Entry::new(
        S::Input,
        "settings.terminal.input.navigationKeysScroll.label",
        "rs.settings.search.navigationKeys.description",
        "paging keyboard interactive alternate 导航 翻页 滚动",
    ),
    Entry::new(
        S::Input,
        "rs.settings.terminal.openTarget.title",
        "rs.settings.terminal.openTarget.description",
        "ctrl command hyperlink url path 文件 链接 路径",
    ),
    Entry::new(
        S::Input,
        "rs.settings.terminal.clickMovesCursor.title",
        "rs.settings.terminal.clickMovesCursor.description",
        "alt mouse caret 光标 点击",
    ),
    Entry::new(
        S::Selection,
        "settings.terminal.copyOnSelect.label",
        "settings.terminal.copyOnSelect.summary",
        "clipboard mouse drag 复制 选中 剪贴板",
    ),
    Entry::new(
        S::Selection,
        "settings.terminal.selectCommandBlockOnClick.label",
        "settings.terminal.selectCommandBlockOnClick.description",
        "highlight dim output 命令块 高亮 选择",
    ),
    Entry::new(
        S::Selection,
        "settings.terminal.wordBoundaries.label",
        "rs.settings.terminal.wordBoundaries.description",
        "separator double click 双击 选词 单词 分隔符 边界",
    ),
    Entry::new(
        S::Bell,
        "settings.terminal.bell.sound.label",
        "settings.terminal.bell.sound.description",
        "bell notification alert audio 铃声 提醒 声音",
    ),
    Entry::new(
        S::Bell,
        "settings.terminal.bell.visual.label",
        "settings.terminal.bell.visual.description",
        "notification alert flash 铃声 提醒 闪烁",
    ),
    Entry::new(
        S::Bell,
        "settings.terminal.bell.tabIndicator.label",
        "settings.terminal.bell.tabIndicator.description",
        "notification alert indicator background 铃声 提醒 标签 后台 标记",
    ),
    Entry::new(
        S::Features,
        "settings.terminal.composer.label",
        "rs.settings.terminal.composer.description",
        "multiline command input editor 命令 输入 编辑器",
    ),
    Entry::new(
        S::Features,
        "settings.terminal.multipleTabs.label",
        "settings.terminal.multipleTabs.description",
        "multi tab split 标签 多标签 分屏",
    ),
    Entry::new(
        S::Recording,
        "rs.settings.search.recording.title",
        "rs.settings.search.recording.description",
        "capture replay logs 历史 录制 回放 日志",
    ),
    Entry::new(
        S::Recording,
        "rs.settings.search.recordingStorage.title",
        "rs.settings.search.recordingStorage.description",
        "location folder directory days quota cleanup 存储 目录 保留 清理",
    ),
    Entry::new(
        S::SftpWorkspace,
        "settings.sftp.workspaceTab.label",
        "settings.sftp.workspaceTab.summary",
        "file transfer 文件 传输 标签",
    ),
    Entry::new(
        S::Editors,
        "common.label.editors",
        "settings.sftp.editors.summary",
        "vim nano vscode application 编辑器 扩展名",
    ),
    Entry::new(
        S::Transfers,
        "settings.sftp.transfers.label",
        "rs.settings.search.transfers.description",
        "parallel threads upload download 并发 上传 下载 传输",
    ),
    Entry::new(
        S::Files,
        "settings.label.sftpFiles",
        "settings.sftp.files.summary",
        "dotfiles folder path 文件 隐藏 目录",
    ),
    Entry::new(
        S::Keyboard,
        "settings.shortcuts.optionAsMeta.label",
        "settings.shortcuts.optionAsMeta.description",
        "keyboard escape modifier option alt meta 键盘 修饰键",
    ),
    Entry::new(
        S::Bindings,
        "rs.settings.search.shortcuts.title",
        "rs.settings.search.shortcuts.description",
        "cmd ctrl copy paste select all search find quick connect edit duplicate ssh sftp close previous next split local tab 快捷键 复制 粘贴 全选 查找 连接 分屏",
    ),
];

fn matches(query: &str, language: crate::localization::Language) -> Vec<usize> {
    let query = query.to_lowercase();
    let terms: Vec<_> = query.split_whitespace().collect();
    ENTRIES
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| entry.matches(&terms, language).then_some(index))
        .collect()
}

fn move_result(active: Option<usize>, length: usize, previous: bool) -> Option<usize> {
    if length == 0 {
        return None;
    }
    Some(match active.filter(|index| *index < length) {
        None if previous => length - 1,
        None => 0,
        Some(index) if previous => (index + length - 1) % length,
        Some(index) => (index + 1) % length,
    })
}

pub(super) struct State {
    language: crate::localization::Language,
    pub input: Entity<InputState>,
    pub query: String,
    active: Option<usize>,
    matches: Vec<usize>,
    pub content: [ScrollHandle; Page::ALL.len()],
    results: ScrollHandle,
    anchors: Vec<ScrollAnchor>,
    pending: Rc<Cell<Option<Section>>>,
    owner: WeakEntity<Settings>,
    _events: Subscription,
    _keys: Subscription,
}
impl State {
    pub fn relocalize(&mut self, language: crate::localization::Language) {
        self.language = language;
        self.matches = matches(&self.query, language);
        self.active = None;
    }
    fn set_query(&mut self, query: &str) {
        if self.query == query.trim() {
            return;
        }
        self.query = query.trim().to_owned();
        self.matches = matches(&self.query, self.language);
        self.active = None;
        self.pending.set(None);
        self.results = ScrollHandle::default();
    }
    pub fn section(&self, section: Section, content: impl IntoElement) -> AnyElement {
        let anchor = self.anchors[section as usize].clone();
        let paint_anchor = anchor.clone();
        let pending = self.pending.clone();
        let owner = self.owner.clone();
        div()
            .w_full()
            .min_w_0()
            .on_children_prepainted(move |_, window, cx| {
                // Anchor positions must come from the newly selected page's
                // layout, rather than the previous search-results frame.
                if pending.get() == Some(section) {
                    pending.set(None);
                    paint_anchor.scroll_to(window, cx);
                    let owner = owner.clone();
                    window.on_next_frame(move |_, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            if this.page == section.page() && this.search.query.is_empty() {
                                cx.notify();
                            }
                        });
                    });
                }
            })
            .id(("settings-section", section as usize))
            .anchor_scroll(Some(anchor))
            .child(content)
            .into_any_element()
    }
}

impl Settings {
    pub(super) fn search_state(
        language: crate::localization::Language,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> State {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::localization::text_for(language, "Search settings"))
        });
        let events = cx.subscribe(&input, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.search.set_query(&input.read(cx).value());
                cx.notify();
            }
        });
        let target = input.clone();
        let owner = cx.weak_entity();
        let id = window.window_handle().window_id();
        let keys = cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle().window_id() != id
                || event.context_stack.iter().any(|context| {
                    context.contains("NautermOverlay")
                        || context.contains("NautermModal")
                        || context.contains("NautermMenu")
                })
            {
                return;
            }
            let focused = target.read(cx).focus_handle(cx).is_focused(window);
            if focused
                && target.update(cx, |input, cx| {
                    input.marked_text_range(window, cx).is_some()
                })
            {
                return;
            }
            if owner
                .update(cx, |this, cx| {
                    this.search_key(&event.keystroke, focused, window, cx)
                })
                .unwrap_or(false)
            {
                window.prevent_default();
                cx.stop_propagation();
            }
        });
        let content = std::array::from_fn(|_| ScrollHandle::default());
        let anchors = Section::ALL
            .into_iter()
            .map(|section| ScrollAnchor::for_handle(content[section.page() as usize].clone()))
            .collect();
        State {
            language,
            input,
            query: String::new(),
            active: None,
            matches: Vec::new(),
            content,
            results: ScrollHandle::default(),
            anchors,
            pending: Rc::new(Cell::new(None)),
            owner: cx.weak_entity(),
            _events: events,
            _keys: keys,
        }
    }

    pub(crate) fn select_page(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        if self.page == page && self.search.query.is_empty() {
            return;
        }
        self.shortcut_recording = None;
        self.clear_search(window, cx);
        self.page = page;
        self.page_generation = self.page_generation.wrapping_add(1);
        let generation = self.page_generation;
        self.search.content[page as usize].set_offset(Point::default());
        self.theme_gallery_ready = false;
        match page {
            Page::Terminal => {
                self.terminal_controls(window, cx);
                // Only gallery tiles wait for the destination's first paint.
                let owner = cx.weak_entity();
                window.on_next_frame(move |_, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.page == Page::Terminal && this.page_generation == generation {
                            this.theme_gallery_ready = true;
                            this.refresh_themes(cx);
                        }
                    });
                });
            }
            Page::Sftp => self.detect_editors(window, cx),
            Page::Ai => self.providers.update(cx, |view, cx| view.open(window, cx)),
            Page::Sync => self.sync.update(cx, |view, cx| view.open(cx)),
            _ => {}
        }
        cx.notify();
    }
    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.set_query("");
        self.search
            .input
            .update(cx, |input, cx| input.set_value("", window, cx));
    }
    fn open_search_result(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = ENTRIES.get(index) else {
            return;
        };
        self.select_page(entry.section.page(), window, cx);
        self.search.pending.set(Some(entry.section));
        self.focus.focus(window, cx);
    }
    fn search_key(
        &mut self,
        stroke: &Keystroke,
        focused: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.shortcut_recording.is_some() {
            return false;
        }
        let m = stroke.modifiers;
        let primary = if cfg!(target_os = "macos") {
            m.platform && !m.control
        } else {
            m.control && !m.platform
        };
        if primary && !m.alt && !m.shift && stroke.key == "f" {
            self.search.input.update(cx, |input, cx| {
                input.focus(window, cx);
                input.select_all(window, cx);
            });
            return true;
        }
        if m.platform || m.control || m.alt || m.shift {
            return false;
        }
        if stroke.key == "escape"
            && (!self.search.query.is_empty()
                || (focused && !self.search.input.read(cx).value().is_empty()))
        {
            self.clear_search(window, cx);
            self.search
                .input
                .update(cx, |input, cx| input.focus(window, cx));
            cx.notify();
            return true;
        }
        if !focused || self.search.query.is_empty() {
            return false;
        }
        match stroke.key.as_str() {
            "up" | "down" => {
                self.search.active = move_result(
                    self.search.active,
                    self.search.matches.len(),
                    stroke.key == "up",
                );
                if let Some(index) = self.search.active {
                    self.search.results.scroll_to_item(index + 2);
                }
                cx.notify();
            }
            "enter" => {
                if let Some(index) = self
                    .search
                    .matches
                    .get(self.search.active.unwrap_or(0))
                    .copied()
                {
                    self.open_search_result(index, window, cx);
                }
            }
            _ => return false,
        }
        true
    }

    pub(super) fn search_results(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::settings(self.values.dark);
        div()
            .id("settings-search-results")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .scrollbars_with(&self.search.results)
            .px(px(if compact { 20. } else { 26. }))
            .py(px(24.))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeights::SEMIBOLD)
                    .child(self.text("settings.label.searchSettings")),
            )
            .child(
                div()
                    .mt(px(3.))
                    .mb(px(22.))
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .child(if self.search.matches.is_empty() {
                        crate::localization::format_for(
                            self.values.general.language,
                            "rs.settings.search.noMatches",
                            &[("query", &self.search.query)],
                        )
                    } else {
                        crate::localization::format_for(
                            self.values.general.language,
                            "rs.settings.search.resultCount",
                            &[("count", &self.search.matches.len().to_string())],
                        )
                    }),
            )
            .children(
                self.search
                    .matches
                    .iter()
                    .enumerate()
                    .map(|(position, index)| {
                        let index = *index;
                        let entry = &ENTRIES[index];
                        Button::new(("settings-search-result", index))
                            .ghost()
                            .w_full()
                            .h_auto()
                            .min_h(px(66.))
                            .py(px(11.))
                            .text_size(px(13.))
                            .mb(px(8.))
                            .custom(ButtonCustomVariant::for_surface(p).color(p.sidebar.into()))
                            .when(self.search.active == Some(position), |view| {
                                view.bg(p.hover)
                            })
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .font_weight(FontWeights::MEDIUM)
                                                    .child(self.text(entry.title)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(p.muted)
                                                    .child(self.text(entry.description)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(p.accent)
                                            .child(self.text(entry.section.page().label())),
                                    )
                                    .child(
                                        Icon::new(IconName::ChevronRight)
                                            .size(px(14.))
                                            .text_color(p.accent),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_search_result(index, window, cx)
                            }))
                    }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn search_combines_terms_across_page_titles_descriptions_and_keywords() {
        let titles = |query| {
            matches(query, crate::localization::Language::English)
                .into_iter()
                .map(|index| {
                    crate::localization::text_for(
                        crate::localization::Language::English,
                        ENTRIES[index].title,
                    )
                    .to_string()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(titles(" terminal LiGAtuRes "), ["Ligatures"]);
        assert_eq!(titles("cjk 汉字"), ["CJK Font"]);
        assert_eq!(titles("sftp parallel"), ["Transfers"]);
        assert_eq!(titles("terminal sound"), ["Sound"]);
        assert_eq!(titles("铃声 闪烁"), ["Visual Bell"]);
        assert!(titles("font padding").is_empty());
        assert!(titles("no-such-setting-9281").is_empty());
        assert!(titles("settings.terminal").is_empty());
        let title = crate::localization::text_for(
            crate::localization::Language::Chinese,
            "settings.general.applicationTheme.title",
        );
        assert!(
            matches(&title, crate::localization::Language::Chinese)
                .iter()
                .any(|index| ENTRIES[*index].title == "settings.general.applicationTheme.title")
        );
    }

    #[test]
    fn every_search_entry_uses_catalog_keys_translated_in_both_languages() {
        use crate::localization::{Language, text_for};
        for entry in ENTRIES {
            for key in [entry.title, entry.description] {
                let english = text_for(Language::English, key);
                let chinese = text_for(Language::Chinese, key);
                assert_ne!(english, key, "missing English copy for {key}");
                assert_ne!(chinese, key, "missing Chinese copy for {key}");
            }
            assert_ne!(
                text_for(Language::Chinese, entry.description),
                text_for(Language::English, entry.description),
                "untranslated Chinese copy for {}",
                entry.description
            );
        }
    }

    #[test]
    fn every_search_section_has_an_index_entry_and_a_unique_anchor_slot() {
        for (index, section) in Section::ALL.into_iter().enumerate() {
            assert_eq!(section as usize, index);
            assert!((section.page() as usize) < Page::ALL.len());
            assert!(ENTRIES.iter().any(|entry| entry.section == section));
        }
        for entry in ENTRIES {
            assert!(Section::ALL.contains(&entry.section));
        }
    }

    #[test]
    fn keyboard_result_navigation_starts_unselected_and_handles_empty_or_changed_results() {
        assert_eq!(move_result(None, 0, false), None);
        assert_eq!(move_result(None, 3, false), Some(0));
        assert_eq!(move_result(None, 3, true), Some(2));
        assert_eq!(move_result(Some(0), 3, true), Some(2));
        assert_eq!(move_result(Some(2), 3, false), Some(0));
        assert_eq!(move_result(Some(10), 2, false), Some(0));
        assert_eq!(move_result(Some(0), 0, true), None);
    }
}
