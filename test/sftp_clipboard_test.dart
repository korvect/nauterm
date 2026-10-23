import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/ui/nauterm_context_menu.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  late List<String> writes;

  setUp(() async {
    writes = [];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, (call) async {
          if (call.method == 'Clipboard.setData') {
            writes.add((call.arguments as Map)['text'] as String);
          }
          return null;
        });
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
  });

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, null);
  });

  test('copies names without removing extensions or whitespace', () async {
    await copySftpEntryText([
      (name: 'report 2026.tar.gz', path: '/home/tester/report 2026.tar.gz'),
      (name: ' 配置文件 ', path: '/home/tester/ 配置文件 '),
      (name: 'folder name', path: '/home/tester/folder name'),
    ], absolutePath: false);
    expect(writes, ['report 2026.tar.gz\n 配置文件 \nfolder name']);
  });

  test(
    'copies complete remote paths without shell quoting or URI encoding',
    () async {
      await copySftpEntryText([
        (name: "a'b.txt", path: "/home/tester/a'b.txt"),
        (name: '部署 #1.conf', path: '/etc/服务/部署 #1.conf'),
      ], absolutePath: true);
      expect(writes, ["/home/tester/a'b.txt\n/etc/服务/部署 #1.conf"]);
    },
  );

  test('keeps Windows drive and UNC paths unchanged', () async {
    await copySftpEntryText([
      (name: 'report.txt', path: r'C:\Users\tester\report.txt'),
      (name: 'folder name', path: r'\\server\share\folder name'),
    ], absolutePath: true);
    expect(writes, [
      'C:\\Users\\tester\\report.txt\n\\\\server\\share\\folder name',
    ]);
  });

  test('keeps duplicate names when they refer to different files', () async {
    await copySftpEntryText([
      (name: 'config.json', path: '/a/config.json'),
      (name: 'config.json', path: '/b/config.json'),
    ], absolutePath: false);
    expect(writes, ['config.json\nconfig.json']);
  });

  for (final absolutePath in [false, true]) {
    test(
      'an empty selection does not overwrite the clipboard, absolute=$absolutePath',
      () async {
        await copySftpEntryText(const [], absolutePath: absolutePath);
        expect(writes, isEmpty);
      },
    );
  }

  test('copy menu labels are distinct from file-transfer actions in both languages', () async {
    final english = await NautermLocalizations.load(const Locale('en'));
    final chinese = await NautermLocalizations.load(const Locale('zh', 'CN'));
    expect(english.tr('sftp.action.copyName'), 'Copy Name');
    expect(english.tr('sftp.action.copyAbsolutePath'), 'Copy Absolute Path');
    expect(chinese.tr('sftp.action.copyName'), '复制名称');
    expect(chinese.tr('sftp.action.copyAbsolutePath'), '复制绝对路径');
    expect(chinese.tr('sftp.action.copyTo'), '复制到…');
  });

  for (final absolutePath in [false, true]) {
    testWidgets(
      'local row menu copies the selected item, absolute=$absolutePath',
      (tester) async {
        final directory = await _openLocalFixture(tester);
        final row = _leftText('a folder');
        await tester.tap(row, buttons: kSecondaryMouseButton);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 150));
        expect(find.text('Copy Name'), findsOneWidget);
        expect(find.text('Copy Absolute Path'), findsOneWidget);
        await tester.tap(
          find.text(absolutePath ? 'Copy Absolute Path' : 'Copy Name'),
        );
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 150));
        expect(writes, [
          absolutePath
              ? '${directory.path}${Platform.pathSeparator}a folder'
              : 'a folder',
        ]);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  testWidgets(
    'right clicking a selected row copies the full selection in list order',
    (tester) async {
      await _openLocalFixture(tester);
      // Reverse the visible order so clipboard order cannot accidentally come
      // from the original directory listing or the order of selection clicks.
      await tester.tap(_leftText('Name'));
      await tester.pump();
      await tester.tap(_leftText('a folder'));
      await tester.pump();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.tap(_leftText('z folder'));
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      await tester.tap(_leftText('z folder'), buttons: kSecondaryMouseButton);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 150));
      await tester.tap(find.text('Copy Name'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 150));
      expect(writes, ['z folder\na folder']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets('the parent-directory row disables both clipboard actions', (
    tester,
  ) async {
    await _openLocalFixture(tester);
    await tester.tap(_leftText('..'), buttons: kSecondaryMouseButton);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 150));
    for (final label in ['Copy Name', 'Copy Absolute Path']) {
      final row = tester.widget<NautermDropdownRow>(
        find.ancestor(
          of: find.text(label),
          matching: find.byType(NautermDropdownRow),
        ),
      );
      expect(row.enabled, isFalse);
    }
    expect(writes, isEmpty);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

Finder _leftText(String text) => find.descendant(
  of: find.byKey(const ValueKey('sftp-transfer-target:left')),
  matching: find.text(text),
);

Future<Directory> _openLocalFixture(WidgetTester tester) async {
  final directory = Directory.systemTemp.createTempSync('nauterm-sftp-copy-');
  Directory('${directory.path}${Platform.pathSeparator}a folder').createSync();
  Directory('${directory.path}${Platform.pathSeparator}z folder').createSync();
  addTearDown(() => directory.deleteSync(recursive: true));
  final previousDirectory = sftpDefaultDownloadDir;
  final previousTab = sftpTabEnabled;
  sftpDefaultDownloadDir = directory.path;
  setSftpTabEnabled(true);
  addTearDown(() {
    sftpDefaultDownloadDir = previousDirectory;
    setSftpTabEnabled(previousTab);
  });
  const channel = MethodChannel('com.korvect.nauterm/file_drop');
  TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, (_) async => null);
  addTearDown(
    () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null),
  );
  tester.view.physicalSize = const Size(1400, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 300));
  await tester.tap(find.byKey(const ValueKey('workspace-top-tab:sftp')));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 300));
  await tester.tap(find.byKey(const ValueKey('sftp-empty-use-local-button')));
  await tester.pump();
  for (
    var attempt = 0;
    attempt < 30 && _leftText('a folder').evaluate().isEmpty;
    attempt++
  ) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 10)),
    );
    await tester.pump();
  }
  await tester.pump(const Duration(milliseconds: 300));
  expect(sftpEffectiveLocalDirectory(), directory.path);
  expect(_leftText('a folder'), findsOneWidget);
  return directory;
}
