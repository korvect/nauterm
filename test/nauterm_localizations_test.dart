import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_localizations.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('supports English and Simplified Chinese locales', () {
    expect(NautermLocalizations.supportedLocales, const [
      Locale('en'),
      Locale('zh', 'CN'),
    ]);
  });

  test('English delegate loads namespaced messages used by the UI', () async {
    final previousLanguage = appLanguage;
    addTearDown(() => setAppLanguage(previousLanguage));

    setAppLanguage(AppLanguage.english);
    final english = await NautermLocalizations.delegate.load(
      const Locale('en'),
    );

    expect(english.tr('settings.label.syncKey'), 'Sync Key');
    expect(
      english.tr('workspace.label.newHost', fallback: 'New Host'),
      'New Host',
    );
    expect(
      english.tr('settings.sync.key.localAvailable.label'),
      'Sync DEK available locally',
    );
    expect(
      tr('settings.label.masterKeyIsNotStored'),
      'Master Key is not stored',
    );
  });

  test(
    'translates known strings and falls back to English source text',
    () async {
      final chinese = await NautermLocalizations.load(const Locale('zh', 'CN'));
      final english = await NautermLocalizations.load(const Locale('en'));

      expect(chinese.tr('General Settings'), '常规设置');
      expect(
        chinese.tr(
          'settings.pages.general.title',
          fallback: 'General Settings',
        ),
        '常规设置',
      );
      expect(
        chinese.tr(
          'settings.general.applicationTheme.light',
          fallback: 'Light',
        ),
        '浅色',
      );
      expect(
        chinese.tr('settings.terminal.fontWeight.light', fallback: 'Light'),
        'Light',
      );
      expect(
        chinese.messages.containsKey('settings.terminal.fontWeight.light'),
        isFalse,
      );
      expect(
        chinese.tr(
          'settings.terminal.padding.description',
          fallback:
              'Inner spacing around terminal content. Supports 1 to 4 values.',
        ),
        '终端内容周围的内边距。支持 1 到 4 个值。',
      );
      expect(
        chinese.tr('missing.namespaced.key', fallback: 'Readable fallback'),
        'Readable fallback',
      );
      expect(
        chinese.tr('Connecting to user@example.com...'),
        '正在连接到 user@example.com…',
      );
      expect(
        chinese.tr('Delete "server" from known hosts?'),
        '从已知主机中删除“server”？',
      );
      expect(
        chinese.tr('Save connection history on this device.'),
        '在本机保存连接历史。',
      );
      expect(
        chinese.tr('Encrypt and save terminal output for later replay.'),
        '加密保存终端输出，以便稍后回放。',
      );
      expect(
        chinese.tr(
          'workspace.contextMenu.host.openInWorkspace',
          args: const {'workspace': 'Production'},
        ),
        '在 Production 工作区中打开',
      );
      expect(
        chinese.tr('workspace.contextMenu.host.connectionMethods'),
        '连接方式',
      );
      expect(chinese.tr('common.action.run'), '运行');
      expect(chinese.tr('workspace.action.duplicate'), '创建副本');
      expect(chinese.tr('workspace.label.duplicateGroup'), '创建分组副本');
      expect(chinese.tr('workspace.host.protocol.remove'), '移除协议');
      expect(chinese.tr('workspace.host.protocol.showMore'), '显示更多');
      expect(chinese.tr('workspace.label.clearSelection'), '清除选择');
      expect(chinese.tr('workspace.label.generateKey'), '生成密钥');
      expect(chinese.tr('workspace.label.newHost'), '新建主机');
      expect(chinese.tr('workspace.label.newKey'), '新建密钥');
      expect(chinese.tr('common.label.privateKey'), '私钥');
      expect(chinese.tr('common.label.remote'), '远程');
      expect(chinese.tr('workspace.import.title'), '导入到保险库');
      expect(chinese.tr('workspace.keys.import.fromFile'), '从密钥文件导入');
      expect(chinese.tr('workspace.keys.field.type'), '密钥类型');
      expect(
        chinese.tr('workspace.keys.validation.privateKeyRequired'),
        '请输入私钥。',
      );
      expect(chinese.tr('workspace.portForward.field.localPort'), '本地端口号');
      expect(chinese.tr('workspace.proxy.validation.hostRequired'), '请输入代理主机。');
      expect(chinese.tr('terminal.action.showComposer'), '显示命令编辑器');
      expect(chinese.tr('terminal.action.hideComposer'), '隐藏命令编辑器');
      expect(chinese.tr('settings.update.action.check'), '检查更新');
      expect(chinese.tr('settings.ai.provider.maxTokens.label'), '最大 Token 数');
      expect(chinese.tr('settings.sync.status.ready'), '同步已就绪');
      expect(chinese.tr('settings.sync.provider.status.connected'), '已连接');
      expect(chinese.tr('settings.sync.provider.status.ready'), '就绪');
      expect(chinese.tr('settings.sync.provider.status.notConfigured'), '未配置');
      expect(chinese.tr('common.label.showAll'), '显示全部');
      expect(chinese.tr('common.label.showLess'), '收起');
      expect(chinese.tr('common.label.home'), '主目录');
      expect(chinese.tr('workspace.credentials.keyOrCertificate'), '密钥或证书');
      expect(chinese.tr('sftp.action.newFolder'), '新建文件夹');
      expect(chinese.tr('sftp.action.download'), '下载');
      expect(chinese.tr('sftp.action.upload'), '上传…');
      expect(chinese.tr('sftp.action.moveTo'), '移动到…');
      expect(chinese.tr('sftp.action.copyTo'), '复制到…');
      expect(chinese.tr('sftp.action.withSudo'), '使用 sudo…');
      expect(chinese.tr('sftp.tooltip.favoriteCurrentPath'), '收藏当前路径');
      expect(chinese.tr('sftp.label.clearTasks'), '清除任务');
      expect(chinese.tr('sftp.sudo.explicit.dialog.title'), '使用 sudo 认证');
      expect(chinese.tr('sftp.sudo.permissionDenied.dialog.title'), '权限不足');
      expect(
        chinese.tr('sftp.sudo.permissionDenied.action.retry'),
        '使用 sudo 重试',
      );
      expect(
        chinese.tr(
          'settings.sync.status.connectedProviders',
          args: const {'count': 3},
        ),
        '已连接 3 个提供商',
      );
      expect(chinese.tr('settings.sync.mergeStrategy.smartMerge'), '智能合并');
      expect(chinese.tr('settings.sync.mergeStrategy.label'), '合并策略');
      expect(chinese.tr('settings.sync.automatic.unit'), '分钟');
      expect(
        chinese.tr('settings.sync.key.localAvailable.label'),
        '同步 DEK 已在本地可用',
      );
      expect(
        chinese.tr('settings.sync.key.localAvailable.description'),
        '此设备无需再次询问主密钥即可同步。',
      );
      expect(
        chinese.tr('settings.sync.key.masterKeyNotStored.description'),
        '主密钥仅用于封装或解封随机同步 DEK。',
      );
      expect(
        chinese.tr('settings.shortcuts.action.newLocalTerminal'),
        '新建本地终端',
      );
      expect(chinese.tr('workspace.terminalTools.mode.systemInfo'), '系统信息');
      expect(chinese.tr('workspace.sort.newestFirst'), '从新到旧');
      expect(
        chinese.tr('workspace.tooltip.sort', args: const {'order': '从新到旧'}),
        '排序：从新到旧',
      );
      expect(chinese.tr('No hosts yet'), '暂无主机');
      expect(
        chinese.tr('Search host:, group:, username:, tag:, or user@host'),
        '搜索 host:、group:、username:、tag: 或 user@host',
      );
      expect(chinese.tr('Saving...'), '正在保存…');
      expect(
        chinese.tr('Untranslated future text'),
        'Untranslated future text',
      );
      expect(english.tr('General Settings'), 'General Settings');
    },
  );

  test('language config values round trip', () {
    for (final language in AppLanguage.values) {
      expect(AppLanguage.fromString(language.configValue), language);
    }
  });

  test(
    'English and Chinese resource files contain the same message keys',
    () async {
      final english = jsonDecode(
        await rootBundle.loadString('assets/i18n/en.json'),
      ) as Map<String, dynamic>;
      final chinese = jsonDecode(
        await rootBundle.loadString('assets/i18n/zh_CN.json'),
      ) as Map<String, dynamic>;
      final englishKeys = english.keys
          .where((key) => key != '_patterns')
          .toList(growable: false);
      final chineseKeys = chinese.keys
          .where((key) => key != '_patterns')
          .toList(growable: false);

      expect(chineseKeys, englishKeys);
      expect(english.length, greaterThan(650));
      final namespacedKey = RegExp(r'^[a-z][A-Za-z0-9]*(\.[A-Za-z0-9]+)+$');
      expect(
        english.keys.where((key) => !namespacedKey.hasMatch(key)),
        isEmpty,
      );
      expect(
        english.keys
            .expand((key) => key.split('.'))
            .where((segment) => segment.length > 36),
        isEmpty,
      );
      expect(
        english.keys.where((key) => key.startsWith('settings.description.')),
        isEmpty,
      );
      expect(
        english.keys.where((key) => RegExp(r'[a-f0-9]{8}$').hasMatch(key)),
        isEmpty,
      );
      final settingLabelsByValue = <Object?, List<String>>{};
      for (final entry in english.entries) {
        if (entry.key.startsWith('settings.') &&
            !entry.key.startsWith('settings.label.')) {
          settingLabelsByValue
              .putIfAbsent(entry.value, () => <String>[])
              .add(entry.key);
        }
      }
      expect(
        english.entries.where(
          (entry) =>
              entry.key.startsWith('settings.label.') &&
              settingLabelsByValue.containsKey(entry.value),
        ),
        isEmpty,
      );
      expect(
        english.entries.where(
          (entry) => const {
            'VS Code',
            'https://s3.example.com',
            'github_pat_...',
            'Extra Bold',
            'sel',
            'Normal',
            'Bright',
          }.contains(entry.value),
        ),
        isEmpty,
      );
      final sortedKeys = [...englishKeys]..sort(_compareLocalizationKeys);
      expect(englishKeys, sortedKeys);
      if (chinese.containsKey('_patterns')) {
        expect(chinese.keys.last, '_patterns');
      }
    },
  );

  test('language names use autonyms in every interface language', () async {
    final english = await NautermLocalizations.load(const Locale('en'));
    final chinese = await NautermLocalizations.load(const Locale('zh', 'CN'));

    expect(AppLanguage.english.displayName(english), 'English');
    expect(AppLanguage.english.displayName(chinese), 'English');
    expect(AppLanguage.simplifiedChinese.displayName(english), '简体中文');
    expect(AppLanguage.simplifiedChinese.displayName(chinese), '简体中文');
    expect(AppLanguage.system.displayName(english), 'System Default');
    expect(AppLanguage.system.displayName(chinese), '跟随系统');
  });
}

const _localizationRoleOrder = <String>[
  'label',
  'title',
  'description',
  'help',
  'summary',
  'hint',
  'prompt',
  'empty',
  'success',
  'warning',
];

const _localizationCategorySegments = <String>{
  'action',
  'description',
  'dialog',
  'example',
  'label',
};

int _compareLocalizationKeys(String left, String right) {
  final leftSegments = left.split('.');
  final rightSegments = right.split('.');
  final leftLeaf = leftSegments.removeLast();
  final rightLeaf = rightSegments.removeLast();
  final parentComparison = leftSegments
      .join('.')
      .compareTo(rightSegments.join('.'));
  if (parentComparison != 0) return parentComparison;
  if (leftSegments.isNotEmpty &&
      _localizationCategorySegments.contains(leftSegments.last)) {
    return leftLeaf.compareTo(rightLeaf);
  }
  final leftRole = _localizationRoleOrder.indexOf(leftLeaf);
  final rightRole = _localizationRoleOrder.indexOf(rightLeaf);
  final leftRank = leftRole < 0 ? _localizationRoleOrder.length : leftRole;
  final rightRank = rightRole < 0 ? _localizationRoleOrder.length : rightRole;
  final rankComparison = leftRank.compareTo(rightRank);
  return rankComparison != 0 ? rankComparison : leftLeaf.compareTo(rightLeaf);
}
