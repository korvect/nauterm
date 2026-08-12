import 'dart:io';
import 'dart:ui' show Locale, PlatformDispatcher;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../ai/ai_config.dart';
import '../app/nauterm_localizations.dart';
import 'terminal_recording_config.dart';
import 'terminal_models.dart';
import 'terminal_theme.dart';

const TerminalConfig defaultTerminalConfig = TerminalConfig();
const int maxSshKeepaliveIntervalSeconds = 0xffffffff;

bool terminalCopyOnSelect = false;
bool terminalSelectCommandBlockOnClick = true;
bool terminalComposerEnabled = true;
bool terminalAutocompleteEnabled = false;
bool terminalMultiTabEnabled = false;
bool terminalScrollbarEnabled = true;
int terminalScrollbackLines = 10000;
int terminalSshKeepaliveIntervalSeconds = 20;
TerminalType terminalEmulationType = TerminalType.xterm256Color;
TerminalEmulatorBackend terminalEmulatorBackend =
    TerminalEmulatorBackend.alacritty;
String? terminalThemeId;
TerminalTheme terminalCustomTheme = defaultTerminalTheme;

EdgeInsets terminalPadding = EdgeInsets.zero;

enum TerminalMoshPredictionMode { adaptive, always, never }

enum TerminalSshPredictionMode {
  adaptive,
  always,
  never;

  static TerminalSshPredictionMode fromString(String? value) => switch (value) {
    'always' => TerminalSshPredictionMode.always,
    'never' => TerminalSshPredictionMode.never,
    _ => TerminalSshPredictionMode.adaptive,
  };
}

enum TerminalOsc52Mode { copy, copyAndPaste }

TerminalOsc52Mode terminalOsc52Mode = TerminalOsc52Mode.copy;

TerminalMoshPredictionMode terminalMoshPredictionMode =
    TerminalMoshPredictionMode.adaptive;
TerminalSshPredictionMode terminalSshPredictionMode =
    TerminalSshPredictionMode.adaptive;

TerminalCursorShape terminalCursorShape = TerminalCursorShape.block;
bool terminalCursorBlink = true;
TerminalBellConfig terminalBellConfig = const TerminalBellConfig();
TerminalPointerConfig terminalPointerConfig = const TerminalPointerConfig();
bool terminalConfirmOnClose = true;
NautermRecordingConfig terminalRecordingConfig = const NautermRecordingConfig();

enum HostIconMode {
  defaultIcon,
  osBadge,
  osIcon;

  static HostIconMode fromString(String? value) {
    return switch (value) {
      'default' => HostIconMode.defaultIcon,
      'osIcon' => HostIconMode.osIcon,
      _ => HostIconMode.osBadge,
    };
  }

  String get configValue => switch (this) {
    HostIconMode.defaultIcon => 'default',
    HostIconMode.osBadge => 'osBadge',
    HostIconMode.osIcon => 'osIcon',
  };
}

HostIconMode hostIconMode = HostIconMode.osBadge;

String? terminalShellPath;

double? terminalWindowWidth;
double? terminalWindowHeight;
double? terminalWindowX;
double? terminalWindowY;

final ValueNotifier<int> terminalPaddingNotifier = ValueNotifier<int>(0);
final ValueNotifier<int> terminalConfigNotifier = ValueNotifier<int>(0);

TerminalFontConfig terminalFontConfig = defaultTerminalConfig.font;

const List<double> terminalFontSizeOptions = [
  9,
  10,
  11,
  12,
  13,
  14,
  15,
  16,
  17,
  18,
  24,
  36,
  48,
  64,
  72,
  96,
  144,
  288,
];

TerminalKeyboardConfig terminalKeyboardConfig = defaultTerminalConfig.keyboard;

TerminalShortcutConfig terminalShortcutConfig = const TerminalShortcutConfig();

enum AppThemeMode {
  light,
  system,
  dark;

  static AppThemeMode fromString(String? value) {
    return switch (value) {
      'light' => AppThemeMode.light,
      'system' => AppThemeMode.system,
      'dark' => AppThemeMode.dark,
      _ => AppThemeMode.light,
    };
  }

  ThemeMode toFlutterThemeMode() {
    return switch (this) {
      AppThemeMode.light => ThemeMode.light,
      AppThemeMode.system => ThemeMode.system,
      AppThemeMode.dark => ThemeMode.dark,
    };
  }
}

AppThemeMode appThemeMode = AppThemeMode.light;
final ValueNotifier<AppThemeMode> appThemeModeListenable =
    ValueNotifier<AppThemeMode>(appThemeMode);

void setAppThemeMode(AppThemeMode value) {
  appThemeMode = value;
  appThemeModeListenable.value = value;
}

String sftpSshEditor = 'vim';
SftpExternalEditorCommand? sftpExternalEditor;
const List<String> sftpDefaultTextFileExtensions = [
  'txt',
  'md',
  'markdown',
  'json',
  'jsonc',
  'yaml',
  'yml',
  'xml',
  'toml',
  'ini',
  'conf',
  'cfg',
  'log',
  'env',
  'properties',
  'sh',
  'bash',
  'zsh',
  'fish',
  'py',
  'rb',
  'pl',
  'php',
  'lua',
  'js',
  'jsx',
  'ts',
  'tsx',
  'dart',
  'swift',
  'm',
  'mm',
  'c',
  'h',
  'cc',
  'cpp',
  'hpp',
  'java',
  'kt',
  'kts',
  'go',
  'rs',
  'sql',
  'html',
  'css',
  'scss',
  'sass',
  'less',
  'vue',
  'svelte',
  'gradle',
];
List<String> sftpTextFileExtensions = sftpDefaultTextFileExtensions;
bool sftpTabEnabled = true;
String? sftpDefaultDownloadDir;
bool sftpShowHiddenFiles = false;
const int sftpDefaultConcurrentTasks = 3;
const int sftpDefaultTransferThreads = 8;
int sftpConcurrentTasks = sftpDefaultConcurrentTasks;
int sftpTransferThreads = sftpDefaultTransferThreads;
bool workspacePageEnabled = true;

String sftpEffectiveLocalDirectory() {
  final configured = sftpDefaultDownloadDir;
  if (configured != null && configured.isNotEmpty) {
    return configured;
  }
  return defaultSftpLocalDirectory();
}

String defaultSftpLocalDirectory() {
  var home =
      Platform.environment['HOME'] ??
      Platform.environment['USERPROFILE'] ??
      Directory.current.path;
  if (Platform.isWindows) {
    home = home.replaceAll('/', Platform.pathSeparator);
  }
  return home;
}

String fallbackDownloadsDirectory() {
  return '${defaultSftpLocalDirectory()}${Platform.pathSeparator}Downloads';
}

final ValueNotifier<bool> sftpTabEnabledListenable = ValueNotifier<bool>(
  sftpTabEnabled,
);

void setSftpTabEnabled(bool value) {
  sftpTabEnabled = value;
  sftpTabEnabledListenable.value = value;
}

final ValueNotifier<bool> workspacePageEnabledListenable = ValueNotifier<bool>(
  workspacePageEnabled,
);

void setWorkspacePageEnabled(bool value) {
  workspacePageEnabled = value;
  workspacePageEnabledListenable.value = value;
}

@immutable
class SftpExternalEditorCommand {
  const SftpExternalEditorCommand({
    required this.label,
    required this.executable,
    this.arguments = const [],
  });

  final String label;
  final String executable;
  final List<String> arguments;

  String get id => [executable, ...arguments].join('\u{1f}');

  Map<String, Object?> toJson() {
    return <String, Object?>{
      'label': label,
      'executable': executable,
      'arguments': arguments,
    };
  }

  static SftpExternalEditorCommand? fromJson(Object? value) {
    if (value is! Map) {
      return null;
    }
    final map = value.cast<String, Object?>();
    final executable = (map['executable'] as String?)?.trim();
    if (executable == null || executable.isEmpty) {
      return null;
    }
    final label = (map['label'] as String?)?.trim();
    final rawArguments = map['arguments'];
    return SftpExternalEditorCommand(
      label: label == null || label.isEmpty ? executable : label,
      executable: executable,
      arguments: rawArguments is List
          ? [
              for (final argument in rawArguments)
                if (argument is String) argument,
            ]
          : const [],
    );
  }
}

String resolveSftpExternalEditorExecutable(
  String executable, {
  Map<String, String>? environment,
  Iterable<String>? additionalSearchDirectories,
  bool Function(String path)? fileExists,
}) {
  final trimmed = executable.trim();
  if (trimmed.isEmpty ||
      trimmed.contains(Platform.pathSeparator) ||
      trimmed.contains('/') ||
      trimmed.contains(r'\')) {
    return trimmed;
  }

  final effectiveEnvironment = environment ?? Platform.environment;
  final effectiveFileExists = fileExists ?? (path) => File(path).existsSync();
  final pathSeparator = Platform.isWindows ? ';' : ':';
  final searchDirectories = <String>[
    ...?additionalSearchDirectories,
    ...?effectiveEnvironment['PATH']
        ?.split(pathSeparator)
        .where((directory) => directory.trim().isNotEmpty),
    if (Platform.isMacOS || Platform.isLinux) ...[
      '/opt/homebrew/bin',
      '/usr/local/bin',
      '/opt/local/bin',
      '/usr/bin',
      '/bin',
      if (effectiveEnvironment['HOME'] case final home?) '$home/.local/bin',
    ],
  ];

  for (final directory in searchDirectories) {
    final normalizedDirectory = directory.trim();
    if (normalizedDirectory.isEmpty) {
      continue;
    }
    final separator = normalizedDirectory.endsWith(Platform.pathSeparator)
        ? ''
        : Platform.pathSeparator;
    final candidate = '$normalizedDirectory$separator$trimmed';
    if (effectiveFileExists(candidate)) {
      return candidate;
    }
  }
  return trimmed;
}

List<String> normalizeSftpTextFileExtensions(Iterable<Object?> values) {
  final normalized = <String>[];
  final seen = <String>{};
  for (final value in values) {
    if (value is! String) {
      continue;
    }
    final extension = value.trim().toLowerCase().replaceFirst(
      RegExp(r'^\*?\.+'),
      '',
    );
    if (extension.isEmpty ||
        !RegExp(r'^[a-z0-9][a-z0-9+_-]*$').hasMatch(extension) ||
        !seen.add(extension)) {
      continue;
    }
    normalized.add(extension);
  }
  return List.unmodifiable(normalized);
}

List<String> parseSftpTextFileExtensions(String value) {
  return normalizeSftpTextFileExtensions(value.split(RegExp(r'[\s,;]+')));
}

bool sftpExternalEditorSupportsFileName(String fileName) {
  final leafName = fileName.replaceAll('\\', '/').split('/').last;
  final dotIndex = leafName.lastIndexOf('.');
  if (dotIndex < 0 || dotIndex == leafName.length - 1) {
    return true;
  }
  final extension = leafName.substring(dotIndex + 1).toLowerCase();
  return sftpTextFileExtensions.contains(extension);
}

@immutable
class SftpConfig {
  const SftpConfig({
    this.showTab = true,
    this.sshEditor = 'vim',
    this.externalEditor,
    this.textFileExtensions = sftpDefaultTextFileExtensions,
    this.defaultDownloadDir,
    this.showHiddenFiles = false,
    this.concurrentTasks = sftpDefaultConcurrentTasks,
    this.transferThreads = sftpDefaultTransferThreads,
  });

  final bool showTab;
  final String sshEditor;
  final SftpExternalEditorCommand? externalEditor;
  final List<String> textFileExtensions;
  final String? defaultDownloadDir;
  final bool showHiddenFiles;
  final int concurrentTasks;
  final int transferThreads;
}

@immutable
class WindowConfig {
  const WindowConfig({this.width, this.height, this.x, this.y});

  final double? width;
  final double? height;
  final double? x;
  final double? y;
}

@immutable
class NautermRuntimeSettings {
  const NautermRuntimeSettings({
    required this.copyOnSelect,
    required this.font,
    required this.keyboard,
    this.selectCommandBlockOnClick = true,
    this.shortcuts = const TerminalShortcutConfig(),
    this.sftp = const SftpConfig(),
    this.aiAssistant = const AiAssistantConfig(),
    this.appThemeMode = AppThemeMode.light,
    this.appLanguage = AppLanguage.system,
    this.padding = EdgeInsets.zero,
    this.composerEnabled = true,
    this.autocompleteEnabled = false,
    this.multiTabEnabled = false,
    this.scrollbarEnabled = true,
    this.scrollbackLines = 10000,
    this.sshKeepaliveIntervalSeconds = 20,
    this.sshPredictionMode = TerminalSshPredictionMode.adaptive,
    this.emulationType = TerminalType.xterm256Color,
    this.emulatorBackend = TerminalEmulatorBackend.alacritty,
    this.themeId,
    this.customThemeJson,
    this.cursor = const TerminalCursorConfig(),
    this.bell = const TerminalBellConfig(),
    this.pointer = const TerminalPointerConfig(),
    this.confirmOnClose = true,
    this.hostIconMode = HostIconMode.osBadge,
    this.shellPath,
    this.window = const WindowConfig(),
    this.workspacePageEnabled = true,
    this.recording = const NautermRecordingConfig(),
  });

  final bool copyOnSelect;
  final bool selectCommandBlockOnClick;
  final TerminalFontConfig font;
  final TerminalKeyboardConfig keyboard;
  final TerminalShortcutConfig shortcuts;
  final SftpConfig sftp;
  final AiAssistantConfig aiAssistant;
  final AppThemeMode appThemeMode;
  final AppLanguage appLanguage;
  final EdgeInsets padding;
  final bool composerEnabled;
  final bool autocompleteEnabled;
  final bool multiTabEnabled;
  final bool scrollbarEnabled;
  final int scrollbackLines;
  final int sshKeepaliveIntervalSeconds;
  final TerminalSshPredictionMode sshPredictionMode;
  final TerminalType emulationType;
  final TerminalEmulatorBackend emulatorBackend;
  final String? themeId;
  final Map<String, Object?>? customThemeJson;
  final TerminalCursorConfig cursor;
  final TerminalBellConfig bell;
  final TerminalPointerConfig pointer;
  final bool confirmOnClose;
  final HostIconMode hostIconMode;
  final String? shellPath;
  final WindowConfig window;
  final bool workspacePageEnabled;
  final NautermRecordingConfig recording;
}

void applyNautermRuntimeSettings(NautermRuntimeSettings settings) {
  terminalCopyOnSelect = settings.copyOnSelect;
  terminalSelectCommandBlockOnClick = settings.selectCommandBlockOnClick;
  terminalComposerEnabled = settings.composerEnabled;
  terminalAutocompleteEnabled = settings.autocompleteEnabled;
  terminalMultiTabEnabled = settings.multiTabEnabled;
  terminalScrollbarEnabled = settings.scrollbarEnabled;
  terminalScrollbackLines = settings.scrollbackLines;
  terminalSshKeepaliveIntervalSeconds = settings.sshKeepaliveIntervalSeconds;
  terminalSshPredictionMode = settings.sshPredictionMode;
  terminalEmulationType = settings.emulationType;
  terminalEmulatorBackend = settings.emulatorBackend;
  terminalThemeId = settings.themeId;
  terminalCustomTheme = settings.customThemeJson != null
      ? TerminalTheme.fromJson(settings.customThemeJson!)
      : defaultTerminalTheme;
  terminalCursorShape = settings.cursor.shape;
  terminalCursorBlink = settings.cursor.blink;
  terminalBellConfig = settings.bell;
  terminalPointerConfig = settings.pointer;
  terminalConfirmOnClose = settings.confirmOnClose;
  hostIconMode = settings.hostIconMode;
  terminalShellPath = settings.shellPath;
  terminalWindowWidth = settings.window.width;
  terminalWindowHeight = settings.window.height;
  terminalWindowX = settings.window.x;
  terminalWindowY = settings.window.y;
  terminalFontConfig = settings.font;
  terminalKeyboardConfig = settings.keyboard;
  terminalShortcutConfig = settings.shortcuts;
  terminalPadding = settings.padding;
  sftpSshEditor = settings.sftp.sshEditor;
  sftpExternalEditor = settings.sftp.externalEditor;
  sftpTextFileExtensions = settings.sftp.textFileExtensions;
  sftpDefaultDownloadDir = settings.sftp.defaultDownloadDir;
  sftpShowHiddenFiles = settings.sftp.showHiddenFiles;
  sftpConcurrentTasks = settings.sftp.concurrentTasks;
  sftpTransferThreads = settings.sftp.transferThreads;
  setSftpTabEnabled(settings.sftp.showTab);
  setWorkspacePageEnabled(settings.workspacePageEnabled);
  terminalRecordingConfig = settings.recording;
  setAiAssistantConfig(settings.aiAssistant);
  setAppThemeMode(settings.appThemeMode);
  setAppLanguage(settings.appLanguage);
}

NautermRuntimeSettings currentNautermRuntimeSettings() {
  return NautermRuntimeSettings(
    copyOnSelect: terminalCopyOnSelect,
    selectCommandBlockOnClick: terminalSelectCommandBlockOnClick,
    font: terminalFontConfig,
    keyboard: terminalKeyboardConfig,
    shortcuts: terminalShortcutConfig,
    sftp: SftpConfig(
      showTab: sftpTabEnabled,
      sshEditor: sftpSshEditor,
      externalEditor: sftpExternalEditor,
      textFileExtensions: sftpTextFileExtensions,
      defaultDownloadDir: sftpDefaultDownloadDir,
      showHiddenFiles: sftpShowHiddenFiles,
      concurrentTasks: sftpConcurrentTasks,
      transferThreads: sftpTransferThreads,
    ),
    appThemeMode: appThemeMode,
    appLanguage: appLanguage,
    aiAssistant: aiAssistantConfig,
    padding: terminalPadding,
    composerEnabled: terminalComposerEnabled,
    autocompleteEnabled: terminalAutocompleteEnabled,
    multiTabEnabled: terminalMultiTabEnabled,
    scrollbarEnabled: terminalScrollbarEnabled,
    scrollbackLines: terminalScrollbackLines,
    sshKeepaliveIntervalSeconds: terminalSshKeepaliveIntervalSeconds,
    sshPredictionMode: terminalSshPredictionMode,
    emulationType: terminalEmulationType,
    emulatorBackend: terminalEmulatorBackend,
    themeId: terminalThemeId,
    customThemeJson: terminalCustomTheme.toJson(),
    cursor: TerminalCursorConfig(
      shape: terminalCursorShape,
      blink: terminalCursorBlink,
    ),
    bell: terminalBellConfig,
    pointer: terminalPointerConfig,
    confirmOnClose: terminalConfirmOnClose,
    hostIconMode: hostIconMode,
    shellPath: terminalShellPath,
    window: WindowConfig(
      width: terminalWindowWidth,
      height: terminalWindowHeight,
      x: terminalWindowX,
      y: terminalWindowY,
    ),
    workspacePageEnabled: workspacePageEnabled,
    recording: terminalRecordingConfig,
  );
}

TerminalConfig currentTerminalConfig() {
  return defaultTerminalConfig.copyWith(
    font: terminalFontConfig,
    keyboard: terminalKeyboardConfig,
    cursor: TerminalCursorConfig(
      shape: terminalCursorShape,
      blink: terminalCursorBlink,
    ),
    composer: TerminalComposerConfig(
      enabled: terminalComposerEnabled,
      autocompleteEnabled: terminalAutocompleteEnabled,
    ),
    copyOnSelect: terminalCopyOnSelect,
    padding: terminalPadding,
    scrollbackLines: terminalScrollbackLines,
    sshKeepaliveIntervalSeconds: terminalSshKeepaliveIntervalSeconds,
    sshPredictionMode: terminalSshPredictionMode,
    emulation: TerminalEmulationConfig(type: terminalEmulationType),
    emulatorBackend: terminalEmulatorBackend,
    moshPredictionMode: terminalMoshPredictionMode,
    osc52Mode: terminalOsc52Mode,
  );
}

@immutable
class TerminalConfig {
  const TerminalConfig({
    this.font = const TerminalFontConfig(),
    this.emulation = const TerminalEmulationConfig(),
    this.emulatorBackend = TerminalEmulatorBackend.alacritty,
    this.cursor = const TerminalCursorConfig(),
    this.keyboard = const TerminalKeyboardConfig(),
    this.composer = const TerminalComposerConfig(),
    this.moshPredictionMode = TerminalMoshPredictionMode.adaptive,
    this.osc52Mode = TerminalOsc52Mode.copy,
    this.padding = EdgeInsets.zero,
    this.scrollbackLines = 10000,
    this.sshKeepaliveIntervalSeconds = 20,
    this.sshPredictionMode = TerminalSshPredictionMode.adaptive,
    this.copyOnSelect = false,
  });

  TerminalConfig copyWith({
    TerminalFontConfig? font,
    TerminalEmulationConfig? emulation,
    TerminalEmulatorBackend? emulatorBackend,
    TerminalCursorConfig? cursor,
    TerminalKeyboardConfig? keyboard,
    TerminalComposerConfig? composer,
    TerminalMoshPredictionMode? moshPredictionMode,
    TerminalOsc52Mode? osc52Mode,
    EdgeInsets? padding,
    int? scrollbackLines,
    int? sshKeepaliveIntervalSeconds,
    TerminalSshPredictionMode? sshPredictionMode,
    bool? copyOnSelect,
  }) {
    return TerminalConfig(
      font: font ?? this.font,
      emulation: emulation ?? this.emulation,
      emulatorBackend: emulatorBackend ?? this.emulatorBackend,
      cursor: cursor ?? this.cursor,
      keyboard: keyboard ?? this.keyboard,
      composer: composer ?? this.composer,
      moshPredictionMode: moshPredictionMode ?? this.moshPredictionMode,
      osc52Mode: osc52Mode ?? this.osc52Mode,
      padding: padding ?? this.padding,
      scrollbackLines: scrollbackLines ?? this.scrollbackLines,
      sshKeepaliveIntervalSeconds:
          sshKeepaliveIntervalSeconds ?? this.sshKeepaliveIntervalSeconds,
      sshPredictionMode: sshPredictionMode ?? this.sshPredictionMode,
      copyOnSelect: copyOnSelect ?? this.copyOnSelect,
    );
  }

  final TerminalFontConfig font;
  final TerminalEmulationConfig emulation;
  final TerminalEmulatorBackend emulatorBackend;
  final TerminalCursorConfig cursor;
  final TerminalKeyboardConfig keyboard;
  final TerminalComposerConfig composer;
  final TerminalMoshPredictionMode moshPredictionMode;
  final TerminalOsc52Mode osc52Mode;
  final EdgeInsets padding;
  final int scrollbackLines;
  final int sshKeepaliveIntervalSeconds;
  final TerminalSshPredictionMode sshPredictionMode;
  final bool copyOnSelect;
}

@immutable
class TerminalFontConfig {
  const TerminalFontConfig({
    this.family = 'monospace',
    this.cjkFamily,
    this.size = 12,
    this.lineHeight = 1.18,
    this.letterSpacing = 0,
    this.enableLigatures = false,
    this.weight = 400,
    this.boldWeight = 700,
  });

  final String family;
  final String? cjkFamily;
  final double size;
  final double lineHeight;
  final double letterSpacing;
  final bool enableLigatures;
  final int weight;
  final int boldWeight;

  TerminalFontConfig copyWith({
    String? family,
    String? cjkFamily,
    bool useAutomaticCjkFont = false,
    double? size,
    double? lineHeight,
    double? letterSpacing,
    bool? enableLigatures,
    int? weight,
    int? boldWeight,
  }) {
    return TerminalFontConfig(
      family: family ?? this.family,
      cjkFamily: useAutomaticCjkFont ? null : cjkFamily ?? this.cjkFamily,
      size: size ?? this.size,
      lineHeight: lineHeight ?? this.lineHeight,
      letterSpacing: letterSpacing ?? this.letterSpacing,
      enableLigatures: enableLigatures ?? this.enableLigatures,
      weight: weight ?? this.weight,
      boldWeight: boldWeight ?? this.boldWeight,
    );
  }

  TextStyle textStyle({
    Color color = terminalDefaultForeground,
    bool bold = false,
  }) {
    final resolvedFontFallback = resolvedFallback();
    return TextStyle(
      color: color,
      fontFamily: resolvedFamily(),
      fontFamilyFallback: resolvedFontFallback.isEmpty
          ? null
          : resolvedFontFallback,
      locale: resolvedLocale(),
      fontSize: size,
      height: lineHeight,
      letterSpacing: letterSpacing,
      fontWeight: FontWeight(bold ? boldWeight : weight),
      fontFeatures: enableLigatures
          ? null
          : const [FontFeature.disable('liga'), FontFeature.disable('calt')],
    );
  }

  /// Resolves the generic 'monospace' placeholder to a concrete font that
  /// ships with each supported desktop platform. Flutter's engine does not
  /// reliably map the bare 'monospace' family name to an actual monospaced
  /// font on every platform (notably Windows, where no such generic alias
  /// exists, and Linux, where resolution depends on the desktop's fontconfig
  /// setup), so a real, broadly available family is used instead.
  String resolvedFamily({bool? windows, bool? linux, bool? macos}) {
    if (family.trim().toLowerCase() != 'monospace') {
      return family;
    }
    if (windows ?? Platform.isWindows) {
      return 'Consolas';
    }
    if (linux ?? Platform.isLinux) {
      return 'DejaVu Sans Mono';
    }
    if (macos ?? Platform.isMacOS) {
      return 'Menlo';
    }
    return family;
  }

  /// Fonts commonly bundled by desktop platforms, used as a last-resort
  /// fallback chain so the terminal still renders with a monospaced font
  /// (instead of silently falling back to a proportional UI font) when the
  /// configured family or the resolved platform default is not installed.
  static const List<String> _platformFallbackFamilies = [
    'Consolas',
    'Menlo',
    'DejaVu Sans Mono',
    'Noto Sans Mono',
    'Liberation Mono',
    'Courier New',
  ];

  List<String> resolvedFallback() {
    final fallback = <String>[];
    final seen = <String>{family.trim().toLowerCase()};

    void addIfNew(String? candidate) {
      final trimmed = candidate?.trim() ?? '';
      if (trimmed.isEmpty || !seen.add(trimmed.toLowerCase())) {
        return;
      }
      fallback.add(trimmed);
    }

    addIfNew(cjkFamily);
    for (final candidate in _platformFallbackFamilies) {
      addIfNew(candidate);
    }
    return List.unmodifiable(fallback);
  }

  Locale resolvedLocale({AppLanguage? language, Locale? systemLocale}) {
    return switch (language ?? appLanguage) {
      AppLanguage.english => const Locale('en'),
      AppLanguage.simplifiedChinese => const Locale('zh', 'CN'),
      AppLanguage.system => systemLocale ?? PlatformDispatcher.instance.locale,
    };
  }
}

enum TerminalType {
  xterm256Color('xterm-256color'),
  xterm16Color('xterm-16color'),
  xterm('xterm');

  const TerminalType(this.term);

  final String term;
}

typedef TerminalEmulationType = TerminalType;

enum TerminalColorTerm { none, truecolor }

@immutable
class TerminalEmulationConfig {
  const TerminalEmulationConfig({
    this.type = TerminalType.xterm256Color,
    this.colorTerm = TerminalColorTerm.truecolor,
  });

  final TerminalType type;
  final TerminalColorTerm colorTerm;
}

@immutable
class TerminalCursorConfig {
  const TerminalCursorConfig({
    this.visible = true,
    this.shape = TerminalCursorShape.block,
    this.blink = true,
    this.blinkInterval = const Duration(milliseconds: 530),
  });

  final bool visible;
  final TerminalCursorShape shape;
  final bool blink;
  final Duration blinkInterval;
}

@immutable
class TerminalBellConfig {
  const TerminalBellConfig({
    this.sound = true,
    this.visual = false,
    this.tabIndicator = true,
  });

  final bool sound;
  final bool visual;
  final bool tabIndicator;

  TerminalBellConfig copyWith({bool? sound, bool? visual, bool? tabIndicator}) {
    return TerminalBellConfig(
      sound: sound ?? this.sound,
      visual: visual ?? this.visual,
      tabIndicator: tabIndicator ?? this.tabIndicator,
    );
  }
}

@immutable
class TerminalPointerConfig {
  const TerminalPointerConfig({
    this.commandClickOpensFilenameOrUrl = true,
    this.optionClickMovesCursor = true,
  });

  final bool commandClickOpensFilenameOrUrl;
  final bool optionClickMovesCursor;

  TerminalPointerConfig copyWith({
    bool? commandClickOpensFilenameOrUrl,
    bool? optionClickMovesCursor,
  }) {
    return TerminalPointerConfig(
      commandClickOpensFilenameOrUrl:
          commandClickOpensFilenameOrUrl ?? this.commandClickOpensFilenameOrUrl,
      optionClickMovesCursor:
          optionClickMovesCursor ?? this.optionClickMovesCursor,
    );
  }
}

@immutable
class TerminalKeyboardConfig {
  const TerminalKeyboardConfig({
    this.useOptionAsMetaKey = true,
    this.reportMouseEvents = true,
    this.navigationKeysScrollOutsideInteractiveApps = true,
  });

  TerminalKeyboardConfig copyWith({
    bool? useOptionAsMetaKey,
    bool? reportMouseEvents,
    bool? navigationKeysScrollOutsideInteractiveApps,
  }) {
    return TerminalKeyboardConfig(
      useOptionAsMetaKey: useOptionAsMetaKey ?? this.useOptionAsMetaKey,
      reportMouseEvents: reportMouseEvents ?? this.reportMouseEvents,
      navigationKeysScrollOutsideInteractiveApps:
          navigationKeysScrollOutsideInteractiveApps ??
          this.navigationKeysScrollOutsideInteractiveApps,
    );
  }

  final bool useOptionAsMetaKey;
  final bool reportMouseEvents;
  final bool navigationKeysScrollOutsideInteractiveApps;
}

LogicalKeyboardKey? keyFromName(String name) {
  return switch (name) {
    // Letters
    'a' => LogicalKeyboardKey.keyA,
    'b' => LogicalKeyboardKey.keyB,
    'c' => LogicalKeyboardKey.keyC,
    'd' => LogicalKeyboardKey.keyD,
    'e' => LogicalKeyboardKey.keyE,
    'f' => LogicalKeyboardKey.keyF,
    'g' => LogicalKeyboardKey.keyG,
    'h' => LogicalKeyboardKey.keyH,
    'i' => LogicalKeyboardKey.keyI,
    'j' => LogicalKeyboardKey.keyJ,
    'k' => LogicalKeyboardKey.keyK,
    'l' => LogicalKeyboardKey.keyL,
    'm' => LogicalKeyboardKey.keyM,
    'n' => LogicalKeyboardKey.keyN,
    'o' => LogicalKeyboardKey.keyO,
    'p' => LogicalKeyboardKey.keyP,
    'q' => LogicalKeyboardKey.keyQ,
    'r' => LogicalKeyboardKey.keyR,
    's' => LogicalKeyboardKey.keyS,
    't' => LogicalKeyboardKey.keyT,
    'u' => LogicalKeyboardKey.keyU,
    'v' => LogicalKeyboardKey.keyV,
    'w' => LogicalKeyboardKey.keyW,
    'x' => LogicalKeyboardKey.keyX,
    'y' => LogicalKeyboardKey.keyY,
    'z' => LogicalKeyboardKey.keyZ,
    // Digits
    '0' => LogicalKeyboardKey.digit0,
    '1' => LogicalKeyboardKey.digit1,
    '2' => LogicalKeyboardKey.digit2,
    '3' => LogicalKeyboardKey.digit3,
    '4' => LogicalKeyboardKey.digit4,
    '5' => LogicalKeyboardKey.digit5,
    '6' => LogicalKeyboardKey.digit6,
    '7' => LogicalKeyboardKey.digit7,
    '8' => LogicalKeyboardKey.digit8,
    '9' => LogicalKeyboardKey.digit9,
    // Brackets & punctuation
    '[' => LogicalKeyboardKey.bracketLeft,
    ']' => LogicalKeyboardKey.bracketRight,
    '/' => LogicalKeyboardKey.slash,
    '\\' => LogicalKeyboardKey.backslash,
    ';' => LogicalKeyboardKey.semicolon,
    "'" => LogicalKeyboardKey.quote,
    ',' => LogicalKeyboardKey.comma,
    '.' => LogicalKeyboardKey.period,
    '-' => LogicalKeyboardKey.minus,
    '=' => LogicalKeyboardKey.equal,
    '`' => LogicalKeyboardKey.backquote,
    // Arrow keys
    'right' => LogicalKeyboardKey.arrowRight,
    'down' => LogicalKeyboardKey.arrowDown,
    'left' => LogicalKeyboardKey.arrowLeft,
    'up' => LogicalKeyboardKey.arrowUp,
    // Navigation
    'home' => LogicalKeyboardKey.home,
    'end' => LogicalKeyboardKey.end,
    'pageup' => LogicalKeyboardKey.pageUp,
    'pagedown' => LogicalKeyboardKey.pageDown,
    // Special keys
    'space' => LogicalKeyboardKey.space,
    'enter' => LogicalKeyboardKey.enter,
    'tab' => LogicalKeyboardKey.tab,
    'backspace' => LogicalKeyboardKey.backspace,
    'delete' => LogicalKeyboardKey.delete,
    // Function keys
    'f1' => LogicalKeyboardKey.f1,
    'f2' => LogicalKeyboardKey.f2,
    'f3' => LogicalKeyboardKey.f3,
    'f4' => LogicalKeyboardKey.f4,
    'f5' => LogicalKeyboardKey.f5,
    'f6' => LogicalKeyboardKey.f6,
    'f7' => LogicalKeyboardKey.f7,
    'f8' => LogicalKeyboardKey.f8,
    'f9' => LogicalKeyboardKey.f9,
    'f10' => LogicalKeyboardKey.f10,
    'f11' => LogicalKeyboardKey.f11,
    'f12' => LogicalKeyboardKey.f12,
    _ => null,
  };
}

Iterable<LogicalKeyboardKey> shortcutKeysFromName(
  String name, {
  required bool shift,
}) sync* {
  final key = keyFromName(name);
  if (key == null) {
    return;
  }
  yield key;
  if (!shift) {
    return;
  }
  if (key == LogicalKeyboardKey.bracketLeft) {
    yield LogicalKeyboardKey.braceLeft;
  } else if (key == LogicalKeyboardKey.bracketRight) {
    yield LogicalKeyboardKey.braceRight;
  }
}

bool shortcutKeyMatches(
  LogicalKeyboardKey key,
  String name, {
  required bool shift,
}) {
  return shortcutKeysFromName(name, shift: shift).contains(key);
}

String? formatShortcutForPlatform(
  String value, {
  TargetPlatform? platform,
  bool compact = true,
}) {
  if (value.trim().isEmpty) {
    return null;
  }
  final effectivePlatform = platform ?? defaultTargetPlatform;
  final isMacOS = effectivePlatform == TargetPlatform.macOS;
  final parts = value.toLowerCase().split('+').map((part) => part.trim());
  final modifiers = parts.where(
    (part) => part == 'cmd' || part == 'alt' || part == 'shift',
  );
  final key = parts.lastWhere(
    (part) => part != 'cmd' && part != 'alt' && part != 'shift',
    orElse: () => '',
  );
  final labels = <String>[
    for (final modifier in modifiers)
      switch (modifier) {
        'cmd' => isMacOS ? (compact ? '⌘' : 'Cmd') : 'Ctrl',
        'alt' => isMacOS ? (compact ? '⌥' : 'Option') : 'Alt',
        'shift' => isMacOS && compact ? '⇧' : 'Shift',
        _ => modifier,
      },
    if (key.isNotEmpty)
      switch (key) {
        'right' => '→',
        'down' => '↓',
        'left' => '←',
        'up' => '↑',
        'enter' => '↵',
        'backspace' => isMacOS && compact ? '⌫' : 'Backspace',
        'delete' => isMacOS && compact ? '⌦' : 'Delete',
        'space' => 'Space',
        'pageup' => 'Page Up',
        'pagedown' => 'Page Down',
        _ => key.toUpperCase(),
      },
  ];
  if (labels.isEmpty) {
    return null;
  }
  if (isMacOS && compact) {
    return labels.join();
  }
  return labels.join(compact ? '+' : ' ');
}

String? formatTerminalEditingShortcutForPlatform(
  String value, {
  TargetPlatform? platform,
  bool compact = true,
}) {
  final effectivePlatform = platform ?? defaultTargetPlatform;
  if (effectivePlatform == TargetPlatform.macOS || value.trim().isEmpty) {
    return formatShortcutForPlatform(
      value,
      platform: effectivePlatform,
      compact: compact,
    );
  }
  final parts = value.toLowerCase().split('+').map((part) => part.trim());
  final normalized = <String>[
    for (final part in parts)
      if (part != 'shift') part,
  ];
  if (normalized.isEmpty) {
    return null;
  }
  final key = normalized.removeLast();
  normalized
    ..add('shift')
    ..add(key);
  return formatShortcutForPlatform(
    normalized.join('+'),
    platform: effectivePlatform,
    compact: compact,
  );
}

bool shortcutMatchesEvent(
  String value,
  KeyEvent event, {
  TargetPlatform? platform,
}) {
  if (value.trim().isEmpty) {
    return false;
  }
  final parts = value.toLowerCase().split('+').map((part) => part.trim());
  final keyName = parts.lastWhere(
    (part) => part != 'cmd' && part != 'alt' && part != 'shift',
    orElse: () => '',
  );
  final needsShift = parts.contains('shift');
  if (!shortcutKeyMatches(event.logicalKey, keyName, shift: needsShift)) {
    return false;
  }
  final keyboard = HardwareKeyboard.instance;
  final isMacOS = (platform ?? defaultTargetPlatform) == TargetPlatform.macOS;
  return keyboard.isMetaPressed == isMacOS &&
      keyboard.isControlPressed == !isMacOS &&
      keyboard.isAltPressed == parts.contains('alt') &&
      keyboard.isShiftPressed == needsShift;
}

@immutable
class TerminalShortcutConfig {
  const TerminalShortcutConfig({
    this.quickConnect = 'cmd+t',
    this.commandPalette = 'cmd+shift+p',
    this.switchToSsh = 'cmd+shift+t',
    this.switchToSftp = 'cmd+shift+s',
    this.previousTab = 'cmd+shift+[',
    this.nextTab = 'cmd+shift+]',
    this.closeTab = 'cmd+w',
    this.selectAll = 'cmd+a',
    this.copy = 'cmd+c',
    this.paste = 'cmd+v',
    this.search = 'cmd+f',
    this.splitRight = 'cmd+alt+right',
    this.splitDown = 'cmd+alt+down',
    this.newLocalTerminal = 'cmd+l',
    this.openSettings = 'cmd+,',
    this.editWorkspaceItem = 'cmd+e',
    this.duplicateWorkspaceItem = 'cmd+d',
    this.tabSwitches = const [
      'cmd+1',
      'cmd+2',
      'cmd+3',
      'cmd+4',
      'cmd+5',
      'cmd+6',
      'cmd+7',
      'cmd+8',
      'cmd+9',
    ],
  });

  final String quickConnect;
  final String commandPalette;
  final String switchToSsh;
  final String switchToSftp;
  final String previousTab;
  final String nextTab;
  final String closeTab;
  final String selectAll;
  final String copy;
  final String paste;
  final String search;
  final String splitRight;
  final String splitDown;
  final String newLocalTerminal;
  final String openSettings;
  final String editWorkspaceItem;
  final String duplicateWorkspaceItem;
  final List<String> tabSwitches;

  TerminalShortcutConfig copyWith({
    String? quickConnect,
    String? commandPalette,
    String? switchToSsh,
    String? switchToSftp,
    String? previousTab,
    String? nextTab,
    String? closeTab,
    String? selectAll,
    String? copy,
    String? paste,
    String? search,
    String? splitRight,
    String? splitDown,
    String? newLocalTerminal,
    String? openSettings,
    String? editWorkspaceItem,
    String? duplicateWorkspaceItem,
    List<String>? tabSwitches,
  }) {
    return TerminalShortcutConfig(
      quickConnect: quickConnect ?? this.quickConnect,
      commandPalette: commandPalette ?? this.commandPalette,
      switchToSsh: switchToSsh ?? this.switchToSsh,
      switchToSftp: switchToSftp ?? this.switchToSftp,
      previousTab: previousTab ?? this.previousTab,
      nextTab: nextTab ?? this.nextTab,
      closeTab: closeTab ?? this.closeTab,
      selectAll: selectAll ?? this.selectAll,
      copy: copy ?? this.copy,
      paste: paste ?? this.paste,
      search: search ?? this.search,
      splitRight: splitRight ?? this.splitRight,
      splitDown: splitDown ?? this.splitDown,
      newLocalTerminal: newLocalTerminal ?? this.newLocalTerminal,
      openSettings: openSettings ?? this.openSettings,
      editWorkspaceItem: editWorkspaceItem ?? this.editWorkspaceItem,
      duplicateWorkspaceItem:
          duplicateWorkspaceItem ?? this.duplicateWorkspaceItem,
      tabSwitches: tabSwitches ?? this.tabSwitches,
    );
  }

  Map<String, Object?> toJson() {
    return <String, Object?>{
      'quickConnect': quickConnect,
      'commandPalette': commandPalette,
      'switchToSsh': switchToSsh,
      'switchToSftp': switchToSftp,
      'previousTab': previousTab,
      'nextTab': nextTab,
      'closeTab': closeTab,
      'selectAll': selectAll,
      'copy': copy,
      'paste': paste,
      'search': search,
      'splitRight': splitRight,
      'splitDown': splitDown,
      'newLocalTerminal': newLocalTerminal,
      'openSettings': openSettings,
      'editWorkspaceItem': editWorkspaceItem,
      'duplicateWorkspaceItem': duplicateWorkspaceItem,
      'tabSwitches': tabSwitches,
    };
  }

  static String _normalizeShortcut(String value) {
    return value
        .toLowerCase()
        .replaceAll('command', 'cmd')
        .replaceAll('ctrl', 'cmd')
        .replaceAll('option', 'alt')
        .split('+')
        .map((part) => part.trim())
        .join('+');
  }

  static TerminalShortcutConfig fromJson(Object? value) {
    if (value is! Map) return const TerminalShortcutConfig();
    final map = value.cast<String, Object?>();
    String s(String key, String fallback) {
      final value = map[key];
      if (value is! String) return fallback;
      final raw = value.trim();
      if (raw.isEmpty) return '';
      return _normalizeShortcut(raw);
    }

    const d = TerminalShortcutConfig();
    final rawTabSwitches = map['tabSwitches'];
    final tabSwitches = rawTabSwitches is List
        ? [
            for (var i = 0; i < 9; i++)
              i < rawTabSwitches.length
                  ? rawTabSwitches[i] is String
                        ? (rawTabSwitches[i] as String).trim().isEmpty
                              ? ''
                              : _normalizeShortcut(
                                  (rawTabSwitches[i] as String).trim(),
                                )
                        : d.tabSwitches[i]
                  : d.tabSwitches[i],
          ]
        : d.tabSwitches;
    return TerminalShortcutConfig(
      quickConnect: s('quickConnect', d.quickConnect),
      commandPalette: s('commandPalette', d.commandPalette),
      switchToSsh: s('switchToSsh', d.switchToSsh),
      switchToSftp: s('switchToSftp', d.switchToSftp),
      previousTab: s('previousTab', d.previousTab),
      nextTab: s('nextTab', d.nextTab),
      closeTab: s('closeTab', d.closeTab),
      selectAll: s('selectAll', d.selectAll),
      copy: s('copy', d.copy),
      paste: s('paste', d.paste),
      search: s('search', d.search),
      splitRight: s('splitRight', d.splitRight),
      splitDown: s('splitDown', d.splitDown),
      newLocalTerminal: s('newLocalTerminal', d.newLocalTerminal),
      openSettings: s('openSettings', d.openSettings),
      editWorkspaceItem: s('editWorkspaceItem', d.editWorkspaceItem),
      duplicateWorkspaceItem: s(
        'duplicateWorkspaceItem',
        d.duplicateWorkspaceItem,
      ),
      tabSwitches: tabSwitches,
    );
  }

  static LogicalKeyboardKey? keyFromName(String name) {
    return switch (name) {
      'a' => LogicalKeyboardKey.keyA,
      'b' => LogicalKeyboardKey.keyB,
      'c' => LogicalKeyboardKey.keyC,
      'd' => LogicalKeyboardKey.keyD,
      'e' => LogicalKeyboardKey.keyE,
      'f' => LogicalKeyboardKey.keyF,
      'g' => LogicalKeyboardKey.keyG,
      'h' => LogicalKeyboardKey.keyH,
      'i' => LogicalKeyboardKey.keyI,
      'j' => LogicalKeyboardKey.keyJ,
      'k' => LogicalKeyboardKey.keyK,
      'l' => LogicalKeyboardKey.keyL,
      'm' => LogicalKeyboardKey.keyM,
      'n' => LogicalKeyboardKey.keyN,
      'o' => LogicalKeyboardKey.keyO,
      'p' => LogicalKeyboardKey.keyP,
      'q' => LogicalKeyboardKey.keyQ,
      'r' => LogicalKeyboardKey.keyR,
      's' => LogicalKeyboardKey.keyS,
      't' => LogicalKeyboardKey.keyT,
      'u' => LogicalKeyboardKey.keyU,
      'v' => LogicalKeyboardKey.keyV,
      'w' => LogicalKeyboardKey.keyW,
      'x' => LogicalKeyboardKey.keyX,
      'y' => LogicalKeyboardKey.keyY,
      'z' => LogicalKeyboardKey.keyZ,
      '[' => LogicalKeyboardKey.bracketLeft,
      ']' => LogicalKeyboardKey.bracketRight,
      '/' => LogicalKeyboardKey.slash,
      '1' => LogicalKeyboardKey.digit1,
      '2' => LogicalKeyboardKey.digit2,
      '3' => LogicalKeyboardKey.digit3,
      '4' => LogicalKeyboardKey.digit4,
      '5' => LogicalKeyboardKey.digit5,
      '6' => LogicalKeyboardKey.digit6,
      '7' => LogicalKeyboardKey.digit7,
      '8' => LogicalKeyboardKey.digit8,
      '9' => LogicalKeyboardKey.digit9,
      'right' => LogicalKeyboardKey.arrowRight,
      'down' => LogicalKeyboardKey.arrowDown,
      'left' => LogicalKeyboardKey.arrowLeft,
      'up' => LogicalKeyboardKey.arrowUp,
      _ => null,
    };
  }

  String? _keyPart(String shortcut) {
    final parts = shortcut.split('+');
    return parts.isNotEmpty ? parts.last.trim() : null;
  }

  bool _matchesKey(String shortcut, LogicalKeyboardKey key) {
    final keyName = _keyPart(shortcut);
    if (keyName == null) return false;
    return keyFromName(keyName) == key;
  }

  bool _hasShift(String shortcut) {
    return shortcut.split('+').contains('shift');
  }

  bool _hasAlt(String shortcut) {
    return shortcut.split('+').contains('alt');
  }

  bool matchesCopy(
    LogicalKeyboardKey key, {
    required bool shift,
    bool alt = false,
  }) {
    return _matchesKey(copy, key) &&
        shift == _hasShift(copy) &&
        alt == _hasAlt(copy);
  }

  bool matchesCopyKey(LogicalKeyboardKey key, {bool alt = false}) {
    return _matchesKey(copy, key) && alt == _hasAlt(copy);
  }

  bool matchesPaste(
    LogicalKeyboardKey key, {
    required bool shift,
    bool alt = false,
  }) {
    return _matchesKey(paste, key) &&
        shift == _hasShift(paste) &&
        alt == _hasAlt(paste);
  }

  bool matchesPasteKey(LogicalKeyboardKey key, {bool alt = false}) {
    return _matchesKey(paste, key) && alt == _hasAlt(paste);
  }

  bool matchesSelectAll(
    LogicalKeyboardKey key, {
    required bool shift,
    bool alt = false,
  }) {
    return _matchesKey(selectAll, key) &&
        shift == _hasShift(selectAll) &&
        alt == _hasAlt(selectAll);
  }

  bool matchesSelectAllKey(LogicalKeyboardKey key, {bool alt = false}) {
    return _matchesKey(selectAll, key) && alt == _hasAlt(selectAll);
  }

  bool matchesSearch(
    LogicalKeyboardKey key, {
    required bool shift,
    bool alt = false,
  }) {
    return _matchesKey(search, key) &&
        shift == _hasShift(search) &&
        alt == _hasAlt(search);
  }

  bool matchesSearchKey(LogicalKeyboardKey key, {bool alt = false}) {
    return _matchesKey(search, key) && alt == _hasAlt(search);
  }
}

@immutable
class TerminalComposerConfig {
  const TerminalComposerConfig({
    this.enabled = true,
    this.autocompleteEnabled = false,
    this.minLines = 1,
    this.maxLines = 3,
    this.maxSuggestions = 999,
    this.placeholder = 'Command',
  });

  final bool enabled;
  final bool autocompleteEnabled;
  final int minLines;
  final int maxLines;
  final int maxSuggestions;
  final String placeholder;
}
