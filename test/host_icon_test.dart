import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/data/host_import.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  setUp(() {
    final previous = hostIconMode;
    addTearDown(() => hostIconMode = previous);
  });

  test('custom icon updates preserve protocol options and OS metadata', () {
    const host = HostEntry(
      name: 'Router',
      type: NautermHostType.remote,
      os: 'linux',
      distro: 'ubuntu',
      config: {
        'icon': 'cisco',
        'ssh': {'compatibility_mode': true},
        'future': [1, 2],
      },
    );
    expect(host.icon, 'cisco');
    expect(host.withPlatform(os: 'linux', distro: 'debian').icon, 'cisco');
    expect(HostEntry.fromJson(host.toJson()).icon, 'cisco');
    final updated = HostEntry.withIcon(host.config, 'huawei');
    expect(host.config['icon'], 'cisco');
    expect(updated['icon'], 'huawei');
    expect(HostEntry.withIcon(updated, null), {
      'ssh': {'compatibility_mode': true},
      'future': [1, 2],
    });
    for (final icon in [null, '', '  ', 1]) {
      expect(
        HostEntry(
          name: 'Host',
          type: NautermHostType.remote,
          config: {'icon': icon},
        ).icon,
        isNull,
      );
    }
  });

  test(
    'custom icons survive database reopen, OS detection and CSV round trip',
    () {
      final directory = Directory.systemTemp.createTempSync(
        'nauterm-host-icons-',
      );
      addTearDown(() => directory.deleteSync(recursive: true));
      final path = '${directory.path}/hosts.sqlite';
      var store = NautermDataStore.openPath(path);
      addTearDown(() => store.dispose());
      final id = store.saveHost(
        const HostEntry(
          name: 'Router',
          type: NautermHostType.remote,
          host: 'router.example',
          sshEnabled: true,
          os: 'linux',
          distro: 'ubuntu',
          config: {
            'icon': 'cisco',
            'ssh': {'compatibility_mode': true},
          },
        ),
      );
      store.dispose();
      store = NautermDataStore.openPath(path);
      final detected = store
          .getHost(id)!
          .withPlatform(os: 'linux', distro: 'debian');
      store.saveHost(detected);
      expect(store.getHost(id)!.icon, 'cisco');
      expect(store.listHosts().single.icon, 'cisco');
      final exported = buildHostCsv(store.listHosts(), const [], const []);
      final imported = parseHostCsv(exported).single;
      expect(imported.icon, 'cisco');
      expect(imported.host, 'router.example');
      final cleared = HostEntry.fromJson({
        ...detected.toJson(),
        'config': HostEntry.withIcon(detected.config, null),
      });
      store.saveHost(cleared);
      final saved = store.getHost(id)!;
      expect(saved.icon, isNull);
      expect(saved.distro, 'debian');
      expect(saved.config['ssh'], {'compatibility_mode': true});
      expect(
        parseHostCsv('Label,Hostname/IP\nLegacy,legacy.example').single.icon,
        isNull,
      );
    },
  );

  for (final mode in HostIconMode.values) {
    testWidgets(
      'custom display follows $mode while editor always previews the main icon',
      (tester) async {
        hostIconMode = mode;
        const host = HostEntry(
          name: 'Appliance',
          type: NautermHostType.remote,
          os: 'linux',
          distro: 'debian',
          config: {'icon': 'cisco'},
        );
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: Row(
                children: [
                  KeyedSubtree(
                    key: const ValueKey('display-icon'),
                    child: buildHostIconPreviewForTesting(host),
                  ),
                  KeyedSubtree(
                    key: const ValueKey('editor-icon'),
                    child: buildHostIconPreviewForTesting(host, editing: true),
                  ),
                ],
              ),
            ),
          ),
        );
        final display = find.byKey(const ValueKey('display-icon'));
        final editor = find.byKey(const ValueKey('editor-icon'));
        final custom = find.byWidgetPredicate(
          (widget) =>
              widget is SvgPicture &&
              widget.bytesLoader is SvgAssetLoader &&
              (widget.bytesLoader as SvgAssetLoader).assetName.endsWith(
                'system-cisco.svg',
              ),
        );
        expect(find.descendant(of: editor, matching: custom), findsOneWidget);
        expect(
          find.descendant(of: editor, matching: find.text('A')),
          findsNothing,
        );
        expect(
          find.descendant(of: display, matching: custom),
          mode == HostIconMode.defaultIcon ? findsNothing : findsOneWidget,
        );
        expect(
          find.descendant(of: display, matching: find.text('A')),
          mode == HostIconMode.osIcon ? findsNothing : findsOneWidget,
        );
        expect(hostIconMode, mode);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  testWidgets('automatic and unknown icon names keep OS fallback', (
    tester,
  ) async {
    hostIconMode = HostIconMode.osIcon;
    for (final icon in [null, 'future-icon', '../untrusted']) {
      final host = HostEntry(
        name: 'Host',
        type: NautermHostType.remote,
        os: 'linux',
        distro: 'debian',
        config: {'icon': ?icon},
      );
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: buildHostIconPreviewForTesting(host, editing: true),
          ),
        ),
      );
      final svg = tester.widget<SvgPicture>(find.byType(SvgPicture));
      expect(
        (svg.bytesLoader as SvgAssetLoader).assetName,
        'assets/icons/os/system-debian.svg',
      );
    }
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('every bundled OS and vendor icon can be selected', (
    tester,
  ) async {
    final files = Directory('assets/icons/os')
        .listSync()
        .whereType<File>()
        .where((file) => file.path.endsWith('.svg'));
    for (final file in files) {
      final filename = file.uri.pathSegments.last;
      final name = filename.substring(
        'system-'.length,
        filename.length - '.svg'.length,
      );
      final host = HostEntry(
        name: 'Host',
        type: NautermHostType.remote,
        config: {'icon': name},
      );
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: buildHostIconPreviewForTesting(host, editing: true),
          ),
        ),
      );
      final svg = tester.widget<SvgPicture>(find.byType(SvgPicture));
      expect(
        (svg.bytesLoader as SvgAssetLoader).assetName,
        'assets/icons/os/$filename',
      );
    }
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
    'host editor icon picker exposes editing, search, selection and automatic reset',
    (tester) async {
      hostIconMode = HostIconMode.defaultIcon;
      tester.view.physicalSize = const Size(1400, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
      await tester.tap(find.text('New host'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 250));
      final picker = find.byKey(const ValueKey('host-icon-picker'));
      Color borderColor() {
        final border = tester.widget<DecoratedBox>(
          find.descendant(
            of: picker,
            matching: find.byWidgetPredicate(
              (widget) =>
                  widget is DecoratedBox &&
                  widget.position == DecorationPosition.foreground,
            ),
          ),
        );
        return ((border.decoration as BoxDecoration).border! as Border)
            .top
            .color;
      }

      expect(borderColor(), Colors.transparent);
      expect(
        find.byKey(const ValueKey('host-icon-edit-indicator')),
        findsOneWidget,
      );
      expect(find.byTooltip('Change icon'), findsOneWidget);
      await tester.tap(picker);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      expect(borderColor(), isNot(Colors.transparent));
      final automatic = find.byKey(const ValueKey('host-icon-automatic'));
      expect(
        find.ancestor(of: automatic, matching: find.byType(GridView)),
        findsOneWidget,
      );
      expect(find.text('Use automatic icon'), findsNothing);
      final search = find.descendant(
        of: find.byKey(const ValueKey('host-icon-search')),
        matching: find.byType(TextField),
      );
      await tester.enterText(search, 'hUaWeI');
      await tester.pump();
      expect(
        find.byKey(const ValueKey('host-icon-option:huawei')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey('host-icon-option:ubuntu')),
        findsNothing,
      );
      await tester.tap(find.byKey(const ValueKey('host-icon-option:huawei')));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.byKey(const ValueKey('host-icon-menu')), findsNothing);
      expect(borderColor(), Colors.transparent);
      final preview = tester.widget<SvgPicture>(
        find.descendant(of: picker, matching: find.byType(SvgPicture)),
      );
      expect(
        (preview.bytesLoader as SvgAssetLoader).assetName,
        'assets/icons/os/system-huawei.svg',
      );
      expect(hostIconMode, HostIconMode.defaultIcon);
      await tester.tap(picker);
      await tester.pump();
      await tester.enterText(search, '华为');
      await tester.pump();
      final selected = find.ancestor(
        of: find.byKey(const ValueKey('host-icon-option:huawei')),
        matching: find.byWidgetPredicate(
          (widget) => widget is Semantics && widget.properties.selected == true,
        ),
      );
      expect(selected, findsOneWidget);
      await tester.enterText(search, 'macOS');
      await tester.pump();
      expect(
        find.byKey(const ValueKey('host-icon-option:apple')),
        findsOneWidget,
      );
      await tester.enterText(search, 'no-such-icon');
      await tester.pump();
      expect(find.text('No matching icons'), findsOneWidget);
      await tester.enterText(search, 'auto');
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey('host-icon-automatic')));
      await tester.pump();
      expect(
        find.descendant(of: picker, matching: find.byType(SvgPicture)),
        findsNothing,
      );
      await tester.tap(picker);
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.byKey(const ValueKey('host-icon-menu')), findsNothing);
      expect(borderColor(), Colors.transparent);
      await tester.tap(picker);
      await tester.pump();
      await tester.tapAt(const Offset(700, 500));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.byKey(const ValueKey('host-icon-menu')), findsNothing);
      expect(borderColor(), Colors.transparent);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}
