import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';

void main() {
  test('memory terminal scrolls its fixed-size cell buffer', () {
    final driver = MemoryTerminalDriver(columns: 2, rows: 2);

    expect(() => driver.write('abcd'), returnsNormally);
    expect(driver.snapshot.cells.map((cell) => cell.text), [
      'c',
      'd',
      ' ',
      ' ',
    ]);
  });

  test(
    'controller refreshes the first idle output without a fixed delay',
    () async {
      final driver = _PollingMemoryTerminalDriver(columns: 2, rows: 2);
      final controller = TerminalController(driver: driver);
      addTearDown(controller.dispose);
      var notifications = 0;
      controller.addListener(() => notifications += 1);

      await Future<void>.delayed(const Duration(milliseconds: 20));
      driver
        ..write('x')
        ..changed = true;

      expect(controller.poll(), isTrue);
      expect(controller.snapshot.cells.first.text, 'x');
      expect(notifications, 1);
    },
  );
}

class _PollingMemoryTerminalDriver extends MemoryTerminalDriver {
  _PollingMemoryTerminalDriver({required super.columns, required super.rows});

  bool changed = false;

  @override
  bool poll() {
    final result = changed;
    changed = false;
    return result;
  }
}
