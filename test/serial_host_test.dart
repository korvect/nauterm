import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/serial_connection_config.dart';
import 'package:nauterm/workspace/workspace_state_store.dart';

void main() {
  test(
    'serial configuration preserves future settings and applies defaults',
    () {
      final host = HostEntry.fromJson({
        'name': 'Console',
        'type': 'serial',
        'serial_port': 'COM3',
        'serial_baud_rate': 9600,
        'config': {
          'serial': {'future_option': true},
          'ssh': {'compatibility_mode': true},
        },
      });
      final serial = host.serialConnectionConfig!;
      expect(serial.dataBits, 8);
      expect(serial.stopBits, 1);
      expect(serial.parity, SerialParity.none);
      expect(serial.flowControl, SerialFlowControl.none);
      final updated = const SerialConnectionConfig(
        serialPort: 'COM3',
        baudRate: 115200,
        dataBits: 7,
        parity: SerialParity.odd,
        stopBits: 2,
        flowControl: SerialFlowControl.software,
      ).mergeHostConfig(host.config);
      expect(updated['ssh'], {'compatibility_mode': true});
      expect(updated['serial'], {
        'future_option': true,
        'data_bits': 7,
        'parity': 'odd',
        'stop_bits': 2,
        'flow_control': 'software',
      });
      expect(host.withPlatform(os: 'test').config, host.config);
      expect(HostEntry.fromJson(host.toJson()).serialPort, 'COM3');
    },
  );

  test('serial host persists through the FFI database and restart', () {
    final directory = Directory.systemTemp.createTempSync(
      'nauterm_serial_host_test_',
    );
    addTearDown(() => directory.deleteSync(recursive: true));
    final path = '${directory.path}/nauterm.sqlite';
    var store = NautermDataStore.openPath(path);
    final id = store.saveHost(
      const HostEntry(
        name: 'Console',
        type: NautermHostType.serial,
        serialPort: '/dev/cu.saved-console',
        serialBaudRate: 19200,
        config: {
          'serial': {
            'data_bits': 7,
            'parity': 'even',
            'stop_bits': 2,
            'flow_control': 'hardware',
          },
          'future': true,
        },
      ),
    );
    store.dispose();
    store = NautermDataStore.openPath(path);
    addTearDown(store.dispose);
    final saved = store.getHost(id)!;
    final config = saved.serialConnectionConfig!;
    expect(config.serialPort, '/dev/cu.saved-console');
    expect(config.baudRate, 19200);
    expect(config.framingLabel, '7E2');
    expect(config.flowControl, SerialFlowControl.hardware);
    expect(saved.config['future'], true);
    expect(store.listHosts().single.serialConnectionConfig!.baudRate, 19200);
    store.deleteHost(id);
    expect(store.listHosts(), isEmpty);
  });

  test('saved serial workspace targets round trip as host references', () {
    const target = WorkspaceTerminalTargetSnapshot(
      protocol: WorkspaceTerminalProtocol.serial,
      hostUuid: 'saved-serial-host',
    );
    final restored = WorkspaceTerminalTargetSnapshot.fromJson(target.toJson());
    expect(restored.protocol, WorkspaceTerminalProtocol.serial);
    expect(restored.hostUuid, 'saved-serial-host');
    expect(restored.shellPath, isNull);
  });

  testWidgets(
    'host types keep General fixed and serial options use full-size columns',
    (tester) async {
      tester.view.physicalSize = const Size(1440, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
      await tester.tap(find.text('New host'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 250));
      final type = find.byKey(const ValueKey('workspace-select-focus:Type'));
      final generalTop = tester.getTopLeft(find.text('General')).dy;
      final typeTop = tester.getTopLeft(type).dy;
      final scrollable = find
          .descendant(
            of: find.byKey(const ValueKey('workspace-editor-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first;
      final position = tester.state<ScrollableState>(scrollable).position;
      Future<void> selectType(String label) async {
        await tester.ensureVisible(type);
        await tester.tap(type);
        await tester.pump();
        final menu = find.byKey(const ValueKey('workspace-select-menu:Type'));
        await tester.tap(find.descendant(of: menu, matching: find.text(label)));
        await tester.pump();
        position.jumpTo(0);
        await tester.pump(const Duration(milliseconds: 1));
        expect(
          tester.getTopLeft(find.text('General')).dy,
          closeTo(generalTop, 0.01),
        );
        expect(tester.getTopLeft(type).dy, closeTo(typeTop, 0.01));
      }

      await selectType('Local');
      await selectType('Serial');
      final serialPort = find
          .ancestor(
            of: find.text('Serial Port'),
            matching: find.byType(InputDecorator),
          )
          .first;
      position.jumpTo(0);
      await tester.pump();
      await tester.ensureVisible(serialPort);
      await tester.pump();
      expect(serialPort, findsOneWidget);
      expect(find.byKey(const ValueKey('protocol-port:SSH')), findsNothing);
      expect(find.text('Data Bits'), findsOneWidget);
      expect(find.text('Flow Control'), findsOneWidget);
      Rect fieldRect(String label) => tester.getRect(
        find
            .ancestor(
              of: find.text(label),
              matching: find.byType(InputDecorator),
            )
            .first,
      );
      final baudRect = fieldRect('Baud Rate');
      final dataRect = fieldRect('Data Bits');
      final parityRect = fieldRect('Parity');
      final stopRect = fieldRect('Stop Bits');
      final flowRect = fieldRect('Flow Control');
      for (final rect in [dataRect, parityRect, stopRect, flowRect]) {
        expect(rect.width, (baudRect.width - 10) / 2);
        expect(rect.height, 40);
      }
      expect(dataRect.left, baudRect.left);
      expect(parityRect.top, dataRect.top);
      expect(parityRect.left, dataRect.right + 10);
      expect(stopRect.left, dataRect.left);
      expect(stopRect.top, greaterThan(dataRect.bottom));
      expect(flowRect.top, stopRect.top);
      expect(flowRect.left, parityRect.left);
      expect(tester.getRect(serialPort).left, baudRect.left);
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('workspace-editor-scroll-view')),
          matching: find.byIcon(Icons.usb_rounded),
        ),
        findsNothing,
      );

      final portField = find
          .descendant(of: serialPort, matching: find.byType(TextField))
          .first;
      await tester.enterText(portField, '/dev/cu.entered-console');
      position.jumpTo(position.maxScrollExtent);
      await tester.pump();
      position.jumpTo(0);
      await tester.pump();
      expect(
        tester.widget<TextField>(portField).controller!.text,
        '/dev/cu.entered-console',
      );
      await selectType('Remote');
      await selectType('Serial');
      expect(
        tester.widget<TextField>(portField).controller!.text,
        '/dev/cu.entered-console',
      );
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}
