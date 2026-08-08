// ignore_for_file: invalid_use_of_internal_member, implementation_imports

import 'dart:async';
import 'dart:io';
import 'dart:isolate';
import 'dart:math' as math;
import 'dart:ui' show BoxHeightStyle, BoxWidthStyle;

import 'package:flutter/foundation.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart' show ScrollCacheExtent;
import 'package:flutter/services.dart';
import 'package:flutter/src/widgets/_window.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:package_info_plus/package_info_plus.dart';

import '../ai/ai_config.dart';
import '../ai/ai_preset_store.dart';
import '../ai/ai_provider_preset.dart';
import '../app/nauterm_localizations.dart';
import '../app/nauterm_log.dart';
import '../app/nauterm_theme.dart';
import '../data/ai_provider_store.dart';
import '../data/nauterm_config_store.dart';
import '../data/nauterm_config.dart';
import '../data/nauterm_data_store.dart';
import '../data/nauterm_paths.dart';
import '../data/terminal_theme_store.dart';
import '../terminal/terminal_config.dart';
import '../terminal/external_editor_catalog.dart';
import '../terminal/terminal_models.dart';
import '../terminal/system_font_catalog.dart';
import '../terminal/system_shells.dart';
import '../terminal/terminal_theme.dart';
import '../ui/nauterm_overlay.dart';
import '../ui/nauterm_typography.dart';
import '../ui/terminal_theme_preview.dart';
import '../update/desktop_update.dart';
import '../update/macos_sparkle_updater.dart';
import '../app/window_config.dart';
import '../window/native_windowing.dart';
import 'github_device_flow.dart';
import 'cloud_oauth.dart';

part 'settings_panel_pages.dart';
part 'settings_panel_controls.dart';
part 'settings_panel_theme.dart';
part 'settings_panel_shortcuts.dart';
part 'settings_panel_ai.dart';
part 'settings_panel_sync.dart';
part 'settings_panel_cloud_sync.dart';
part 'settings_panel_update.dart';

final ValueNotifier<int> nautermDatabaseBulkChangeRevision = ValueNotifier(0);
final ValueNotifier<int> nautermSyncPreferencesRevision = ValueNotifier(0);
final ValueNotifier<int> nautermSyncStatusRevision = ValueNotifier(0);

@visibleForTesting
Future<void> Function(Directory directory)? settingsDirectoryOpenerOverride;

void notifyNautermDatabaseBulkChange() {
  nautermDatabaseBulkChangeRevision.value++;
}

void notifyNautermSyncPreferencesChanged() {
  nautermSyncPreferencesRevision.value++;
}

void notifyNautermSyncCompleted() {
  _cachedGithubSyncDatabasePath = null;
  _cachedGithubSyncSettings = null;
  nautermSyncStatusRevision.value++;
}

bool get _settingsDark {
  return switch (appThemeMode) {
    AppThemeMode.dark => true,
    AppThemeMode.light => false,
    AppThemeMode.system =>
      WidgetsBinding.instance.platformDispatcher.platformBrightness ==
          Brightness.dark,
  };
}

Color get _primary =>
    _settingsDark ? const Color(0xff4da6ff) : const Color(0xff2563eb);
Color get _secondary =>
    _settingsDark ? const Color(0xff35d394) : const Color(0xff10b981);
Color get _surface => _settingsDark ? const Color(0xff1a2028) : Colors.white;
Color get _surfaceContainer =>
    _settingsDark ? const Color(0xff202832) : const Color(0xfff9fafb);
Color get _text =>
    _settingsDark ? const Color(0xffedf3f7) : const Color(0xff111827);
Color get _mutedText =>
    _settingsDark ? const Color(0xffa4b3bd) : const Color(0xff4b5563);
Color get _faintText =>
    _settingsDark ? const Color(0xff73838f) : const Color(0xff9ca3af);
Color get _softOutline =>
    _settingsDark ? const Color(0xff303a46) : const Color(0xffe5e7eb);
Color get _settingsFieldHover =>
    _settingsDark ? const Color(0xff232c37) : const Color(0xfff8fafc);
const double _settingsSelectHeight = 34;
const double _settingsFieldFontSize = 14;
const double _settingsFieldTextHeight = 18;
const int _settingsSelectMaximumVisibleRows = 6;
const double _settingsCompactSidebarBreakpoint = 720;
const double _settingsStackedRowBreakpoint = 480;
const double _settingsStackedFieldsBreakpoint = 520;
const double _settingsContentMaxWidth = 1000;
const EdgeInsets _settingsSelectPadding = EdgeInsets.only(left: 10, right: 8);
const EdgeInsets _settingsInputPadding = EdgeInsets.symmetric(horizontal: 10);

BoxDecoration _settingsFieldDecoration({
  required bool focused,
  required bool hovered,
}) {
  return BoxDecoration(
    color: hovered && !focused ? _settingsFieldHover : _surface,
    borderRadius: BorderRadius.circular(7),
    border: Border.all(color: focused ? _primary : _softOutline, width: 1),
    boxShadow: focused
        ? const [
            BoxShadow(color: Color(0x1a2563eb), blurRadius: 0, spreadRadius: 2),
          ]
        : null,
  );
}

enum _SettingsPage { general, terminal, sftp, ai, sync, shortcuts, about }

_SettingsPage? _takeRequestedSettingsPage() =>
    switch (takeRequestedSettingsPage()) {
      NautermSettingsPage.terminal => _SettingsPage.terminal,
      NautermSettingsPage.about => _SettingsPage.about,
      null => null,
    };

class _SettingsSearchEntry {
  const _SettingsSearchEntry({
    required this.page,
    required this.section,
    required this.title,
    required this.subtitle,
    this.keywords = '',
  });

  final _SettingsPage page;
  final String? section;
  final String title;
  final String subtitle;
  final String keywords;

  bool matches(String query) {
    final haystack = '${page.name} $title $subtitle $keywords'.toLowerCase();
    return query
        .toLowerCase()
        .split(RegExp(r'\s+'))
        .where((term) => term.isNotEmpty)
        .every(haystack.contains);
  }
}

const _settingsSearchEntries = <_SettingsSearchEntry>[
  _SettingsSearchEntry(
    page: _SettingsPage.general,
    section: 'general-appearance',
    title: 'Application Theme',
    subtitle: 'Light, dark, or system application appearance.',
    keywords: 'color appearance',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.general,
    section: 'general-appearance',
    title: 'Host Icon',
    subtitle: 'Default host icon, OS badge, or OS icon.',
    keywords: 'workspace operating system badge',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.general,
    section: 'general-window',
    title: 'Window Size',
    subtitle: 'Default window width and height.',
    keywords: 'dimensions pixels',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.general,
    section: 'general-behavior',
    title: 'Workspace Page',
    subtitle: 'Show the workspace overview page.',
    keywords: 'sessions',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.general,
    section: 'general-behavior',
    title: 'Confirm on Close',
    subtitle: 'Ask before closing a connected terminal tab.',
    keywords: 'tabs confirmation',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.terminal,
    section: 'terminal-theme',
    title: 'Terminal Theme',
    subtitle: 'Terminal colors and custom theme.',
    keywords: 'background foreground ansi',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.terminal,
    section: 'terminal-font',
    title: 'Terminal Font',
    subtitle: 'Primary and CJK font families, size, weight, and ligatures.',
    keywords: 'text typography cjk chinese japanese korean line height spacing',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.terminal,
    section: 'terminal-shell',
    title: 'Shell Path',
    subtitle: 'Shell executable used for local sessions.',
    keywords: 'bash zsh fish command',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.terminal,
    section: 'terminal-shell',
    title: 'Terminal Emulation',
    subtitle: 'TERM emulation and scrollback settings.',
    keywords: 'xterm scrollback ssh keepalive',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.terminal,
    section: 'terminal-layout',
    title: 'Terminal Padding',
    subtitle: 'Inner spacing around terminal content.',
    keywords: 'layout scrollbar',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.terminal,
    section: 'terminal-cursor',
    title: 'Cursor',
    subtitle: 'Cursor shape and blinking behavior.',
    keywords: 'block beam underline blink',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.terminal,
    section: 'terminal-interaction',
    title: 'Copy on Select',
    subtitle: 'Automatically copy selected terminal text.',
    keywords: 'clipboard',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.terminal,
    section: 'terminal-interaction',
    title: 'Composer',
    subtitle: 'Command composer, autocomplete, and multiple tabs.',
    keywords: 'input suggestions multi tab',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.terminal,
    section: 'terminal-recording',
    title: 'History & Storage',
    subtitle: 'Manage terminal history and raw output.',
    keywords: 'capture logs retention encryption storage privacy',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.sftp,
    section: 'sftp-workspace',
    title: 'SFTP Tab',
    subtitle: 'Show the SFTP tab in the workspace.',
    keywords: 'file transfer',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.sftp,
    section: 'sftp-editors',
    title: 'Editors',
    subtitle: 'SSH editor, external editor, and text extensions.',
    keywords: 'vim nano vscode file types',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.sftp,
    section: 'sftp-files',
    title: 'SFTP Files',
    subtitle: 'Default directory and hidden files.',
    keywords: 'download dotfiles folder',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.ai,
    section: 'ai-providers',
    title: 'AI Providers',
    subtitle: 'Provider, endpoint, model, and API key.',
    keywords: 'openai anthropic base url',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.ai,
    section: 'ai-context',
    title: 'Terminal Context',
    subtitle: 'Include selection and recent terminal output.',
    keywords: 'ai assistant context',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.shortcuts,
    section: 'shortcuts-keyboard',
    title: 'Use Option as Meta Key',
    subtitle: 'Send Option key combinations with an Escape prefix.',
    keywords: 'alt escape keyboard',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.shortcuts,
    section: 'shortcuts-keyboard',
    title: 'Terminal Shortcuts',
    subtitle: 'Platform and Control Shift shortcuts.',
    keywords: 'cmd ctrl copy paste',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.shortcuts,
    section: 'shortcuts-bindings',
    title: 'Key Bindings',
    subtitle: 'Configure application keyboard shortcuts.',
    keywords: 'hotkeys commands',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.sync,
    section: null,
    title: 'Sync & Backup',
    subtitle: 'Configure backup providers, conflict handling, and automation.',
    keywords: 'github gist webdav s3 google drive onedrive backup',
  ),
  _SettingsSearchEntry(
    page: _SettingsPage.about,
    section: null,
    title: 'About Nauterm',
    subtitle: 'Application information, version, and updates.',
    keywords: 'build platform license download upgrade',
  ),
];

LoadedAiProviderCatalog? _cachedSettingsAiProviders;
String? _cachedSettingsAiProvidersDatabasePath;
Future<void>? _settingsSecureDataPreload;

Future<void> preloadNautermSettingsSecureData() {
  return _settingsSecureDataPreload ??= _preloadNautermSettingsSecureData();
}

Future<void> _preloadNautermSettingsSecureData() async {
  await waitForNautermUiPresentation();
  final paths = NautermPaths.resolve();
  try {
    final settings = await NautermConfigStore(paths).loadRuntimeSettings();
    _cachedSettingsAiProviders = AiProviderStore(
      paths,
    ).loadCatalog(settings.aiAssistant);
    _cachedSettingsAiProvidersDatabasePath = paths.databasePath;
  } on Object {
    // Settings can retry this read if startup preloading is unavailable.
  }
  try {
    await _loadGithubSyncSettingsInBackground(paths.databasePath);
  } on Object {
    // Sync settings remain optional when the encrypted store is unavailable.
  }
}

class SettingsPanel extends StatefulWidget {
  const SettingsPanel({super.key, this.detectExternalEditors = true});

  final bool detectExternalEditors;

  @override
  State<SettingsPanel> createState() => _SettingsPanelState();
}

class _SettingsPanelState extends State<SettingsPanel> {
  AppThemeMode _applicationTheme = appThemeMode;
  AppLanguage _applicationLanguage = appLanguage;
  bool _copyOnSelect = terminalCopyOnSelect;
  bool _composerEnabled = terminalComposerEnabled;
  bool _multiTabEnabled = terminalMultiTabEnabled;
  bool _scrollbarEnabled = terminalScrollbarEnabled;
  String _shellPath = _normalizedSettingsShellPath(terminalShellPath);
  late List<String> _shellPaths;
  List<String> get _selectableShellPaths {
    final defaultPath = systemDefaultShellPath();
    return _shellPaths
        .where((path) => path != defaultPath)
        .toList(growable: false);
  }

  String _scrollbackLines = terminalScrollbackLines.toString();
  String _sshKeepaliveIntervalSeconds = terminalSshKeepaliveIntervalSeconds
      .toString();
  TerminalType _emulationType = terminalEmulationType;
  TerminalEmulatorBackend _emulatorBackend = terminalEmulatorBackend;
  String? _themeId = terminalThemeId;
  TerminalTheme _customTheme = terminalCustomTheme;
  List<StoredTerminalTheme>? _allThemes;
  String _themeSearchQuery = '';
  TerminalTheme get _effectiveTheme {
    if (_themeId == 'custom') return _customTheme;
    if (_allThemes != null && _themeId != null) {
      final match = _allThemes!.where((t) => t.id == _themeId).firstOrNull;
      if (match != null) return match.theme;
    }
    return defaultTerminalTheme;
  }

  bool _useOptionAsMetaKey = terminalKeyboardConfig.useOptionAsMetaKey;
  TerminalShortcutConfig _shortcutConfig = terminalShortcutConfig;
  bool _sftpTabEnabled = sftpTabEnabled;
  bool _workspacePageEnabled = workspacePageEnabled;
  String _sftpDefaultDownloadDir = sftpDefaultDownloadDir ?? '';
  List<String> _sftpTextFileExtensions = sftpTextFileExtensions;
  bool _sftpShowHiddenFiles = sftpShowHiddenFiles;
  bool _includeTerminalSelection = aiAssistantConfig.includeTerminalSelection;
  bool _includeRecentTerminalOutput =
      aiAssistantConfig.includeRecentTerminalOutput;
  String _fontFamily = terminalFontConfig.resolvedFamily();
  String _cjkFontFamily = terminalFontConfig.cjkFamily ?? '';
  List<String> _monospaceFontFamilies = fallbackMonospaceFontFamilies;
  List<String> _systemFontFamilies = fallbackMonospaceFontFamilies;
  String _fontSize = terminalFontConfig.size.toStringAsFixed(0);
  EdgeInsets _terminalPadding = terminalPadding;
  String _windowWidth = (terminalWindowWidth ?? mainWindowSize.width)
      .round()
      .toString();
  String _windowHeight = (terminalWindowHeight ?? mainWindowSize.height)
      .round()
      .toString();
  _SettingsPage _selectedPage = _SettingsPage.general;
  String _settingsSearchQuery = '';
  late TextEditingController _settingsSearchController;
  final Map<String, GlobalKey> _settingsSectionKeys = {};
  late TextEditingController _sshEditorController;
  late TextEditingController _aiBaseUrlController;
  late TextEditingController _aiModelController;
  late TextEditingController _aiApiKeyController;
  late TextEditingController _scrollbackLinesController;
  late TextEditingController _sshKeepaliveIntervalSecondsController;
  late TextEditingController _sftpDownloadDirController;
  late TextEditingController _sftpTextFileExtensionsController;
  late TextEditingController _windowWidthController;
  late TextEditingController _windowHeightController;
  late TextEditingController _githubTokenController;
  late TextEditingController _githubRepositoryUrlController;
  late TextEditingController _githubBranchController;
  late TextEditingController _githubPathController;
  late TextEditingController _githubGistIdController;
  late TextEditingController _webdavUrlController;
  late TextEditingController _webdavUsernameController;
  late TextEditingController _webdavPasswordController;
  late TextEditingController _webdavPathController;
  late TextEditingController _s3EndpointController;
  late TextEditingController _s3RegionController;
  late TextEditingController _s3BucketController;
  late TextEditingController _s3AccessKeyController;
  late TextEditingController _s3SecretKeyController;
  late TextEditingController _s3PrefixController;
  late TextEditingController _s3FilenameController;
  late TextEditingController _syncMasterKeyController;
  late TextEditingController _syncMasterKeyConfirmController;
  late TextEditingController _autoSyncMinutesController;
  late TextEditingController _syncBackupCountController;
  final ScrollController _contentScrollController = ScrollController();
  Timer? _windowSizeUpdateTimer;
  bool _windowSizeDirty = false;
  Timer? _aiProviderSaveTimer;
  Timer? _syncPreferencesSaveTimer;
  bool _aiProviderDirty = false;
  AiProviderEntry? _aiProviderEntry;
  List<AiProviderEntry> _aiProviders = [];
  int _selectedAiProviderIndex = -1;
  List<_EditorEntry> _externalEditors = const [];
  SftpExternalEditorCommand? _externalEditor;
  Map<String, AiProviderPreset> _aiPresets = {};
  bool _aiPresetsRefreshing = false;
  String? _aiPresetsStatus;
  bool _githubHasToken = false;
  bool _githubGistHasToken = false;
  bool _s3HasCredentials = false;
  bool _s3CredentialsDirty = false;
  List<_CloudProviderInstance> _cloudProviders = const [];
  bool _hasLocalSyncKey = false;
  bool _githubSyncRunning = false;
  String? _githubSyncStatus;
  bool _githubSyncStatusIsError = false;
  List<Map<String, dynamic>> _githubSyncHistory = const [];
  bool _githubSyncHistoryLoading = false;
  List<Map<String, dynamic>> _githubGistSyncHistory = const [];
  bool _githubGistSyncHistoryLoading = false;
  bool _syncManagementUnlocked = false;
  String? _activeSyncProviderId;
  _SyncStrategy _syncStrategy = _SyncStrategy.smartMerge;
  bool _autoSync = false;
  int? _syncRevision;
  String? _syncSnapshotId;
  int? _remoteSyncRevision;
  String? _remoteSyncSnapshotId;
  bool _remoteSyncStatusRefreshing = false;

  @override
  void initState() {
    super.initState();
    nautermSyncStatusRevision.addListener(_reloadSyncStatus);
    final requestedPage = _takeRequestedSettingsPage();
    if (requestedPage != null) {
      _selectedPage = requestedPage;
    }
    _settingsSearchController = TextEditingController();
    settingsPageRequestRevision.addListener(_handleSettingsPageRequest);
    _sshEditorController = TextEditingController(text: sftpSshEditor);
    _aiBaseUrlController = TextEditingController(
      text: aiAssistantConfig.baseUrl,
    );
    _aiModelController = TextEditingController(text: aiAssistantConfig.model);
    _aiApiKeyController = TextEditingController(text: aiAssistantConfig.apiKey);
    _shellPaths = discoverSystemShells(current: _shellPath);
    _scrollbackLinesController = TextEditingController(text: _scrollbackLines);
    _sshKeepaliveIntervalSecondsController = TextEditingController(
      text: _sshKeepaliveIntervalSeconds,
    );
    _sftpDownloadDirController = TextEditingController(
      text: _sftpDefaultDownloadDir,
    );
    _sftpTextFileExtensionsController = TextEditingController(
      text: _sftpTextFileExtensions.join(', '),
    );
    _windowWidthController = TextEditingController(text: _windowWidth);
    _windowHeightController = TextEditingController(text: _windowHeight);
    _githubTokenController = TextEditingController();
    _githubRepositoryUrlController = TextEditingController();
    _githubBranchController = TextEditingController(text: 'main');
    _githubPathController = TextEditingController(text: 'nauterm-sync.enc');
    _githubGistIdController = TextEditingController();
    _autoSyncMinutesController = TextEditingController(
      text: '${nautermDefaultSyncIntervalMilliseconds ~/ 60000}',
    );
    _syncBackupCountController = TextEditingController(text: '10');
    _webdavUrlController = TextEditingController();
    _webdavUsernameController = TextEditingController();
    _webdavPasswordController = TextEditingController();
    _webdavPathController = TextEditingController(text: 'nauterm-sync.enc');
    _s3EndpointController = TextEditingController();
    _s3RegionController = TextEditingController(text: 'auto');
    _s3BucketController = TextEditingController();
    _s3AccessKeyController = TextEditingController();
    _s3SecretKeyController = TextEditingController();
    _s3PrefixController = TextEditingController();
    _s3FilenameController = TextEditingController(text: 'nauterm-sync.enc');
    _syncMasterKeyController = TextEditingController();
    _syncMasterKeyConfirmController = TextEditingController();
    _applyCachedGithubSyncSettings();
    _externalEditor = sftpExternalEditor;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      unawaited(_loadInitialDataAfterUiPresentation());
    });
  }

  Future<void> _loadInitialDataAfterUiPresentation() async {
    await waitForNautermUiPresentation();
    if (!mounted) {
      return;
    }
    await _loadRuntimeSettings();
    if (!mounted) {
      return;
    }
    unawaited(_loadAiPresets());
    unawaited(_loadSystemFonts());
    if (widget.detectExternalEditors) {
      unawaited(_detectEditors());
    }
  }

  Future<void> _loadSystemFonts() async {
    final families = await Future.wait([
      loadMonospaceFontFamilies(),
      loadSystemFontFamilies(),
    ]);
    if (!mounted) return;
    final monospaceFamilies = families[0];
    setState(() {
      _monospaceFontFamilies = monospaceFamilies;
      _systemFontFamilies = families[1];
    });
    if ((Platform.isWindows || Platform.isLinux) &&
        terminalFontConfig.family.trim().toLowerCase() == 'monospace' &&
        monospaceFamilies.isNotEmpty) {
      _updateTerminalFont(
        family:
            preferredMonospaceFontFamily(monospaceFamilies) ??
            monospaceFamilies.first,
      );
    }
  }

  void _reloadSyncStatus() {
    unawaited(_reloadSyncSettingsAfterBackgroundSync());
  }

  void _handleSettingsPageRequest() {
    final requestedPage = _takeRequestedSettingsPage();
    if (requestedPage != null) {
      _selectPage(requestedPage);
    }
  }

  Future<void> _reloadSyncSettingsAfterBackgroundSync() async {
    try {
      final preferences = await _loadSyncStatusInBackground(
        _githubSyncDatabasePath,
      );
      final snapshot = Map<String, dynamic>.from(
        preferences['sync_snapshot'] as Map? ?? const <String, dynamic>{},
      );
      if (!mounted) return;
      _mutate(() {
        _syncRevision = snapshot['revision'] as int?;
        _syncSnapshotId = snapshot['snapshot_id'] as String?;
        _remoteSyncRevision = preferences['remote_revision'] as int?;
        _remoteSyncSnapshotId = preferences['remote_snapshot_id'] as String?;
      });
      _cacheGithubSyncSetting(
        _githubSyncDatabasePath,
        'syncPreferences',
        preferences,
      );
    } on Object {
      // The next settings refresh can retry a failed status read.
    }
  }

  Future<void> _loadRuntimeSettings() async {
    try {
      await preloadNautermSettingsSecureData();
      final paths = NautermPaths.resolve();
      final configStore = NautermConfigStore(paths);
      final settings = await configStore.loadRuntimeSettings();
      final catalog =
          _cachedSettingsAiProvidersDatabasePath == paths.databasePath
          ? _cachedSettingsAiProviders
          : null;
      final providers =
          catalog ?? AiProviderStore(paths).loadCatalog(settings.aiAssistant);
      _cachedSettingsAiProviders = providers;
      _cachedSettingsAiProvidersDatabasePath = paths.databasePath;
      final provider = providers.active;
      applyNautermRuntimeSettings(settings);
      setAiAssistantConfig(provider.config);
      _aiProviderEntry = provider.entry;
      final allProviders = providers.providers;
      final activeIndex = allProviders.indexWhere((p) => p.active);
      if (!mounted) return;
      setState(() {
        _aiProviders = allProviders;
        _selectedAiProviderIndex = activeIndex >= 0 ? activeIndex : 0;
        _copyOnSelect = settings.copyOnSelect;
        _applicationLanguage = settings.appLanguage;
        _composerEnabled = settings.composerEnabled;
        _multiTabEnabled = settings.multiTabEnabled;
        _scrollbarEnabled = settings.scrollbarEnabled;
        _shellPath = _normalizedSettingsShellPath(settings.shellPath);
        terminalShellPath = _shellPath.isEmpty ? null : _shellPath;
        _shellPaths = discoverSystemShells(current: _shellPath);
        _scrollbackLines = settings.scrollbackLines.toString();
        _scrollbackLinesController.text = _scrollbackLines;
        _sshKeepaliveIntervalSeconds = settings.sshKeepaliveIntervalSeconds
            .toString();
        _sshKeepaliveIntervalSecondsController.text =
            _sshKeepaliveIntervalSeconds;
        _emulationType = settings.emulationType;
        _emulatorBackend = settings.emulatorBackend;
        _themeId = settings.themeId;
        if (settings.customThemeJson != null) {
          _customTheme = TerminalTheme.fromJson(settings.customThemeJson!);
        }
        _windowWidth = (settings.window.width ?? mainWindowSize.width)
            .round()
            .toString();
        _windowHeight = (settings.window.height ?? mainWindowSize.height)
            .round()
            .toString();
        _windowWidthController.text = _windowWidth;
        _windowHeightController.text = _windowHeight;
        _fontFamily = Platform.isWindows
            ? settings.font.resolvedFamily()
            : settings.font.family;
        _cjkFontFamily = settings.font.cjkFamily ?? '';
        _fontSize = settings.font.size.toStringAsFixed(0);
        _terminalPadding = settings.padding;
        _useOptionAsMetaKey = settings.keyboard.useOptionAsMetaKey;
        _sftpTabEnabled = settings.sftp.showTab;
        _workspacePageEnabled = settings.workspacePageEnabled;
        _aiBaseUrlController.text = provider.config.baseUrl;
        _aiModelController.text = provider.config.model;
        _aiApiKeyController.text = provider.config.apiKey;
        _includeTerminalSelection = provider.config.includeTerminalSelection;
        _includeRecentTerminalOutput =
            provider.config.includeRecentTerminalOutput;
        _sshEditorController.text = settings.sftp.sshEditor;
        _sftpTextFileExtensions = settings.sftp.textFileExtensions;
        _sftpTextFileExtensionsController.text = _sftpTextFileExtensions.join(
          ', ',
        );
        _externalEditor = settings.sftp.externalEditor;
        final externalEditor = settings.sftp.externalEditor;
        if (externalEditor != null &&
            !_externalEditors.any(
              (entry) => entry.command.id == externalEditor.id,
            )) {
          _externalEditors = [
            ..._externalEditors,
            _EditorEntry(name: externalEditor.label, command: externalEditor),
          ];
        }
      });
    } on Object {
      // Settings still work in-memory if the config file is unavailable.
    }
    _loadThemeCatalog();
  }

  Future<void> _loadThemeCatalog() async {
    try {
      final paths = NautermPaths.resolve();
      final catalog = TerminalThemeCatalog(
        paths.themesDirectory,
        additionalDirectories: paths.additionalThemeDirectories,
      );
      final themes = await catalog.loadThemes();
      if (!mounted) return;
      setState(() {
        _allThemes = themes;
      });
    } on Object {
      // Themes still work with just the default.
    }
  }

  Future<void> _loadAiPresets() async {
    try {
      final paths = NautermPaths.resolve();
      final configStore = NautermConfigStore(paths);
      final presetsUrl = await configStore.loadAiPresetsUrl();
      final presetStore = AiPresetStore(paths);
      final presets = await presetStore.loadPresets(remoteUrl: presetsUrl);
      if (!mounted) return;
      setState(() {
        _aiPresets = presets;
      });
    } on Object {
      // Presets are optional, settings work without them.
    }
  }

  Future<void> _refreshAiPresets() async {
    if (_aiPresetsRefreshing) {
      return;
    }
    setState(() {
      _aiPresetsRefreshing = true;
      _aiPresetsStatus = null;
    });
    try {
      final paths = NautermPaths.resolve();
      final configStore = NautermConfigStore(paths);
      final presetsUrl = await configStore.loadAiPresetsUrl();
      final presetStore = AiPresetStore(paths);
      final presets = await presetStore.refreshPresets(remoteUrl: presetsUrl);
      if (!mounted) return;
      setState(() {
        _aiPresets = presets;
        _aiPresetsStatus = 'Presets loaded (${presets.length}).';
      });
    } on Object {
      if (!mounted) return;
      setState(() {
        _aiPresetsStatus = 'Unable to update presets.';
      });
    } finally {
      if (mounted) {
        setState(() => _aiPresetsRefreshing = false);
      }
    }
  }

  void _persistRuntimeSettings() {
    unawaited(
      NautermConfigStore(
        NautermPaths.resolve(),
      ).saveRuntimeSettings(currentNautermRuntimeSettings()),
    );
    terminalConfigNotifier.value++;
  }

  void _setRawTerminalCaptureEnabled(bool enabled) {
    _mutate(() {
      terminalRecordingConfig = terminalRecordingConfig.copyWith(
        captureEnabled: enabled,
      );
    });
    _persistRuntimeSettings();
  }

  void _mutate(VoidCallback fn) {
    if (!mounted) {
      return;
    }
    setState(fn);
  }

  void _scheduleWindowSizeUpdate() {
    _windowSizeUpdateTimer?.cancel();
    _windowSizeUpdateTimer = Timer(
      const Duration(milliseconds: 300),
      _applyWindowSizeSettings,
    );
  }

  void _applyWindowSizeSettings() {
    _windowSizeUpdateTimer?.cancel();
    _windowSizeUpdateTimer = null;
    if (!_windowSizeDirty) {
      return;
    }
    final width = double.tryParse(_windowWidthController.text);
    final height = double.tryParse(_windowHeightController.text);
    if (width == null ||
        height == null ||
        width < mainWindowMinSize.width ||
        height < mainWindowMinSize.height) {
      return;
    }
    _windowSizeDirty = false;
    final applied = resizeMainWindow(Size(width, height));
    terminalWindowWidth = applied.width;
    terminalWindowHeight = applied.height;
    _windowWidth = applied.width.round().toString();
    _windowHeight = applied.height.round().toString();
    _persistRuntimeSettings();
  }

  void _updateKeyboardSettings(TerminalKeyboardConfig keyboard) {
    setState(() {
      _useOptionAsMetaKey = keyboard.useOptionAsMetaKey;
    });
    terminalKeyboardConfig = keyboard;
    _persistRuntimeSettings();
  }

  void _updateShortcutSettings(TerminalShortcutConfig shortcut) {
    setState(() => _shortcutConfig = shortcut);
    terminalShortcutConfig = shortcut;
    _persistRuntimeSettings();
  }

  void _updateTerminalFont({
    String? family,
    double? size,
    double? lineHeight,
    double? letterSpacing,
    bool? enableLigatures,
    int? weight,
    int? boldWeight,
  }) {
    final nextFont = TerminalFontConfig(
      family: family ?? terminalFontConfig.family,
      cjkFamily: terminalFontConfig.cjkFamily,
      size: size ?? terminalFontConfig.size,
      lineHeight: lineHeight ?? terminalFontConfig.lineHeight,
      letterSpacing: letterSpacing ?? terminalFontConfig.letterSpacing,
      enableLigatures: enableLigatures ?? terminalFontConfig.enableLigatures,
      weight: weight ?? terminalFontConfig.weight,
      boldWeight: boldWeight ?? terminalFontConfig.boldWeight,
    );
    setState(() {
      _fontFamily = nextFont.family;
      _fontSize = nextFont.size.toStringAsFixed(0);
    });
    terminalFontConfig = nextFont;
    _persistRuntimeSettings();
  }

  void _updateTerminalCjkFont(String value) {
    final normalized = value.trim();
    final nextFont = TerminalFontConfig(
      family: terminalFontConfig.family,
      cjkFamily: normalized.isEmpty ? null : normalized,
      size: terminalFontConfig.size,
      lineHeight: terminalFontConfig.lineHeight,
      letterSpacing: terminalFontConfig.letterSpacing,
      enableLigatures: terminalFontConfig.enableLigatures,
      weight: terminalFontConfig.weight,
      boldWeight: terminalFontConfig.boldWeight,
    );
    setState(() {
      _cjkFontFamily = nextFont.cjkFamily ?? '';
    });
    terminalFontConfig = nextFont;
    _persistRuntimeSettings();
  }

  void _updateAiSettings(
    AiAssistantConfig config, {
    bool providerChanged = true,
  }) {
    setAiAssistantConfig(config);
    if (providerChanged) {
      _aiProviderDirty = true;
      _aiProviderSaveTimer?.cancel();
      _aiProviderSaveTimer = Timer(
        const Duration(milliseconds: 350),
        _saveAiProvider,
      );
    }
    _persistRuntimeSettings();
  }

  void _saveAiProvider() {
    _aiProviderSaveTimer?.cancel();
    _aiProviderSaveTimer = null;
    if (!_aiProviderDirty) {
      return;
    }
    try {
      final saved = AiProviderStore(
        NautermPaths.resolve(),
      ).save(aiAssistantConfig, existing: _aiProviderEntry);
      _aiProviderEntry = saved;
      final index = _aiProviders.indexWhere((entry) => entry.id == saved.id);
      if (index >= 0) {
        _aiProviders = [
          ..._aiProviders.take(index),
          saved,
          ..._aiProviders.skip(index + 1),
        ];
      }
      _aiProviderDirty = false;
      _cacheAiProviderState();
    } on Object {
      // Keep the in-memory provider usable if the database is unavailable.
    }
  }

  void _cacheAiProviderState() {
    final paths = NautermPaths.resolve();
    final activeEntry = _aiProviders.where((entry) => entry.active).firstOrNull;
    final preferences = aiAssistantConfig;
    final activeConfig = activeEntry == null
        ? preferences
        : AiAssistantConfig(
            protocol: AiApiProtocol.fromString(activeEntry.protocol),
            baseUrl: activeEntry.baseUrl,
            model: activeEntry.model,
            apiKey: activeEntry.apiKey,
            maxTokens: activeEntry.maxTokens,
            includeTerminalSelection: preferences.includeTerminalSelection,
            includeRecentTerminalOutput:
                preferences.includeRecentTerminalOutput,
          );
    _cachedSettingsAiProviders = LoadedAiProviderCatalog(
      active: LoadedAiProvider(config: activeConfig, entry: activeEntry),
      providers: List<AiProviderEntry>.unmodifiable(_aiProviders),
    );
    _cachedSettingsAiProvidersDatabasePath = paths.databasePath;
  }

  Future<void> _detectEditors() async {
    final candidates = <_EditorEntry>[];
    final seen = <String>{};

    if (Platform.isMacOS) {
      for (final application in await loadSystemFileApplications(
        sftpTextFileExtensions,
      )) {
        final normalizedName = application.name.toLowerCase();
        if (seen.add(normalizedName)) {
          candidates.add(
            _EditorEntry(
              name: application.name,
              command: application.command,
              iconBytes: application.iconBytes,
            ),
          );
        }
      }
    } else {
      for (final (name, command) in _externalEditorCliCandidates()) {
        if (seen.contains(name.toLowerCase())) continue;
        try {
          final result = await Process.run(
            Platform.isWindows ? 'where' : 'which',
            [command.executable],
          );
          if (result.exitCode == 0) {
            final resolvedExecutable = result.stdout
                .toString()
                .split(RegExp(r'[\r\n]+'))
                .map((line) => line.trim())
                .firstWhere(
                  (line) => line.isNotEmpty,
                  orElse: () => command.executable,
                );
            seen.add(name.toLowerCase());
            candidates.add(
              _EditorEntry(
                name: name,
                command: SftpExternalEditorCommand(
                  label: command.label,
                  executable: resolvedExecutable,
                  arguments: command.arguments,
                ),
              ),
            );
          }
        } on Object {
          // Missing binaries are expected on most machines.
        }
      }
    }

    if (!mounted) return;
    setState(() {
      final current = _externalEditor;
      final resolvedCurrent = current == null
          ? null
          : candidates
                .where(
                  (entry) =>
                      entry.command.id == current.id ||
                      entry.name.toLowerCase() == current.label.toLowerCase(),
                )
                .firstOrNull;
      if (resolvedCurrent != null) {
        _externalEditor = resolvedCurrent.command;
        sftpExternalEditor = resolvedCurrent.command;
      }
      _externalEditors = [
        ...candidates,
        if (current != null && resolvedCurrent == null)
          _EditorEntry(name: current.label, command: current),
      ];
      if (_externalEditor == null && candidates.isNotEmpty) {
        _externalEditor = candidates.first.command;
        sftpExternalEditor = _externalEditor;
      }
    });
  }

  List<(String, SftpExternalEditorCommand)> _externalEditorCliCandidates() {
    return [
      (
        'Cursor',
        const SftpExternalEditorCommand(label: 'Cursor', executable: 'cursor'),
      ),
      (
        'VS Code',
        const SftpExternalEditorCommand(label: 'VS Code', executable: 'code'),
      ),
      (
        'VS Code Insiders',
        const SftpExternalEditorCommand(
          label: 'VS Code Insiders',
          executable: 'code-insiders',
        ),
      ),
      ('Zed', const SftpExternalEditorCommand(label: 'Zed', executable: 'zed')),
      (
        'Sublime Text',
        const SftpExternalEditorCommand(
          label: 'Sublime Text',
          executable: 'subl',
        ),
      ),
      if (Platform.isMacOS) ...[
        (
          'BBEdit',
          const SftpExternalEditorCommand(
            label: 'BBEdit',
            executable: 'bbedit',
          ),
        ),
        (
          'TextMate',
          const SftpExternalEditorCommand(
            label: 'TextMate',
            executable: 'mate',
          ),
        ),
        (
          'MacVim',
          const SftpExternalEditorCommand(label: 'MacVim', executable: 'mvim'),
        ),
      ],
      if (Platform.isLinux) ...[
        (
          'GNOME Text Editor',
          const SftpExternalEditorCommand(
            label: 'GNOME Text Editor',
            executable: 'gnome-text-editor',
          ),
        ),
        (
          'Gedit',
          const SftpExternalEditorCommand(label: 'Gedit', executable: 'gedit'),
        ),
        (
          'Kate',
          const SftpExternalEditorCommand(label: 'Kate', executable: 'kate'),
        ),
      ],
      if (Platform.isWindows)
        (
          'Notepad',
          const SftpExternalEditorCommand(
            label: 'Notepad',
            executable: 'notepad',
          ),
        ),
    ];
  }

  void _selectPage(_SettingsPage page) {
    if (_selectedPage == page && _settingsSearchQuery.isEmpty) return;
    final enteredSyncPage =
        page == _SettingsPage.sync && _selectedPage != _SettingsPage.sync;
    setState(() {
      _selectedPage = page;
      _settingsSearchQuery = '';
      _settingsSearchController.clear();
    });
    if (_contentScrollController.hasClients) {
      _contentScrollController.jumpTo(0);
    }
    if (enteredSyncPage) {
      unawaited(_refreshRemoteSyncStatus());
    }
  }

  void _closeSettingsWindow(BuildContext context) {
    if (defaultTargetPlatform != TargetPlatform.linux) {
      final windowController = WindowScope.maybeOf(context);
      if (windowController is RegularWindowController) {
        hideSettingsNativeWindow();
        windowController.destroy();
        return;
      }
    }

    hideSettingsWindow();
  }

  @override
  void dispose() {
    nautermSyncStatusRevision.removeListener(_reloadSyncStatus);
    settingsPageRequestRevision.removeListener(_handleSettingsPageRequest);
    _saveAiProvider();
    _applyWindowSizeSettings();
    _sshEditorController.dispose();
    _aiBaseUrlController.dispose();
    _aiModelController.dispose();
    _aiApiKeyController.dispose();
    _scrollbackLinesController.dispose();
    _sshKeepaliveIntervalSecondsController.dispose();
    _sftpDownloadDirController.dispose();
    _sftpTextFileExtensionsController.dispose();
    _windowWidthController.dispose();
    _windowHeightController.dispose();
    _githubTokenController.dispose();
    _githubRepositoryUrlController.dispose();
    _githubBranchController.dispose();
    _githubPathController.dispose();
    _githubGistIdController.dispose();
    _autoSyncMinutesController.dispose();
    _syncBackupCountController.dispose();
    _syncPreferencesSaveTimer?.cancel();
    _webdavUrlController.dispose();
    _webdavUsernameController.dispose();
    _webdavPasswordController.dispose();
    _webdavPathController.dispose();
    _s3EndpointController.dispose();
    _s3RegionController.dispose();
    _s3BucketController.dispose();
    _s3AccessKeyController.dispose();
    _s3SecretKeyController.dispose();
    _s3PrefixController.dispose();
    _s3FilenameController.dispose();
    _syncMasterKeyController.dispose();
    _syncMasterKeyConfirmController.dispose();
    _settingsSearchController.dispose();
    _contentScrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = context.nautermPalette;
    return Scaffold(
      backgroundColor: palette.background,
      body: LayoutBuilder(
        builder: (context, constraints) {
          final compactSidebar =
              constraints.maxWidth < _settingsCompactSidebarBreakpoint;
          return DefaultTextStyle.merge(
            style: TextStyle(color: palette.text),
            child: IconTheme.merge(
              data: IconThemeData(color: palette.text),
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: palette.surface,
                  border: Border.all(color: palette.softOutline),
                ),
                child: Row(
                  children: [
                    FocusScope(
                      key: const ValueKey('settings-sidebar-focus-scope'),
                      child: FocusTraversalGroup(
                        key: const ValueKey('settings-sidebar-focus-group'),
                        policy: OrderedTraversalPolicy(),
                        child: _SettingsSidebar(
                          selectedPage: _selectedPage,
                          compact: compactSidebar,
                          onPageSelected: _selectPage,
                        ),
                      ),
                    ),
                    Expanded(
                      child: FocusScope(
                        key: const ValueKey('settings-content-focus-scope'),
                        child: FocusTraversalGroup(
                          key: const ValueKey('settings-content-focus-group'),
                          policy: OrderedTraversalPolicy(),
                          child: Column(
                            children: [
                              _SettingsHeader(
                                searchController: _settingsSearchController,
                                onSearchChanged: (value) => setState(
                                  () => _settingsSearchQuery = value.trim(),
                                ),
                                onResetPage: _canResetPage
                                    ? () =>
                                          unawaited(_resetSettings(all: false))
                                    : null,
                                onResetAll: () =>
                                    unawaited(_resetSettings(all: true)),
                                onDone: () => _closeSettingsWindow(context),
                              ),
                              Expanded(child: _buildContent()),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          );
        },
      ),
    );
  }

  EdgeInsets get _contentPadding {
    final compact =
        MediaQuery.sizeOf(context).width < _settingsCompactSidebarBreakpoint;
    return compact
        ? const EdgeInsets.fromLTRB(20, 20, 22, 24)
        : const EdgeInsets.fromLTRB(26, 24, 30, 30);
  }

  Widget _buildContent() {
    if (_settingsSearchQuery.isNotEmpty) {
      return _buildSettingsSearchResults(this);
    }
    if (_selectedPage == _SettingsPage.terminal) {
      return _buildTerminalContent();
    }

    if (_selectedPage == _SettingsPage.sftp) {
      return _buildSftpContent();
    }

    if (_selectedPage == _SettingsPage.ai) {
      return _buildAiContent();
    }

    if (_selectedPage == _SettingsPage.sync) {
      return _buildSyncContent();
    }

    if (_selectedPage == _SettingsPage.shortcuts) {
      return _buildShortcutsContent();
    }

    if (_selectedPage == _SettingsPage.about) {
      return const _AboutSettingsPage();
    }

    return _buildSettingsGeneralContent(this);
  }

  bool get _canResetPage => switch (_selectedPage) {
    _SettingsPage.general ||
    _SettingsPage.terminal ||
    _SettingsPage.sftp ||
    _SettingsPage.ai ||
    _SettingsPage.shortcuts => true,
    _ => false,
  };

  GlobalKey _settingsSectionKey(String id) {
    return _settingsSectionKeys.putIfAbsent(id, GlobalKey.new);
  }

  void _openSearchResult(_SettingsSearchEntry entry) {
    final enteredSyncPage =
        entry.page == _SettingsPage.sync && _selectedPage != _SettingsPage.sync;
    setState(() {
      _selectedPage = entry.page;
      _settingsSearchQuery = '';
      _settingsSearchController.clear();
    });
    if (enteredSyncPage) {
      unawaited(_refreshRemoteSyncStatus());
    }
    unawaited(_scrollToSearchResult(entry));
  }

  Future<void> _scrollToSearchResult(_SettingsSearchEntry entry) async {
    final section = entry.section;
    if (section == null) return;
    await WidgetsBinding.instance.endOfFrame;
    if (!mounted) return;
    var targetContext = _settingsSectionKeys[section]?.currentContext;
    if (targetContext == null) {
      await WidgetsBinding.instance.endOfFrame;
      if (!mounted) return;
      targetContext = _settingsSectionKeys[section]?.currentContext;
    }
    if (targetContext == null) return;
    if (!targetContext.mounted) return;
    await Scrollable.ensureVisible(
      targetContext,
      duration: const Duration(milliseconds: 220),
      curve: Curves.easeOutCubic,
      alignment: 0.08,
    );
  }

  Future<void> _resetSettings({required bool all}) async {
    final defaults = await NautermConfigStore(
      NautermPaths.resolve(),
    ).loadDefaultRuntimeSettings();
    if (!mounted) return;
    final pages = all
        ? const {
            _SettingsPage.general,
            _SettingsPage.terminal,
            _SettingsPage.sftp,
            _SettingsPage.ai,
            _SettingsPage.shortcuts,
          }
        : {_selectedPage};

    setState(() {
      if (pages.contains(_SettingsPage.general)) {
        _applicationTheme = defaults.appThemeMode;
        setAppThemeMode(defaults.appThemeMode);
        _applicationLanguage = defaults.appLanguage;
        setAppLanguage(defaults.appLanguage);
        hostIconMode = defaults.hostIconMode;
        terminalWindowWidth = defaults.window.width;
        terminalWindowHeight = defaults.window.height;
        terminalWindowX = defaults.window.x;
        terminalWindowY = defaults.window.y;
        _windowWidth = (defaults.window.width ?? mainWindowSize.width)
            .round()
            .toString();
        _windowHeight = (defaults.window.height ?? mainWindowSize.height)
            .round()
            .toString();
        _windowWidthController.text = _windowWidth;
        _windowHeightController.text = _windowHeight;
        _workspacePageEnabled = defaults.workspacePageEnabled;
        setWorkspacePageEnabled(defaults.workspacePageEnabled);
        terminalConfirmOnClose = defaults.confirmOnClose;
      }
      if (pages.contains(_SettingsPage.terminal)) {
        _copyOnSelect = defaults.copyOnSelect;
        terminalCopyOnSelect = defaults.copyOnSelect;
        _composerEnabled = defaults.composerEnabled;
        terminalComposerEnabled = defaults.composerEnabled;
        terminalAutocompleteEnabled = defaults.autocompleteEnabled;
        _multiTabEnabled = defaults.multiTabEnabled;
        terminalMultiTabEnabled = defaults.multiTabEnabled;
        _scrollbarEnabled = defaults.scrollbarEnabled;
        terminalScrollbarEnabled = defaults.scrollbarEnabled;
        _shellPath = defaults.shellPath ?? '';
        terminalShellPath = defaults.shellPath;
        _shellPaths = discoverSystemShells(current: _shellPath);
        _scrollbackLines = defaults.scrollbackLines.toString();
        terminalScrollbackLines = defaults.scrollbackLines;
        _scrollbackLinesController.text = _scrollbackLines;
        _sshKeepaliveIntervalSeconds = defaults.sshKeepaliveIntervalSeconds
            .toString();
        terminalSshKeepaliveIntervalSeconds =
            defaults.sshKeepaliveIntervalSeconds;
        _sshKeepaliveIntervalSecondsController.text =
            _sshKeepaliveIntervalSeconds;
        _emulationType = defaults.emulationType;
        terminalEmulationType = defaults.emulationType;
        _emulatorBackend = defaults.emulatorBackend;
        terminalEmulatorBackend = defaults.emulatorBackend;
        _themeId = defaults.themeId;
        terminalThemeId = defaults.themeId;
        _customTheme = defaults.customThemeJson == null
            ? defaultTerminalTheme
            : TerminalTheme.fromJson(defaults.customThemeJson!);
        terminalCustomTheme = _customTheme;
        _fontFamily = defaults.font.resolvedFamily();
        _cjkFontFamily = defaults.font.cjkFamily ?? '';
        _fontSize = defaults.font.size.toStringAsFixed(0);
        terminalFontConfig = defaults.font;
        _terminalPadding = defaults.padding;
        terminalPadding = defaults.padding;
        terminalCursorShape = defaults.cursor.shape;
        terminalCursorBlink = defaults.cursor.blink;
        terminalRecordingConfig = defaults.recording;
        terminalPaddingNotifier.value++;
      }
      if (pages.contains(_SettingsPage.sftp)) {
        _sftpTabEnabled = defaults.sftp.showTab;
        setSftpTabEnabled(defaults.sftp.showTab);
        _sshEditorController.text = defaults.sftp.sshEditor;
        sftpSshEditor = defaults.sftp.sshEditor;
        sftpExternalEditor = defaults.sftp.externalEditor;
        _sftpTextFileExtensions = defaults.sftp.textFileExtensions;
        sftpTextFileExtensions = defaults.sftp.textFileExtensions;
        _sftpTextFileExtensionsController.text = defaults
            .sftp
            .textFileExtensions
            .join(', ');
        _sftpDefaultDownloadDir = defaults.sftp.defaultDownloadDir ?? '';
        sftpDefaultDownloadDir = defaults.sftp.defaultDownloadDir;
        _sftpDownloadDirController.text = _sftpDefaultDownloadDir;
        _sftpShowHiddenFiles = defaults.sftp.showHiddenFiles;
        sftpShowHiddenFiles = defaults.sftp.showHiddenFiles;
      }
      if (pages.contains(_SettingsPage.shortcuts)) {
        _useOptionAsMetaKey = defaults.keyboard.useOptionAsMetaKey;
        terminalKeyboardConfig = defaults.keyboard;
        _shortcutConfig = defaults.shortcuts;
        terminalShortcutConfig = defaults.shortcuts;
      }
      if (pages.contains(_SettingsPage.ai)) {
        _includeTerminalSelection =
            defaults.aiAssistant.includeTerminalSelection;
        _includeRecentTerminalOutput =
            defaults.aiAssistant.includeRecentTerminalOutput;
        setAiAssistantConfig(
          aiAssistantConfig.copyWith(
            includeTerminalSelection:
                defaults.aiAssistant.includeTerminalSelection,
            includeRecentTerminalOutput:
                defaults.aiAssistant.includeRecentTerminalOutput,
          ),
        );
      }
    });
    _persistRuntimeSettings();
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            context.tr(
              all
                  ? 'All settings reset to defaults.'
                  : 'Page reset to defaults.',
            ),
          ),
          duration: const Duration(seconds: 2),
        ),
      );
    }
  }

  void _selectTheme(String? id) {
    setState(() {
      _themeId = id;
      if (id != 'custom') {
        terminalThemeId = id;
        terminalCustomTheme = _effectiveTheme;
      }
    });
    _persistRuntimeSettings();
  }

  void _updateThemeColor(String field, Color color) {
    setState(() {
      final t = _customTheme;
      _customTheme = TerminalTheme(
        name: t.name,
        type: t.type,
        primary: switch (field) {
          'accent' => TerminalPrimaryColors(
            accent: color,
            background: t.primary.background,
            foreground: t.primary.foreground,
          ),
          'foreground' => TerminalPrimaryColors(
            accent: t.primary.accent,
            background: t.primary.background,
            foreground: color,
          ),
          'background' => TerminalPrimaryColors(
            accent: t.primary.accent,
            background: color,
            foreground: t.primary.foreground,
          ),
          _ => t.primary,
        },
        cursor: switch (field) {
          'cursor' => TerminalCursorColors(cursor: color, text: t.cursor.text),
          'cursorText' => TerminalCursorColors(
            cursor: t.cursor.cursor,
            text: color,
          ),
          _ => t.cursor,
        },
        selection: switch (field) {
          'selection' => TerminalSelectionColors(
            background: color,
            text: t.selection.text,
          ),
          'selectionText' => TerminalSelectionColors(
            background: t.selection.background,
            text: color,
          ),
          _ => t.selection,
        },
        normal: t.normal,
        bright: t.bright,
      );
      _themeId = 'custom';
      terminalThemeId = 'custom';
      terminalCustomTheme = _customTheme;
    });
    _persistRuntimeSettings();
  }

  void _updateAnsiColor(bool bright, int index, Color color) {
    setState(() {
      if (bright) {
        _customTheme = TerminalTheme(
          name: _customTheme.name,
          type: _customTheme.type,
          primary: _customTheme.primary,
          cursor: _customTheme.cursor,
          selection: _customTheme.selection,
          normal: _customTheme.normal,
          bright: _customTheme.bright.copyWithIndex(index, color),
        );
      } else {
        _customTheme = TerminalTheme(
          name: _customTheme.name,
          type: _customTheme.type,
          primary: _customTheme.primary,
          cursor: _customTheme.cursor,
          selection: _customTheme.selection,
          normal: _customTheme.normal.copyWithIndex(index, color),
          bright: _customTheme.bright,
        );
      }
      _themeId = 'custom';
      terminalThemeId = 'custom';
      terminalCustomTheme = _customTheme;
    });
    _persistRuntimeSettings();
  }

  Widget _buildTerminalContent() => _buildSettingsTerminalContent(this);

  Widget _buildSftpContent() => _buildSettingsSftpContent(this);

  Widget _buildShortcutsContent() => _buildSettingsShortcutsContent(this);

  Widget _buildAiContent() => _buildSettingsAiContent(this);

  Widget _buildSyncContent() => _buildSettingsSyncContent(this);

  void _selectAiProvider(int index) {
    if (index < 0 || index >= _aiProviders.length) return;
    final entry = _aiProviders[index];
    setState(() {
      _selectedAiProviderIndex = index;
      _aiBaseUrlController.text = entry.baseUrl;
      _aiModelController.text = entry.model;
      _aiApiKeyController.text = entry.apiKey;
      _aiProviderDirty = false;
    });
  }

  void _addAiProvider() {
    if (_aiPresets.isEmpty) {
      _createAiProviderFromPreset(null);
      return;
    }

    unawaited(_showAddAiProviderDialog());
  }

  Future<void> _showAddAiProviderDialog() async {
    final result = await showNautermDialog<_AiPresetDialogResult>(
      context: context,
      builder: (context) => _AiPresetDialog(presets: _aiPresets),
    );
    if (!mounted || result == null) {
      return;
    }
    _createAiProviderFromPreset(result.preset);
  }

  void _createAiProviderFromPreset(AiProviderPreset? preset) {
    final store = AiProviderStore(NautermPaths.resolve());
    final hasActive = _aiProviders.any((p) => p.active);
    final protocol = preset != null
        ? (preset.protocol == 'anthropic'
              ? AiApiProtocol.anthropic
              : AiApiProtocol.openAi)
        : AiApiProtocol.openAi;
    final newEntry = store.save(
      AiAssistantConfig(
        protocol: protocol,
        baseUrl: preset?.baseUrl ?? AiAssistantConfig.openAiDefaultBaseUrl,
        model: preset?.defaultModels.firstOrNull ?? '',
      ),
      existing: AiProviderEntry(
        name: preset?.name ?? 'New Provider',
        protocol: protocol.storageValue,
        baseUrl: preset?.baseUrl ?? AiAssistantConfig.openAiDefaultBaseUrl,
        model: preset?.defaultModels.firstOrNull ?? '',
        apiKey: '',
        config: const <String, Object?>{'max_tokens': 4096},
        active: !hasActive,
      ),
    );
    setState(() {
      _aiProviders = store.listProviders();
      _selectedAiProviderIndex = _aiProviders.indexWhere(
        (p) => p.id == newEntry.id,
      );
    });
    _selectAiProvider(_selectedAiProviderIndex);
    if (!hasActive) {
      setAiAssistantConfig(
        AiAssistantConfig(
          protocol: protocol,
          baseUrl: preset?.baseUrl ?? AiAssistantConfig.openAiDefaultBaseUrl,
          model: preset?.defaultModels.firstOrNull ?? '',
        ),
      );
      _persistRuntimeSettings();
    }
    _cacheAiProviderState();
  }

  void _deleteAiProvider(int index) {
    if (index < 0 || index >= _aiProviders.length) return;
    final entry = _aiProviders[index];
    final wasActive = entry.active;
    final store = AiProviderStore(NautermPaths.resolve());
    store.deleteProvider(entry);
    setState(() {
      _aiProviders = store.listProviders();
      if (_selectedAiProviderIndex >= _aiProviders.length) {
        _selectedAiProviderIndex = _aiProviders.length - 1;
      }
    });
    // If we deleted the active provider, activate the first remaining one
    if (wasActive && _aiProviders.isNotEmpty) {
      _setActiveAiProvider(0);
    } else if (_aiProviders.isNotEmpty) {
      _selectAiProvider(_selectedAiProviderIndex);
    }
    _cacheAiProviderState();
  }

  void _setActiveAiProvider(int index) {
    if (index < 0 || index >= _aiProviders.length) return;
    final entry = _aiProviders[index];
    final store = AiProviderStore(NautermPaths.resolve());
    final config = AiAssistantConfig(
      protocol: AiApiProtocol.fromString(entry.protocol),
      baseUrl: entry.baseUrl,
      model: entry.model,
      apiKey: entry.apiKey,
      maxTokens: entry.maxTokens,
    );
    store.save(config, existing: entry, active: true);
    setAiAssistantConfig(config);
    _persistRuntimeSettings();
    setState(() {
      _aiProviders = store.listProviders();
      _selectedAiProviderIndex = index;
    });
    _cacheAiProviderState();
  }

  void _updateSelectedAiProviderField({
    String? name,
    String? protocol,
    String? baseUrl,
    String? model,
    String? apiKey,
    int? maxTokens,
  }) {
    if (_selectedAiProviderIndex < 0 ||
        _selectedAiProviderIndex >= _aiProviders.length) {
      return;
    }
    final entry = _aiProviders[_selectedAiProviderIndex];
    final store = AiProviderStore(NautermPaths.resolve());
    final config = AiAssistantConfig(
      protocol: AiApiProtocol.fromString(protocol ?? entry.protocol),
      baseUrl: baseUrl ?? entry.baseUrl,
      model: model ?? entry.model,
      apiKey: apiKey ?? entry.apiKey,
      maxTokens: maxTokens ?? entry.maxTokens,
    );
    store.save(
      config,
      existing: entry,
      name: name ?? entry.name,
      active: entry.active,
    );
    if (entry.active) {
      setAiAssistantConfig(config);
      _persistRuntimeSettings();
    }
    setState(() {
      _aiProviders = store.listProviders();
      _selectedAiProviderIndex = _aiProviders.indexWhere(
        (p) => p.id == entry.id,
      );
    });
    _cacheAiProviderState();
  }
}
