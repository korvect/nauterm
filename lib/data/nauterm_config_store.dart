import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../terminal/terminal_config.dart';
import 'nauterm_config.dart';
import 'nauterm_paths.dart';

const String nautermMacOSDefaultConfigAsset =
    'assets/config/default-macos.json';
const String nautermLinuxDefaultConfigAsset =
    'assets/config/default-linux.json';
const String nautermWindowsDefaultConfigAsset =
    'assets/config/default-windows.json';

class NautermConfigStore {
  NautermConfigStore(this.paths, {AssetBundle? bundle})
    : _bundle = bundle ?? rootBundle;

  final NautermPaths paths;
  final AssetBundle _bundle;
  static Future<void> _saveQueue = Future<void>.value();

  Future<void> ensureDefaultConfig({TargetPlatform? platform}) async {
    await paths.ensureCreated();
    final file = paths.configFile;
    if (await file.exists() && await file.length() > 0) return;

    final text = await _bundle.loadString(
      defaultConfigAssetPath(platform ?? defaultTargetPlatform),
    );
    await file.writeAsString(text);
  }

  Future<NautermConfig> loadConfig() async {
    await ensureDefaultConfig();
    return _readConfig();
  }

  Future<NautermRuntimeSettings> loadDefaultRuntimeSettings({
    TargetPlatform? platform,
  }) async {
    final text = await _bundle.loadString(
      defaultConfigAssetPath(platform ?? defaultTargetPlatform),
    );
    return _runtimeSettingsFromConfig(NautermConfig.fromJson(jsonDecode(text)));
  }

  Future<NautermRuntimeSettings> loadRuntimeSettings() async {
    final config = await loadConfig();
    return _runtimeSettingsFromConfig(config);
  }

  NautermRuntimeSettings _runtimeSettingsFromConfig(NautermConfig config) {
    final terminal = config.terminal;
    return NautermRuntimeSettings(
      copyOnSelect: terminal.behavior.copyOnSelect,
      font: terminal.appearance.font,
      keyboard: terminal.input,
      shortcuts: config.shortcuts,
      sftp: config.sftp,
      aiAssistant: config.ai.toAssistantConfig(),
      appThemeMode: config.appearance.theme,
      appLanguage: config.appearance.language,
      padding: terminal.appearance.padding,
      composerEnabled: terminal.features.composer,
      autocompleteEnabled: terminal.features.autocomplete,
      multiTabEnabled: terminal.features.multiTab,
      scrollbarEnabled: terminal.appearance.scrollbar,
      scrollbackLines: terminal.behavior.scrollbackLines,
      sshKeepaliveIntervalSeconds: config.ssh.keepaliveInterval ~/ 1000,
      emulationType: terminal.emulation,
      emulatorBackend: terminal.emulationEngine,
      themeId: terminal.appearance.themeId,
      customThemeJson: terminal.appearance.customThemeJson,
      cursor: terminal.appearance.cursor,
      confirmOnClose: config.tabs.confirmOnClose,
      hostIconMode: config.appearance.hostIcon,
      shellPath: terminal.shellPath,
      window: config.window,
      workspacePageEnabled: config.workspace.enabled,
      recording: config.recording,
    );
  }

  Future<void> saveRuntimeSettings(NautermRuntimeSettings settings) {
    return _enqueueSave(() async {
      final existing = await loadConfig();
      final config = NautermConfig(
        appearance: NautermAppearanceConfig(
          theme: settings.appThemeMode,
          hostIcon: settings.hostIconMode,
          language: settings.appLanguage,
        ),
        window: settings.window,
        tabs: NautermTabsConfig(confirmOnClose: settings.confirmOnClose),
        terminal: NautermTerminalConfig(
          shellPath: settings.shellPath,
          emulation: settings.emulationType,
          emulationEngine: settings.emulatorBackend,
          appearance: NautermTerminalAppearanceConfig(
            themeId: settings.themeId,
            customThemeJson: settings.customThemeJson,
            font: settings.font,
            cursor: settings.cursor,
            scrollbar: settings.scrollbarEnabled,
            padding: settings.padding,
          ),
          behavior: NautermTerminalBehaviorConfig(
            copyOnSelect: settings.copyOnSelect,
            scrollbackLines: settings.scrollbackLines,
          ),
          features: NautermTerminalFeaturesConfig(
            composer: settings.composerEnabled,
            autocomplete: settings.autocompleteEnabled,
            multiTab: settings.multiTabEnabled,
          ),
          input: settings.keyboard,
        ),
        ssh: NautermSshConfig(
          keepaliveInterval: settings.sshKeepaliveIntervalSeconds * 1000,
        ),
        sftp: settings.sftp,
        workspace: NautermWorkspaceConfig(
          enabled: settings.workspacePageEnabled,
        ),
        shortcuts: settings.shortcuts,
        ai: NautermAiConfig(
          presetsUrl: existing.ai.presetsUrl,
          includeTerminalSelection:
              settings.aiAssistant.includeTerminalSelection,
          includeRecentTerminalOutput:
              settings.aiAssistant.includeRecentTerminalOutput,
        ),
        recording: settings.recording,
        sync: existing.sync,
      );
      await _writeConfig(config);
    });
  }

  Future<void> saveTerminalTheme({
    required String? themeId,
    required Map<String, Object?> customThemeJson,
  }) {
    return _enqueueSave(() async {
      final config = await loadConfig();
      final appearance = config.terminal.appearance.copyWith(
        themeId: themeId,
        customThemeJson: customThemeJson,
        clearThemeId: themeId == null,
      );
      await _writeConfig(
        config.copyWith(
          terminal: config.terminal.copyWith(appearance: appearance),
        ),
      );
    });
  }

  Future<String?> loadAiPresetsUrl() async {
    final config = await loadConfig();
    return config.ai.presetsUrl;
  }

  Future<NautermSyncConfig> loadSyncConfig() async {
    return (await loadConfig()).sync;
  }

  Future<void> saveSyncConfig(NautermSyncConfig sync) {
    return _enqueueSave(() async {
      final config = await loadConfig();
      await _writeConfig(config.copyWith(sync: sync));
    });
  }

  Future<NautermConfig> _readConfig() async {
    final file = paths.configFile;
    if (!await file.exists()) return const NautermConfig();
    final decoded = jsonDecode(await file.readAsString());
    return NautermConfig.fromJson(decoded);
  }

  Future<void> _writeConfig(NautermConfig config) async {
    const encoder = JsonEncoder.withIndent('  ');
    await paths.configFile.writeAsString(
      '${encoder.convert(config.toJson())}\n',
    );
  }

  Future<void> _enqueueSave(Future<void> Function() operation) {
    final save = _saveQueue.then((_) => operation());
    _saveQueue = save.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return save;
  }

  static String defaultConfigAssetPath(TargetPlatform platform) {
    return switch (platform) {
      TargetPlatform.macOS => nautermMacOSDefaultConfigAsset,
      TargetPlatform.windows => nautermWindowsDefaultConfigAsset,
      _ => nautermLinuxDefaultConfigAsset,
    };
  }
}
