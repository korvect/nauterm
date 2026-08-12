import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ai/ai_config.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/data/nauterm_config_store.dart';
import 'package:nauterm/data/nauterm_config.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/terminal/terminal_recording_config.dart';
import 'package:nauterm/terminal/terminal_theme.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('automatic sync defaults to three days', () {
    expect(const NautermSyncConfig().interval, 3 * 24 * 60 * 60 * 1000);
  });

  test('terminal emulation engine round trips through schema version 1', () {
    final config = NautermConfig.fromJson(const {
      'schemaVersion': 1,
      'terminal': {'emulationEngine': 'ghostty'},
    });
    expect(config.terminal.emulationEngine, TerminalEmulatorBackend.ghostty);
    expect(
      (config.toJson()['terminal'] as Map<String, Object?>)['emulationEngine'],
      'ghostty',
    );
    expect(
      NautermConfig.fromJson(const {
        'schemaVersion': 1,
      }).terminal.emulationEngine,
      TerminalEmulatorBackend.alacritty,
    );
    expect(
      NautermConfig.fromJson(const {
        'schemaVersion': 1,
        'terminal': {'emulator': 'ghostty'},
      }).terminal.emulationEngine,
      TerminalEmulatorBackend.alacritty,
    );
  });

  test('selecting a command block on click defaults to enabled', () {
    expect(
      NautermConfig.fromJson(const {
        'schemaVersion': 1,
      }).terminal.behavior.selectCommandBlockOnClick,
      isTrue,
    );
  });

  test('selects the platform default config asset', () {
    expect(
      NautermConfigStore.defaultConfigAssetPath(TargetPlatform.macOS),
      nautermMacOSDefaultConfigAsset,
    );
    expect(
      NautermConfigStore.defaultConfigAssetPath(TargetPlatform.linux),
      nautermLinuxDefaultConfigAsset,
    );
    expect(
      NautermConfigStore.defaultConfigAssetPath(TargetPlatform.windows),
      nautermWindowsDefaultConfigAsset,
    );
  });

  test('Linux default uses a concrete terminal font family', () async {
    final json = jsonDecode(
      await rootBundle.loadString(nautermLinuxDefaultConfigAsset),
    );
    final config = NautermConfig.fromJson(json);

    expect(config.terminal.appearance.font.family, 'DejaVu Sans Mono');
  });

  test('formats shortcut labels for the current desktop platform', () {
    expect(
      formatShortcutForPlatform(
        'cmd+alt+shift+right',
        platform: TargetPlatform.macOS,
      ),
      '⌘⌥⇧→',
    );
    expect(
      formatShortcutForPlatform(
        'cmd+alt+shift+right',
        platform: TargetPlatform.windows,
      ),
      'Ctrl+Alt+Shift+→',
    );
    expect(
      formatShortcutForPlatform(
        'cmd+alt+shift+right',
        platform: TargetPlatform.linux,
        compact: false,
      ),
      'Ctrl Alt Shift →',
    );
    expect(
      formatShortcutForPlatform('', platform: TargetPlatform.macOS),
      isNull,
    );
    expect(
      formatTerminalEditingShortcutForPlatform(
        'cmd+c',
        platform: TargetPlatform.windows,
      ),
      'Ctrl+Shift+C',
    );
  });

  test('writes the platform default config only when missing', () async {
    final directory = await Directory.systemTemp.createTemp(
      'nauterm_config_store_test_',
    );
    addTearDown(() async {
      if (await directory.exists()) {
        await directory.delete(recursive: true);
      }
    });

    final paths = NautermPaths(
      configDirectory: Directory('${directory.path}/config'),
      dataDirectory: Directory('${directory.path}/data'),
    );
    final store = NautermConfigStore(
      paths,
      bundle: _MapAssetBundle({
        nautermLinuxDefaultConfigAsset: '{"platform":"linux"}',
      }),
    );

    await store.ensureDefaultConfig(platform: TargetPlatform.linux);
    expect(await paths.configFile.readAsString(), '{"platform":"linux"}');

    await paths.configFile.writeAsString('{"custom":true}');
    await store.ensureDefaultConfig(platform: TargetPlatform.linux);
    expect(await paths.configFile.readAsString(), '{"custom":true}');
  });

  test('reads the AI preset URL from the ai section', () async {
    final directory = await Directory.systemTemp.createTemp(
      'nauterm_config_store_test_',
    );
    addTearDown(() async {
      if (await directory.exists()) {
        await directory.delete(recursive: true);
      }
    });

    final paths = NautermPaths(
      configDirectory: directory,
      dataDirectory: directory,
    );
    await paths.ensureCreated();
    await paths.configFile.writeAsString(
      '{"schemaVersion":1,"ai":{"presetsUrl":"https://example.com/presets.json"}}',
    );

    final url = await NautermConfigStore(paths).loadAiPresetsUrl();

    expect(url, 'https://example.com/presets.json');
  });

  test(
    'sync behavior is stored in config and survives runtime saves',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'nauterm_sync_config_test_',
      );
      addTearDown(() => directory.delete(recursive: true));
      final paths = NautermPaths(
        configDirectory: directory,
        dataDirectory: directory,
      );
      await paths.ensureCreated();
      await paths.configFile.writeAsString('{"schemaVersion":1}\n');
      final store = NautermConfigStore(paths);

      await store.saveSyncConfig(
        const NautermSyncConfig(
          mergeStrategy: 'remote_wins',
          automatic: true,
          interval: 10800000,
          backupCount: 7,
        ),
      );
      await store.saveRuntimeSettings(
        const NautermRuntimeSettings(
          copyOnSelect: false,
          font: TerminalFontConfig(),
          keyboard: TerminalKeyboardConfig(),
        ),
      );

      final sync = await store.loadSyncConfig();
      expect(sync.mergeStrategy, 'remote_wins');
      expect(sync.automatic, isTrue);
      expect(sync.interval, 10800000);
      expect(sync.backupCount, 7);
      final json = jsonDecode(await paths.configFile.readAsString()) as Map;
      expect(json['sync'], {
        'mergeStrategy': 'remote_wins',
        'automatic': true,
        'interval': 10800000,
        'backupCount': 7,
      });
    },
  );

  test('application language is loaded and survives runtime saves', () async {
    final directory = await Directory.systemTemp.createTemp(
      'nauterm_language_config_test_',
    );
    addTearDown(() => directory.delete(recursive: true));
    final paths = NautermPaths(
      configDirectory: directory,
      dataDirectory: directory,
    );
    await paths.ensureCreated();
    await paths.configFile.writeAsString(
      '{"schemaVersion":1,"appearance":{"language":"zh-CN"}}\n',
    );
    final store = NautermConfigStore(paths);

    final loaded = await store.loadRuntimeSettings();
    expect(loaded.appLanguage, AppLanguage.simplifiedChinese);

    await store.saveRuntimeSettings(
      NautermRuntimeSettings(
        copyOnSelect: loaded.copyOnSelect,
        font: loaded.font,
        keyboard: loaded.keyboard,
        appLanguage: loaded.appLanguage,
      ),
    );

    final json = jsonDecode(await paths.configFile.readAsString()) as Map;
    expect((json['appearance'] as Map)['language'], 'zh-CN');
  });

  test('unknown or missing application language follows the system', () {
    expect(
      NautermConfig.fromJson(const {
        'schemaVersion': 1,
        'appearance': {'language': 'unsupported'},
      }).appearance.language,
      AppLanguage.system,
    );
    expect(
      NautermConfig.fromJson(const {'schemaVersion': 1}).appearance.language,
      AppLanguage.system,
    );
  });

  test('rejects unsupported config schema versions', () async {
    final directory = await Directory.systemTemp.createTemp(
      'nauterm_config_schema_test_',
    );
    addTearDown(() => directory.delete(recursive: true));
    final paths = NautermPaths(
      configDirectory: directory,
      dataDirectory: directory,
    );
    await paths.ensureCreated();
    await paths.configFile.writeAsString('{"schemaVersion":40}\n');
    final store = NautermConfigStore(paths);

    await expectLater(store.loadRuntimeSettings(), throwsFormatException);
  });

  test('runtime settings persist terminal keyboard options', () async {
    final directory = await Directory.systemTemp.createTemp(
      'nauterm_config_store_test_',
    );
    addTearDown(() async {
      if (await directory.exists()) {
        await directory.delete(recursive: true);
      }
    });

    final paths = NautermPaths(
      configDirectory: directory,
      dataDirectory: directory,
    );
    await paths.ensureCreated();
    await paths.configFile.writeAsString('{"schemaVersion":1}\n');
    final store = NautermConfigStore(paths);

    await store.saveRuntimeSettings(
      const NautermRuntimeSettings(
        copyOnSelect: true,
        selectCommandBlockOnClick: false,
        font: TerminalFontConfig(
          family: 'JetBrains Mono',
          cjkFamily: 'Sarasa Mono SC',
          size: 15,
        ),
        keyboard: TerminalKeyboardConfig(
          useOptionAsMetaKey: false,
          reportMouseEvents: false,
          navigationKeysScrollOutsideInteractiveApps: false,
        ),
        shortcuts: TerminalShortcutConfig(
          quickConnect: '',
          tabSwitches: [
            '',
            'cmd+2',
            'cmd+3',
            'cmd+4',
            'cmd+5',
            'cmd+6',
            'cmd+7',
            'cmd+8',
            'cmd+9',
          ],
        ),
        sftp: SftpConfig(
          sshEditor: 'nano',
          showTab: false,
          textFileExtensions: ['md', 'toml'],
          concurrentTasks: 6,
          transferThreads: 16,
        ),
        aiAssistant: AiAssistantConfig(
          protocol: AiApiProtocol.anthropic,
          baseUrl: 'https://ai.example/v1',
          model: 'claude-test',
          apiKey: 'test-key',
          includeTerminalSelection: false,
        ),
        window: WindowConfig(width: 1180, height: 760, x: -320, y: 48),
        autocompleteEnabled: true,
        scrollbarEnabled: false,
        bell: TerminalBellConfig(
          sound: false,
          visual: true,
          tabIndicator: false,
        ),
        pointer: TerminalPointerConfig(
          commandClickOpensFilenameOrUrl: false,
          optionClickMovesCursor: false,
        ),
        sshKeepaliveIntervalSeconds: 45,
        sshPredictionMode: TerminalSshPredictionMode.always,
        hostIconMode: HostIconMode.osIcon,
        recording: NautermRecordingConfig(
          enabled: false,
          captureEnabled: true,
          retentionDays: 14,
          maxSessionBytes: 1234,
          maxTotalBytes: 5678,
        ),
      ),
    );

    final loaded = await store.loadRuntimeSettings();
    expect(loaded.copyOnSelect, isTrue);
    expect(loaded.selectCommandBlockOnClick, isFalse);
    expect(loaded.font.family, 'JetBrains Mono');
    expect(loaded.font.cjkFamily, 'Sarasa Mono SC');
    expect(loaded.font.size, 15);
    expect(loaded.keyboard.useOptionAsMetaKey, isFalse);
    expect(loaded.keyboard.reportMouseEvents, isFalse);
    expect(loaded.keyboard.navigationKeysScrollOutsideInteractiveApps, isFalse);
    expect(loaded.shortcuts.quickConnect, isEmpty);
    expect(loaded.shortcuts.tabSwitches.first, isEmpty);
    expect(loaded.sftp.sshEditor, 'nano');
    expect(loaded.sftp.showTab, isFalse);
    expect(loaded.sftp.textFileExtensions, ['md', 'toml']);
    expect(loaded.sftp.concurrentTasks, 6);
    expect(loaded.sftp.transferThreads, 16);
    expect(loaded.autocompleteEnabled, isTrue);
    expect(loaded.scrollbarEnabled, isFalse);
    expect(loaded.bell.sound, isFalse);
    expect(loaded.bell.visual, isTrue);
    expect(loaded.bell.tabIndicator, isFalse);
    expect(loaded.pointer.commandClickOpensFilenameOrUrl, isFalse);
    expect(loaded.pointer.optionClickMovesCursor, isFalse);
    expect(loaded.sshKeepaliveIntervalSeconds, 45);
    expect(loaded.sshPredictionMode, TerminalSshPredictionMode.always);
    expect(loaded.hostIconMode, HostIconMode.osIcon);
    expect(loaded.recording.enabled, isFalse);
    expect(loaded.recording.captureEnabled, isTrue);
    expect(loaded.recording.retentionDays, 14);
    expect(loaded.recording.maxSessionBytes, 1234);
    expect(loaded.recording.maxTotalBytes, 5678);
    // AI provider settings are now stored in the database, not JSON config
    // Only preference booleans are persisted in JSON
    expect(loaded.window.width, 1180);
    expect(loaded.window.height, 760);
    expect(loaded.window.x, -320);
    expect(loaded.window.y, 48);

    final json =
        jsonDecode(await paths.configFile.readAsString())
            as Map<String, dynamic>;
    expect(json['schemaVersion'], 1);
    expect(json, isNot(contains('aiAssistant')));
    expect((json['appearance'] as Map)['hostIcon'], 'osIcon');
    final terminal = json['terminal'] as Map<String, dynamic>;
    expect(terminal, isNot(contains('font')));
    expect(terminal['emulation'], 'xterm-256color');
    final persistedFont =
        (terminal['appearance'] as Map)['font'] as Map<String, dynamic>;
    expect(persistedFont['size'], 15.0);
    expect(persistedFont, isNot(contains('fallback')));
    expect(persistedFont['cjkFamily'], 'Sarasa Mono SC');
    expect((terminal['appearance'] as Map)['scrollbar'], isFalse);
    expect((terminal['behavior'] as Map)['copyOnSelect'], isTrue);
    expect((terminal['behavior'] as Map)['selectCommandBlockOnClick'], isFalse);
    expect((terminal['behavior'] as Map)['bell'], {
      'sound': false,
      'visual': true,
      'tabIndicator': false,
    });
    expect((terminal['behavior'] as Map)['pointer'], {
      'commandClickOpensFilenameOrUrl': false,
      'optionClickMovesCursor': false,
    });
    expect((terminal['features'] as Map)['autocomplete'], isTrue);
    expect((terminal['input'] as Map)['useOptionAsMetaKey'], isFalse);
    expect((terminal['input'] as Map)['reportMouseEvents'], isFalse);
    expect(
      (terminal['input'] as Map)['navigationKeysScrollOutsideInteractiveApps'],
      isFalse,
    );
    expect((json['tabs'] as Map)['confirmOnClose'], isTrue);
    expect((json['ssh'] as Map)['keepaliveInterval'], 45000);
    expect(json['shortcuts'], isA<Map>());
    expect((json['recording'] as Map)['retentionDays'], 14);
  });

  test(
    'missing shortcuts use defaults while empty shortcuts stay disabled',
    () {
      const defaults = TerminalShortcutConfig();
      final loaded = TerminalShortcutConfig.fromJson({
        'quickConnect': '',
        'copy': ' Command + Shift + C ',
        'tabSwitches': ['', 'cmd+2'],
      });

      expect(loaded.quickConnect, isEmpty);
      expect(loaded.copy, 'cmd+shift+c');
      expect(loaded.paste, defaults.paste);
      expect(loaded.openSettings, defaults.openSettings);
      expect(loaded.editWorkspaceItem, defaults.editWorkspaceItem);
      expect(loaded.duplicateWorkspaceItem, defaults.duplicateWorkspaceItem);
      expect(loaded.tabSwitches.first, isEmpty);
      expect(loaded.tabSwitches[1], defaults.tabSwitches[1]);
      expect(loaded.tabSwitches[2], defaults.tabSwitches[2]);
    },
  );

  test('default SFTP directory is the user home', () {
    final home =
        Platform.environment['HOME'] ??
        Platform.environment['USERPROFILE'] ??
        Directory.current.path;
    final normalizedHome = home.replaceAll('/', Platform.pathSeparator);
    expect(defaultSftpLocalDirectory(), normalizedHome);
    expect(
      fallbackDownloadsDirectory(),
      '$normalizedHome${Platform.pathSeparator}Downloads',
    );
  });

  test('saving a terminal theme preserves unrelated settings', () async {
    final directory = await Directory.systemTemp.createTemp(
      'nauterm_config_store_theme_test_',
    );
    addTearDown(() async {
      if (await directory.exists()) {
        await directory.delete(recursive: true);
      }
    });

    final paths = NautermPaths(
      configDirectory: directory,
      dataDirectory: directory,
    );
    await paths.ensureCreated();
    await paths.configFile.writeAsString(
      '{"schemaVersion":1,'
      '"terminal":{"appearance":{"theme":{"id":"default"}},'
      '"behavior":{"copyOnSelect":true}},'
      '"sftp":{"showTab":false},"workspace":{"enabled":false}}\n',
    );

    await NautermConfigStore(paths).saveTerminalTheme(
      themeId: 'one-dark',
      customThemeJson: defaultTerminalTheme.toJson(),
    );

    final config =
        jsonDecode(await paths.configFile.readAsString())
            as Map<String, dynamic>;
    final terminal = config['terminal'] as Map;
    expect((terminal['behavior'] as Map)['copyOnSelect'], isTrue);
    expect((terminal['appearance'] as Map)['theme']['id'], 'one-dark');
    expect((config['sftp'] as Map)['showTab'], isFalse);
    expect((config['workspace'] as Map)['enabled'], isFalse);
  });
}

class _MapAssetBundle extends AssetBundle {
  _MapAssetBundle(this.assets);

  final Map<String, String> assets;

  @override
  Future<ByteData> load(String key) async {
    final value = assets[key];
    if (value == null) {
      throw FlutterError('Missing test asset: $key');
    }
    final bytes = Uint8List.fromList(utf8.encode(value));
    return ByteData.sublistView(bytes);
  }
}
