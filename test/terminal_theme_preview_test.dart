import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/terminal/terminal_theme.dart';
import 'package:nauterm/ui/terminal_theme_preview.dart';

void main() {
  testWidgets('sample uses the terminal background without color mixing', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Center(
          child: SizedBox(
            width: 240,
            height: 132,
            child: TerminalThemePreviewCard(
              title: 'Default',
              theme: defaultTerminalTheme,
            ),
          ),
        ),
      ),
    );

    final sample = tester.widget<Container>(
      find.byKey(const ValueKey('terminal-theme-preview-sample')),
    );
    final decoration = sample.decoration! as BoxDecoration;

    expect(decoration.color, defaultTerminalTheme.primary.background);
  });

  testWidgets('preview adapts without overflow at narrow widths', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Center(
          child: SizedBox(
            width: 84,
            height: 132,
            child: TerminalThemePreviewCard(
              title: 'Default',
              theme: defaultTerminalTheme,
              compact: true,
            ),
          ),
        ),
      ),
    );

    expect(tester.takeException(), isNull);
  });

  testWidgets('cursor stays immediately after the sample command', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Center(
          child: SizedBox(
            width: 240,
            height: 132,
            child: TerminalThemePreviewCard(
              title: 'Default',
              theme: defaultTerminalTheme,
            ),
          ),
        ),
      ),
    );

    final commandRect = tester.getRect(
      find.byKey(const ValueKey('terminal-theme-preview-command')),
    );
    final cursorRect = tester.getRect(
      find.byKey(const ValueKey('terminal-theme-preview-cursor')),
    );

    expect(cursorRect.left - commandRect.right, closeTo(5, 0.01));
    expect(tester.takeException(), isNull);
  });

  testWidgets('preview sample marker is not localized', (tester) async {
    setAppLanguage(AppLanguage.simplifiedChinese);
    NautermLocalizations.current = const NautermLocalizations(
      Locale('zh', 'CN'),
      messages: {'common.label.sel': '选中'},
    );
    addTearDown(() {
      setAppLanguage(AppLanguage.english);
      NautermLocalizations.current = const NautermLocalizations(Locale('en'));
    });

    await tester.pumpWidget(
      const MaterialApp(
        home: Center(
          child: SizedBox(
            width: 240,
            height: 132,
            child: TerminalThemePreviewCard(
              title: 'Default',
              theme: defaultTerminalTheme,
            ),
          ),
        ),
      ),
    );

    expect(find.text('sel'), findsOneWidget);
    expect(find.text('选中'), findsNothing);
  });
}
