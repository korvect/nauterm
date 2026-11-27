import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:nauterm/terminal/terminal_ffi.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  testWidgets('system information ignores paused samples and resumes polling', (
    tester,
  ) async {
    final requests = <Completer<FfiHostSystemInfoResult>>[];
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: buildTerminalSystemInfoPanelForTesting(() {
            final request = Completer<FfiHostSystemInfoResult>();
            requests.add(request);
            return request.future;
          }),
        ),
      ),
    );
    expect(requests, hasLength(1));

    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    requests.first.complete(
      const FfiHostSystemInfoResult(hostname: 'stale-host'),
    );
    await tester.pump(const Duration(seconds: 6));
    expect(find.text('Updates paused'), findsOneWidget);
    expect(find.textContaining('stale-host'), findsNothing);
    expect(requests, hasLength(1));

    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    expect(requests, hasLength(2));
    requests.last.complete(
      const FfiHostSystemInfoResult(hostname: 'current-host'),
    );
    await tester.pump();
    expect(find.text('Updates paused'), findsNothing);
    expect(find.textContaining('current-host'), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  test('host system information parses native JSON values', () {
    final info = FfiHostSystemInfoResult.fromJson({
      'hostname': 'server-01',
      'os_name': 'Debian GNU/Linux 13',
      'kernel': 'Linux 6.12',
      'architecture': 'x86_64',
      'uptime_seconds': 86461,
      'load_average': 0.42,
      'load_average_5': 0.38,
      'load_average_15': 0.31,
      'cpu_count': 8,
      'cpu_usage_percent': 37.5,
      'memory_total_bytes': 17179869184,
      'memory_used_bytes': 8589934592,
      'swap_total_bytes': 4294967296,
      'swap_used_bytes': 1073741824,
      'disk_total_bytes': 107374182400,
      'disk_used_bytes': 26843545600,
      'latency_ms': 8.4,
      'processes': [
        {
          'memory_bytes': 134217728,
          'cpu_usage_percent': 2.5,
          'command': 'sshd',
        },
      ],
      'network_interfaces': [
        {'name': 'eth0', 'received_bytes': 1024, 'transmitted_bytes': 512},
      ],
      'filesystems': [
        {'path': '/', 'total_bytes': 107374182400, 'used_bytes': 26843545600},
      ],
      'events': const [],
    });

    expect(info.hasData, isTrue);
    expect(info.hostname, 'server-01');
    expect(info.osName, 'Debian GNU/Linux 13');
    expect(info.cpuCount, 8);
    expect(info.cpuUsagePercent, 37.5);
    expect(info.memoryUsedBytes, 8589934592);
    expect(info.diskTotalBytes, 107374182400);
    expect(info.swapUsedBytes, 1073741824);
    expect(info.latencyMs, 8.4);
    expect(info.processes.single.command, 'sshd');
    expect(info.networkInterfaces.single.name, 'eth0');
    expect(info.filesystems.single.path, '/');
  });

  test('active SSH latency replaces the collector value', () {
    final info = FfiHostSystemInfoResult.fromJson({
      'hostname': 'server-01',
      'latency_ms': 52,
    });

    final current = info.withLatency(2.7);

    expect(current.hostname, 'server-01');
    expect(current.latencyMs, 2.7);
  });
}
