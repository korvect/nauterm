import 'package:flutter/foundation.dart';
import 'package:flutter/painting.dart';

import '../ai/ai_config.dart';
import '../app/nauterm_localizations.dart';
import '../terminal/terminal_config.dart';
import '../terminal/terminal_models.dart';
import '../terminal/terminal_recording_config.dart';
import '../terminal/terminal_theme.dart';

const int nautermConfigSchemaVersion = 1;
const int nautermDefaultSyncIntervalMilliseconds = 3 * 24 * 60 * 60 * 1000;

@immutable
class NautermConfig {
  const NautermConfig({
    this.schemaVersion = nautermConfigSchemaVersion,
    this.appearance = const NautermAppearanceConfig(),
    this.window = const WindowConfig(),
    this.tabs = const NautermTabsConfig(),
    this.terminal = const NautermTerminalConfig(),
    this.ssh = const NautermSshConfig(),
    this.sftp = const SftpConfig(),
    this.workspace = const NautermWorkspaceConfig(),
    this.shortcuts = const TerminalShortcutConfig(),
    this.ai = const NautermAiConfig(),
    this.recording = const NautermRecordingConfig(),
    this.sync = const NautermSyncConfig(),
  });

  final int schemaVersion;
  final NautermAppearanceConfig appearance;
  final WindowConfig window;
  final NautermTabsConfig tabs;
  final NautermTerminalConfig terminal;
  final NautermSshConfig ssh;
  final SftpConfig sftp;
  final NautermWorkspaceConfig workspace;
  final TerminalShortcutConfig shortcuts;
  final NautermAiConfig ai;
  final NautermRecordingConfig recording;
  final NautermSyncConfig sync;

  factory NautermConfig.fromJson(Object? value) {
    final json = _map(value);
    final schemaVersion = (json['schemaVersion'] as num?)?.toInt();
    if (schemaVersion != nautermConfigSchemaVersion) {
      throw FormatException(
        'Unsupported config schema version: $schemaVersion',
      );
    }
    return NautermConfig(
      schemaVersion: nautermConfigSchemaVersion,
      appearance: NautermAppearanceConfig.fromJson(json['appearance']),
      window: _windowFromJson(json['window']),
      tabs: NautermTabsConfig.fromJson(json['tabs']),
      terminal: NautermTerminalConfig.fromJson(json['terminal']),
      ssh: NautermSshConfig.fromJson(json['ssh']),
      sftp: _sftpFromJson(json['sftp']),
      workspace: NautermWorkspaceConfig.fromJson(json['workspace']),
      shortcuts: TerminalShortcutConfig.fromJson(json['shortcuts']),
      ai: NautermAiConfig.fromJson(json['ai']),
      recording: NautermRecordingConfig.fromJson(json['recording']),
      sync: NautermSyncConfig.fromJson(json['sync']),
    );
  }

  NautermConfig copyWith({
    NautermTerminalConfig? terminal,
    NautermSyncConfig? sync,
  }) {
    return NautermConfig(
      schemaVersion: schemaVersion,
      appearance: appearance,
      window: window,
      tabs: tabs,
      terminal: terminal ?? this.terminal,
      ssh: ssh,
      sftp: sftp,
      workspace: workspace,
      shortcuts: shortcuts,
      ai: ai,
      recording: recording,
      sync: sync ?? this.sync,
    );
  }

  Map<String, Object?> toJson() {
    return <String, Object?>{
      'schemaVersion': schemaVersion,
      'appearance': appearance.toJson(),
      'window': _windowToJson(window),
      'tabs': tabs.toJson(),
      'terminal': terminal.toJson(),
      'ssh': ssh.toJson(),
      'sftp': _sftpToJson(sftp),
      'workspace': workspace.toJson(),
      'shortcuts': shortcuts.toJson(),
      'ai': ai.toJson(),
      'recording': recording.toJson(),
      'sync': sync.toJson(),
    };
  }
}

@immutable
class NautermSyncConfig {
  const NautermSyncConfig({
    this.mergeStrategy = 'smart_merge',
    this.automatic = false,
    this.interval = nautermDefaultSyncIntervalMilliseconds,
    this.backupCount = 10,
  });

  final String mergeStrategy;
  final bool automatic;
  final int interval;
  final int backupCount;

  factory NautermSyncConfig.fromJson(Object? value) {
    final json = _map(value);
    final strategy = json['mergeStrategy'] as String?;
    final interval =
        (json['interval'] as num?)?.toInt() ??
        nautermDefaultSyncIntervalMilliseconds;
    return NautermSyncConfig(
      mergeStrategy: switch (strategy) {
        'local_wins' || 'remote_wins' => strategy!,
        'smart_merge' => 'smart_merge',
        _ => 'smart_merge',
      },
      automatic: json['automatic'] as bool? ?? false,
      interval: interval.clamp(300000, 604800000),
      backupCount: ((json['backupCount'] as num?)?.toInt() ?? 10).clamp(1, 100),
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'mergeStrategy': mergeStrategy,
    'automatic': automatic,
    'interval': interval,
    'backupCount': backupCount,
  };
}

@immutable
class NautermAppearanceConfig {
  const NautermAppearanceConfig({
    this.theme = AppThemeMode.light,
    this.hostIcon = HostIconMode.osBadge,
    this.language = AppLanguage.system,
  });

  final AppThemeMode theme;
  final HostIconMode hostIcon;
  final AppLanguage language;

  factory NautermAppearanceConfig.fromJson(Object? value) {
    final json = _map(value);
    return NautermAppearanceConfig(
      theme: AppThemeMode.fromString(json['theme'] as String?),
      hostIcon: HostIconMode.fromString(json['hostIcon'] as String?),
      language: AppLanguage.fromString(json['language'] as String?),
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'theme': theme.name,
    'hostIcon': hostIcon.configValue,
    'language': language.configValue,
  };
}

@immutable
class NautermTabsConfig {
  const NautermTabsConfig({this.confirmOnClose = true});

  final bool confirmOnClose;

  factory NautermTabsConfig.fromJson(Object? value) {
    final json = _map(value);
    return NautermTabsConfig(
      confirmOnClose: json['confirmOnClose'] as bool? ?? true,
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'confirmOnClose': confirmOnClose,
  };
}

@immutable
class NautermTerminalConfig {
  const NautermTerminalConfig({
    this.shellPath,
    this.emulation = TerminalType.xterm256Color,
    this.emulationEngine = TerminalEmulatorBackend.ghostty,
    this.appearance = const NautermTerminalAppearanceConfig(),
    this.behavior = const NautermTerminalBehaviorConfig(),
    this.features = const NautermTerminalFeaturesConfig(),
    this.input = const TerminalKeyboardConfig(),
  });

  final String? shellPath;
  final TerminalType emulation;
  final TerminalEmulatorBackend emulationEngine;
  final NautermTerminalAppearanceConfig appearance;
  final NautermTerminalBehaviorConfig behavior;
  final NautermTerminalFeaturesConfig features;
  final TerminalKeyboardConfig input;

  factory NautermTerminalConfig.fromJson(Object? value) {
    final json = _map(value);
    final shellPath = (json['shellPath'] as String?)?.trim();
    return NautermTerminalConfig(
      shellPath: shellPath == null || shellPath.isEmpty ? null : shellPath,
      emulation: _terminalType(json['emulation']),
      emulationEngine: TerminalEmulatorBackend.fromString(
        json['emulationEngine'] as String?,
      ),
      appearance: NautermTerminalAppearanceConfig.fromJson(json['appearance']),
      behavior: NautermTerminalBehaviorConfig.fromJson(json['behavior']),
      features: NautermTerminalFeaturesConfig.fromJson(json['features']),
      input: _keyboardFromJson(json['input']),
    );
  }

  NautermTerminalConfig copyWith({
    NautermTerminalAppearanceConfig? appearance,
  }) {
    return NautermTerminalConfig(
      shellPath: shellPath,
      emulation: emulation,
      emulationEngine: emulationEngine,
      appearance: appearance ?? this.appearance,
      behavior: behavior,
      features: features,
      input: input,
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'shellPath': shellPath,
    'emulation': emulation.term,
    'emulationEngine': emulationEngine.name,
    'appearance': appearance.toJson(),
    'behavior': behavior.toJson(),
    'features': features.toJson(),
    'input': _keyboardToJson(input),
  };
}

@immutable
class NautermTerminalAppearanceConfig {
  const NautermTerminalAppearanceConfig({
    this.themeId,
    this.customThemeJson,
    this.font = const TerminalFontConfig(),
    this.cursor = const TerminalCursorConfig(),
    this.scrollbar = true,
    this.padding = EdgeInsets.zero,
  });

  final String? themeId;
  final Map<String, Object?>? customThemeJson;
  final TerminalFontConfig font;
  final TerminalCursorConfig cursor;
  final bool scrollbar;
  final EdgeInsets padding;

  factory NautermTerminalAppearanceConfig.fromJson(Object? value) {
    final json = _map(value);
    final theme = _map(json['theme']);
    final id = (theme['id'] as String?)?.trim();
    final custom = theme['custom'];
    return NautermTerminalAppearanceConfig(
      themeId:
          id == null ||
              id.isEmpty ||
              id == 'default' ||
              id == nysaLightTerminalThemeId
          ? null
          : id,
      customThemeJson: custom is Map ? custom.cast<String, Object?>() : null,
      font: _fontFromJson(json['font']),
      cursor: _cursorFromJson(json['cursor']),
      scrollbar: json['scrollbar'] as bool? ?? true,
      padding: _paddingFromJson(json['padding']),
    );
  }

  NautermTerminalAppearanceConfig copyWith({
    String? themeId,
    Map<String, Object?>? customThemeJson,
    bool clearThemeId = false,
    bool clearCustomTheme = false,
  }) {
    return NautermTerminalAppearanceConfig(
      themeId: clearThemeId ? null : themeId ?? this.themeId,
      customThemeJson: clearCustomTheme
          ? null
          : customThemeJson ?? this.customThemeJson,
      font: font,
      cursor: cursor,
      scrollbar: scrollbar,
      padding: padding,
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'theme': <String, Object?>{
      'id': themeId ?? 'default',
      if (customThemeJson != null) 'custom': customThemeJson,
    },
    'font': _fontToJson(font),
    'cursor': <String, Object?>{
      'shape': cursor.shape.name,
      'blink': cursor.blink,
    },
    'scrollbar': scrollbar,
    'padding': _paddingToJson(padding),
  };
}

@immutable
class NautermTerminalBehaviorConfig {
  const NautermTerminalBehaviorConfig({
    this.copyOnSelect = false,
    this.selectCommandBlockOnClick = true,
    this.scrollbackLines = 10000,
    this.bell = const TerminalBellConfig(),
    this.pointer = const TerminalPointerConfig(),
  });

  final bool copyOnSelect;
  final bool selectCommandBlockOnClick;
  final int scrollbackLines;
  final TerminalBellConfig bell;
  final TerminalPointerConfig pointer;

  factory NautermTerminalBehaviorConfig.fromJson(Object? value) {
    final json = _map(value);
    return NautermTerminalBehaviorConfig(
      copyOnSelect: json['copyOnSelect'] as bool? ?? false,
      selectCommandBlockOnClick:
          json['selectCommandBlockOnClick'] as bool? ?? true,
      scrollbackLines: (json['scrollbackLines'] as num?)?.toInt() ?? 10000,
      bell: _terminalBellFromJson(json['bell']),
      pointer: _terminalPointerFromJson(json['pointer']),
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'copyOnSelect': copyOnSelect,
    'selectCommandBlockOnClick': selectCommandBlockOnClick,
    'scrollbackLines': scrollbackLines,
    'bell': _terminalBellToJson(bell),
    'pointer': _terminalPointerToJson(pointer),
  };
}

TerminalBellConfig _terminalBellFromJson(Object? value) {
  final json = _map(value);
  return TerminalBellConfig(
    sound: json['sound'] as bool? ?? true,
    visual: json['visual'] as bool? ?? false,
    tabIndicator: json['tabIndicator'] as bool? ?? true,
  );
}

Map<String, Object?> _terminalBellToJson(TerminalBellConfig bell) => {
  'sound': bell.sound,
  'visual': bell.visual,
  'tabIndicator': bell.tabIndicator,
};

TerminalPointerConfig _terminalPointerFromJson(Object? value) {
  final json = _map(value);
  return TerminalPointerConfig(
    commandClickOpensFilenameOrUrl:
        json['commandClickOpensFilenameOrUrl'] as bool? ?? true,
    optionClickMovesCursor: json['optionClickMovesCursor'] as bool? ?? true,
  );
}

Map<String, Object?> _terminalPointerToJson(TerminalPointerConfig pointer) => {
  'commandClickOpensFilenameOrUrl': pointer.commandClickOpensFilenameOrUrl,
  'optionClickMovesCursor': pointer.optionClickMovesCursor,
};

@immutable
class NautermTerminalFeaturesConfig {
  const NautermTerminalFeaturesConfig({
    this.composer = true,
    this.autocomplete = false,
    this.multiTab = false,
  });

  final bool composer;
  final bool autocomplete;
  final bool multiTab;

  factory NautermTerminalFeaturesConfig.fromJson(Object? value) {
    final json = _map(value);
    return NautermTerminalFeaturesConfig(
      composer: json['composer'] as bool? ?? true,
      autocomplete: json['autocomplete'] as bool? ?? false,
      multiTab: json['multiTab'] as bool? ?? false,
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'composer': composer,
    'autocomplete': autocomplete,
    'multiTab': multiTab,
  };
}

@immutable
class NautermSshConfig {
  const NautermSshConfig({
    this.keepaliveInterval = 20000,
    this.predictionMode = TerminalSshPredictionMode.adaptive,
  });

  final int keepaliveInterval;
  final TerminalSshPredictionMode predictionMode;

  factory NautermSshConfig.fromJson(Object? value) {
    final json = _map(value);
    return NautermSshConfig(
      keepaliveInterval: ((json['keepaliveInterval'] as num?)?.toInt() ?? 20000)
          .clamp(0, maxSshKeepaliveIntervalSeconds * 1000)
          .toInt(),
      predictionMode: TerminalSshPredictionMode.fromString(
        json['predictionMode'] as String?,
      ),
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'keepaliveInterval': keepaliveInterval,
    'predictionMode': predictionMode.name,
  };
}

@immutable
class NautermWorkspaceConfig {
  const NautermWorkspaceConfig({this.enabled = true});

  final bool enabled;

  factory NautermWorkspaceConfig.fromJson(Object? value) {
    final json = _map(value);
    return NautermWorkspaceConfig(enabled: json['enabled'] as bool? ?? true);
  }

  Map<String, Object?> toJson() => <String, Object?>{'enabled': enabled};
}

@immutable
class NautermAiConfig {
  const NautermAiConfig({
    this.presetsUrl,
    this.includeTerminalSelection = true,
    this.includeRecentTerminalOutput = true,
  });

  final String? presetsUrl;
  final bool includeTerminalSelection;
  final bool includeRecentTerminalOutput;

  factory NautermAiConfig.fromJson(Object? value) {
    final json = _map(value);
    final presetsUrl = (json['presetsUrl'] as String?)?.trim();
    return NautermAiConfig(
      presetsUrl: presetsUrl == null || presetsUrl.isEmpty ? null : presetsUrl,
      includeTerminalSelection:
          json['includeTerminalSelection'] as bool? ?? true,
      includeRecentTerminalOutput:
          json['includeRecentTerminalOutput'] as bool? ?? true,
    );
  }

  AiAssistantConfig toAssistantConfig() => AiAssistantConfig(
    includeTerminalSelection: includeTerminalSelection,
    includeRecentTerminalOutput: includeRecentTerminalOutput,
  );

  Map<String, Object?> toJson() => <String, Object?>{
    if (presetsUrl != null) 'presetsUrl': presetsUrl,
    'includeTerminalSelection': includeTerminalSelection,
    'includeRecentTerminalOutput': includeRecentTerminalOutput,
  };
}

Map<String, Object?> _map(Object? value) {
  if (value is Map) return value.cast<String, Object?>();
  return <String, Object?>{};
}

WindowConfig _windowFromJson(Object? value) {
  final json = _map(value);
  double? dimension(String key) {
    final value = json[key];
    return value is num && value.toDouble().isFinite ? value.toDouble() : null;
  }

  return WindowConfig(
    width: dimension('width'),
    height: dimension('height'),
    x: dimension('x'),
    y: dimension('y'),
  );
}

Map<String, Object?> _windowToJson(WindowConfig value) => <String, Object?>{
  if (value.width != null) 'width': value.width,
  if (value.height != null) 'height': value.height,
  if (value.x != null) 'x': value.x,
  if (value.y != null) 'y': value.y,
};

TerminalType _terminalType(Object? value) => switch (value) {
  'xterm-16color' => TerminalType.xterm16Color,
  'xterm' => TerminalType.xterm,
  _ => TerminalType.xterm256Color,
};

TerminalFontConfig _fontFromJson(Object? value) {
  final json = _map(value);
  return TerminalFontConfig(
    family: (json['family'] as String?)?.trim().isNotEmpty == true
        ? (json['family'] as String).trim()
        : terminalFontConfig.family,
    cjkFamily: (json['cjkFamily'] as String?)?.trim().isNotEmpty == true
        ? (json['cjkFamily'] as String).trim()
        : null,
    size: (json['size'] as num?)?.toDouble() ?? 12,
    lineHeight:
        (json['lineHeight'] as num?)?.toDouble() ??
        terminalFontConfig.lineHeight,
    letterSpacing:
        (json['letterSpacing'] as num?)?.toDouble() ??
        terminalFontConfig.letterSpacing,
    enableLigatures:
        json['enableLigatures'] as bool? ?? terminalFontConfig.enableLigatures,
    weight: (json['weight'] as num?)?.toInt() ?? terminalFontConfig.weight,
    boldWeight:
        (json['boldWeight'] as num?)?.toInt() ?? terminalFontConfig.boldWeight,
  );
}

Map<String, Object?> _fontToJson(TerminalFontConfig value) => <String, Object?>{
  'family': value.family,
  'cjkFamily': value.cjkFamily,
  'size': value.size,
  'lineHeight': value.lineHeight,
  'letterSpacing': value.letterSpacing,
  'enableLigatures': value.enableLigatures,
  'weight': value.weight,
  'boldWeight': value.boldWeight,
};

TerminalCursorConfig _cursorFromJson(Object? value) {
  final json = _map(value);
  final shape = TerminalCursorShape.values.firstWhere(
    (candidate) => candidate.name == json['shape'],
    orElse: () => TerminalCursorShape.block,
  );
  return TerminalCursorConfig(
    shape: shape,
    blink: json['blink'] as bool? ?? true,
  );
}

EdgeInsets _paddingFromJson(Object? value) {
  final json = _map(value);
  return EdgeInsets.fromLTRB(
    (json['left'] as num?)?.toDouble() ?? 0,
    (json['top'] as num?)?.toDouble() ?? 0,
    (json['right'] as num?)?.toDouble() ?? 0,
    (json['bottom'] as num?)?.toDouble() ?? 0,
  );
}

Map<String, Object?> _paddingToJson(EdgeInsets value) => <String, Object?>{
  'left': value.left,
  'top': value.top,
  'right': value.right,
  'bottom': value.bottom,
};

TerminalKeyboardConfig _keyboardFromJson(Object? value) {
  final json = _map(value);
  return TerminalKeyboardConfig(
    useOptionAsMetaKey: json['useOptionAsMetaKey'] as bool? ?? true,
    reportMouseEvents: json['reportMouseEvents'] as bool? ?? true,
    navigationKeysScrollOutsideInteractiveApps:
        json['navigationKeysScrollOutsideInteractiveApps'] as bool? ?? true,
  );
}

Map<String, Object?> _keyboardToJson(TerminalKeyboardConfig value) =>
    <String, Object?>{
      'useOptionAsMetaKey': value.useOptionAsMetaKey,
      'reportMouseEvents': value.reportMouseEvents,
      'navigationKeysScrollOutsideInteractiveApps':
          value.navigationKeysScrollOutsideInteractiveApps,
    };

SftpConfig _sftpFromJson(Object? value) {
  final json = _map(value);
  return SftpConfig(
    showTab: json['showTab'] as bool? ?? true,
    sshEditor: (json['sshEditor'] as String?)?.trim().isNotEmpty == true
        ? (json['sshEditor'] as String).trim()
        : 'vim',
    externalEditor: SftpExternalEditorCommand.fromJson(json['externalEditor']),
    textFileExtensions: json['textFileExtensions'] is List
        ? normalizeSftpTextFileExtensions(json['textFileExtensions'] as List)
        : sftpDefaultTextFileExtensions,
    defaultDownloadDir: (json['defaultDownloadDir'] as String?)?.trim(),
    showHiddenFiles: json['showHiddenFiles'] as bool? ?? false,
    concurrentTasks:
        ((json['concurrentTasks'] as num?)?.toInt() ??
                sftpDefaultConcurrentTasks)
            .clamp(1, 8)
            .toInt(),
    transferThreads:
        ((json['transferThreads'] as num?)?.toInt() ??
                sftpDefaultTransferThreads)
            .clamp(1, 32)
            .toInt(),
  );
}

Map<String, Object?> _sftpToJson(SftpConfig value) => <String, Object?>{
  'showTab': value.showTab,
  'sshEditor': value.sshEditor,
  if (value.externalEditor != null)
    'externalEditor': value.externalEditor!.toJson(),
  'textFileExtensions': value.textFileExtensions,
  if (value.defaultDownloadDir != null && value.defaultDownloadDir!.isNotEmpty)
    'defaultDownloadDir': value.defaultDownloadDir,
  'showHiddenFiles': value.showHiddenFiles,
  'concurrentTasks': value.concurrentTasks,
  'transferThreads': value.transferThreads,
};
