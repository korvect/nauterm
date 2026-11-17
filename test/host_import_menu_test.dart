import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';

void main() {
  testWidgets('new host dropdown exposes group and host import flow', (
    tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await tester.tap(find.byTooltip('New host actions'));
    await tester.pump();

    expect(find.text('New group'), findsOneWidget);
    expect(find.text('Import…'), findsOneWidget);

    await tester.tap(find.text('Import…'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 150));

    expect(find.text('Import to vault'), findsOneWidget);
    expect(find.text('CSV file'), findsOneWidget);
    expect(find.text('~/.ssh'), findsOneWidget);
    expect(find.text('PuTTY'), findsOneWidget);
    expect(find.text('MobaXterm'), findsOneWidget);
    expect(find.text('SecureCRT'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });
}
