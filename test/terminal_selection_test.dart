import 'dart:ui';

import 'package:flutter/widgets.dart' show EdgeInsets;
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/terminal/terminal_selection.dart';
import 'package:nauterm/terminal/terminal_theme.dart';
import 'package:nauterm/terminal/terminal_widget.dart';

void main() {
  test('word selection treats underscores and hyphens as word characters', () {
    final snapshot = _snapshotFromLines(['abc foo_bar-baz qux']);

    final selection = terminalWordSelectionAt(
      snapshot,
      const TerminalCellPosition(row: 0, column: 7),
    );

    expect(terminalSelectedText(snapshot, selection), 'foo_bar-baz');
  });

  test('word selection stops at punctuation outside the word set', () {
    final snapshot = _snapshotFromLines(['git:main*']);

    final selection = terminalWordSelectionAt(
      snapshot,
      const TerminalCellPosition(row: 0, column: 4),
    );

    expect(terminalSelectedText(snapshot, selection), 'main');
  });

  test('line selection copies a whole row without trailing cell padding', () {
    final snapshot = _snapshotFromLines(['prompt \$ ls  '], columns: 16);

    final selection = TerminalSelection.line(row: 0, snapshot: snapshot);

    expect(terminalSelectedText(snapshot, selection), 'prompt \$ ls');
  });

  test('range selection copies visible text across rows', () {
    final snapshot = _snapshotFromLines(['alpha beta', 'gamma delta']);
    final selection = TerminalSelection.fromCellRange(
      anchor: const TerminalCellPosition(row: 0, column: 6),
      extent: const TerminalCellPosition(row: 1, column: 4),
      snapshot: snapshot,
    );

    expect(terminalSelectedText(snapshot, selection), 'beta\ngamma');
  });

  test('visible text selection stops at the last nonblank cell', () {
    final snapshot = _snapshotFromLines(['alpha', 'beta', ''], columns: 8);

    final selection = terminalVisibleTextSelection(snapshot);

    expect(terminalSelectedText(snapshot, selection), 'alpha\nbeta');
  });

  test('selection coordinates remain stable when the viewport scrolls', () {
    final snapshot = _snapshotFromLines(
      ['history', 'visible'],
      columns: 8,
      historyLines: 10,
      displayOffset: 4,
    );
    final selection = TerminalSelection.fromCellRange(
      anchor: const TerminalCellPosition(row: 0, column: 0),
      extent: const TerminalCellPosition(row: 1, column: 6),
      snapshot: snapshot,
    );

    expect(selection, const TerminalSelection(start: -32, end: -17));
    expect(terminalSelectedText(snapshot, selection), 'history\nvisible');
  });

  test('selection preserves wide and combined grapheme cells', () {
    final snapshot = _snapshotFromCells([
      const TerminalCell(
        text: '表',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: terminalFlagWideChar,
      ),
      const TerminalCell(
        text: ' ',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: terminalFlagWideCharSpacer,
      ),
      const TerminalCell(
        text: 'e\u{301}',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: 0,
      ),
      const TerminalCell(
        text: '👍🏽',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: terminalFlagWideChar,
      ),
      const TerminalCell(
        text: ' ',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: terminalFlagWideCharSpacer,
      ),
      const TerminalCell(
        text: '👨‍👩‍👧‍👦',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: terminalFlagWideChar,
      ),
      const TerminalCell(
        text: ' ',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: terminalFlagWideCharSpacer,
      ),
    ]);

    final selection = TerminalSelection(start: 0, end: snapshot.columns);

    expect(
      terminalSelectedText(snapshot, selection),
      '表e\u{301}👍🏽👨‍👩‍👧‍👦',
    );
  });

  test('structured prompts ignore prompt-like cat output', () {
    final snapshot = _snapshotFromLines([
      r'user@host:~$ cat note.md',
      '# Heading',
      r'$ price',
      'body',
      r'user@host:~$ echo done',
    ]);

    expect(
      terminalCommandBlockAt(
        snapshot,
        const TerminalCellPosition(row: 2, column: 2),
      ),
      TerminalSelection(start: 0, end: snapshot.columns * 4),
    );
  });

  test('HTML closing tags do not split a command block', () {
    final snapshot = _snapshotFromLines([
      r'user@host:~$ cat index.html',
      '<div class="magnetic-wrapper">',
      '</div>',
      '<script>',
      "wrapper.addEventListener('mouseleave', reset);",
      '</script>',
      r'user@host:~$ ',
    ]);

    expect(
      terminalCommandBlockAt(
        snapshot,
        const TerminalCellPosition(row: 4, column: 2),
      ),
      TerminalSelection(start: 0, end: snapshot.columns * 6),
    );
  });

  test('block cursor spans wide cells', () {
    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final snapshot = _snapshotFromCells([
      const TerminalCell(
        text: '表',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: terminalFlagWideChar,
      ),
      const TerminalCell(
        text: ' ',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: terminalFlagWideCharSpacer,
      ),
    ], cursorColumn: 1);
    final painter = TerminalPainter(
      snapshot: snapshot,
      metrics: metrics,
      textStyle: defaultTerminalConfig.font.textStyle(),
      showCursor: true,
      composingText: null,
      selection: null,
      focused: true,
      theme: defaultTerminalTheme,
    );

    expect(
      painter.cursorRectForTesting(),
      Rect.fromLTWH(0, 0, metrics.cellSize.width * 2, metrics.cellSize.height),
    );
  });

  test('terminal cursor is hidden while viewing scrollback', () {
    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final painter = TerminalPainter(
      snapshot: _snapshotFromLines(
        ['history'],
        historyLines: 10,
        displayOffset: 1,
      ),
      metrics: metrics,
      textStyle: defaultTerminalConfig.font.textStyle(),
      showCursor: true,
      composingText: null,
      selection: null,
      focused: true,
      theme: defaultTerminalTheme,
    );

    expect(painter.cursorRectForTesting(), isNull);
  });

  test('Powerline triangles fill their cell symmetrically', () async {
    const capColor = Color(0xff4a72a8);
    const canvasColor = terminalDefaultBackground;
    const metrics = TerminalMetrics(
      cellSize: Size(10, 20),
      textOffset: Offset.zero,
      decorationThickness: 1,
      underlineY: 18,
      strikeoutY: 10,
    );
    final snapshot = _snapshotFromCells(const [
      TerminalCell(
        text: '\uE0B2',
        foreground: capColor,
        background: terminalDefaultBackground,
        flags: 0,
      ),
      TerminalCell(
        text: ' ',
        foreground: terminalDefaultForeground,
        background: capColor,
        flags: 0,
      ),
      TerminalCell(
        text: '\uE0B0',
        foreground: capColor,
        background: terminalDefaultBackground,
        flags: 0,
      ),
    ]);
    final painter = TerminalPainter(
      snapshot: snapshot,
      metrics: metrics,
      textStyle: defaultTerminalConfig.font.textStyle(),
      showCursor: false,
      composingText: null,
      selection: null,
      focused: false,
      theme: defaultTerminalTheme,
    );
    final recorder = PictureRecorder();
    painter.paint(Canvas(recorder), const Size(30, 20));
    final picture = recorder.endRecording();
    final image = await picture.toImage(30, 20);
    final bytes = await image.toByteData(format: ImageByteFormat.rawRgba);
    picture.dispose();
    image.dispose();
    final pixels = bytes!.buffer.asUint8List();

    Color pixelAt(int x, int y) {
      final offset = (y * 30 + x) * 4;
      return Color.fromARGB(
        pixels[offset + 3],
        pixels[offset],
        pixels[offset + 1],
        pixels[offset + 2],
      );
    }

    expect(pixelAt(9, 0), isNot(canvasColor));
    expect(pixelAt(10, 0), capColor);
    expect(pixelAt(19, 0), capColor);
    expect(pixelAt(20, 0), isNot(canvasColor));
    expect(pixelAt(0, 0), canvasColor);
    expect(pixelAt(29, 0), canvasColor);

    // Both slopes mirror around the row center.
    expect(pixelAt(5, 4), pixelAt(5, 15));
    expect(pixelAt(24, 4), pixelAt(24, 15));
  });

  test('Ghostty geometric Powerline sprite set paints every glyph', () async {
    const spriteColor = Color(0xff4a72a8);
    const canvasColor = terminalDefaultBackground;
    const glyphs = [
      '\uE0B0',
      '\uE0B1',
      '\uE0B2',
      '\uE0B3',
      '\uE0B4',
      '\uE0B5',
      '\uE0B6',
      '\uE0B7',
      '\uE0B8',
      '\uE0B9',
      '\uE0BA',
      '\uE0BB',
      '\uE0BC',
      '\uE0BD',
      '\uE0BE',
      '\uE0BF',
      '\uE0D2',
      '\uE0D4',
    ];
    const metrics = TerminalMetrics(
      cellSize: Size(10, 20),
      textOffset: Offset.zero,
      decorationThickness: 1,
      underlineY: 18,
      strikeoutY: 10,
    );
    final snapshot = _snapshotFromCells([
      for (final glyph in glyphs)
        TerminalCell(
          text: glyph,
          foreground: spriteColor,
          background: canvasColor,
          flags: 0,
        ),
    ]);
    final painter = TerminalPainter(
      snapshot: snapshot,
      metrics: metrics,
      textStyle: defaultTerminalConfig.font.textStyle(),
      showCursor: false,
      composingText: null,
      selection: null,
      focused: false,
      theme: defaultTerminalTheme,
    );
    final recorder = PictureRecorder();
    painter.paint(Canvas(recorder), const Size(180, 20));
    final picture = recorder.endRecording();
    final image = await picture.toImage(180, 20);
    final bytes = await image.toByteData(format: ImageByteFormat.rawRgba);
    picture.dispose();
    image.dispose();
    final pixels = bytes!.buffer.asUint8List();

    Color pixelAt(int x, int y) {
      final offset = (y * 180 + x) * 4;
      return Color.fromARGB(
        pixels[offset + 3],
        pixels[offset],
        pixels[offset + 1],
        pixels[offset + 2],
      );
    }

    for (var glyphIndex = 0; glyphIndex < glyphs.length; glyphIndex++) {
      var paintedPixels = 0;
      for (var y = 0; y < 20; y++) {
        for (var x = glyphIndex * 10; x < (glyphIndex + 1) * 10; x++) {
          if (pixelAt(x, y) != canvasColor) paintedPixels++;
        }
      }
      expect(
        paintedPixels,
        greaterThan(2),
        reason: 'Powerline sprite ${glyphs[glyphIndex]} was not painted',
      );
    }
  });

  test('command block focus frame includes horizontal terminal padding', () {
    final snapshot = _snapshotFromLines([
      'first',
      'focus',
      'last ',
    ], columns: 5);
    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final painter = TerminalCommandBlockFocusPainter(
      snapshot: snapshot,
      metrics: metrics,
      selection: const TerminalSelection(start: 5, end: 10),
      theme: defaultTerminalTheme,
      contentPadding: const EdgeInsets.fromLTRB(12, 8, 18, 6),
    );

    final focusRect = painter.focusRectForTesting(const Size(200, 100));

    expect(focusRect.left, 0);
    expect(focusRect.right, 200);
    expect(focusRect.top, 8 + metrics.cellSize.height);
    expect(focusRect.bottom, 8 + metrics.cellSize.height * 2);
  });

  test(
    'command block focus frame paints both viewport edges at scaled DPI',
    () async {
      final snapshot = _snapshotFromLines([
        'first',
        'focus',
        'last ',
      ], columns: 5);
      final metrics = TerminalMetrics.measure(
        defaultTerminalConfig.font.textStyle(),
      );
      final painter = TerminalCommandBlockFocusPainter(
        snapshot: snapshot,
        metrics: metrics,
        selection: const TerminalSelection(start: 5, end: 10),
        theme: defaultTerminalTheme,
        contentPadding: const EdgeInsets.fromLTRB(12, 8, 18, 6),
      );
      const logicalSize = Size(200, 100);
      const scale = 1.25;
      const imageWidth = 250;
      const imageHeight = 125;
      final recorder = PictureRecorder();
      final canvas = Canvas(recorder)..scale(scale);

      painter.paint(canvas, logicalSize);

      final image = await recorder.endRecording().toImage(
        imageWidth,
        imageHeight,
      );
      final bytes = await image.toByteData(format: ImageByteFormat.rawRgba);
      expect(bytes, isNotNull);
      final focusRect = painter.focusRectForTesting(logicalSize);
      final sampleY = (focusRect.center.dy * scale).floor();
      int alphaAt(int x) => bytes!.getUint8((sampleY * imageWidth + x) * 4 + 3);

      expect(alphaAt(0), greaterThan(0));
      expect(alphaAt(imageWidth - 1), greaterThan(0));
    },
  );

  test('terminal painter maps the Nysa native palette', () {
    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final painter = TerminalPainter(
      snapshot: _snapshotFromLines([' ']),
      metrics: metrics,
      textStyle: defaultTerminalConfig.font.textStyle(),
      showCursor: false,
      composingText: null,
      selection: null,
      focused: false,
      theme: nysaDarkTerminalTheme,
    );

    expect(
      painter.resolveColorForTesting(terminalDefaultForeground),
      nysaDarkTerminalTheme.primary.foreground,
    );
    expect(
      painter.resolveColorForTesting(terminalDefaultBackground),
      nysaDarkTerminalTheme.primary.background,
    );
    expect(
      painter.resolveColorForTesting(terminalDefaultCursor),
      nysaDarkTerminalTheme.cursor.cursor,
    );
    expect(
      painter.resolveColorForTesting(nysaLightTerminalTheme.normal.red),
      nysaDarkTerminalTheme.normal.red,
    );
    expect(
      painter.resolveColorForTesting(nysaLightTerminalTheme.bright.blue),
      nysaDarkTerminalTheme.bright.blue,
    );
    expect(
      painter.resolveColorForTesting(nysaLightTerminalTheme.normal.white),
      nysaDarkTerminalTheme.normal.white,
    );
    expect(
      painter.resolveColorForTesting(nysaLightTerminalTheme.bright.white),
      nysaDarkTerminalTheme.bright.white,
    );
  });

  test('ANSI black prompt backgrounds remain distinct from default text', () {
    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final painter = TerminalPainter(
      snapshot: _snapshotFromLines([' ']),
      metrics: metrics,
      textStyle: defaultTerminalConfig.font.textStyle(),
      showCursor: false,
      composingText: null,
      selection: null,
      focused: false,
      theme: nysaDarkTerminalTheme,
    );
    const textCell = TerminalCell(
      text: 'u',
      foreground: terminalDefaultForeground,
      background: terminalDefaultForeground,
      flags: 0,
    );
    const separatorCell = TerminalCell(
      text: '\uE0B0',
      foreground: terminalDefaultForeground,
      background: terminalDefaultBackground,
      flags: 0,
    );

    expect(
      painter.resolveCellForegroundForTesting(textCell),
      nysaDarkTerminalTheme.primary.foreground,
    );
    expect(
      painter.resolveCellBackgroundForTesting(textCell),
      nysaDarkTerminalTheme.normal.black,
    );
    expect(
      painter.resolveCellForegroundForTesting(separatorCell),
      nysaDarkTerminalTheme.normal.black,
    );
  });

  test('terminal text cache reuses only unchanged rows', () {
    final cache = TerminalTextCache()
      ..prepare(rowCount: 1, configuration: const (revision: 1));
    addTearDown(cache.dispose);
    final cells = _snapshotFromLines(['cache']).cells;
    final recorder = PictureRecorder();
    Canvas(recorder);
    final picture = recorder.endRecording();

    cache.store(
      row: 0,
      cells: cells,
      cellOffset: 0,
      cellCount: cells.length,
      cursorTextColumn: -1,
      picture: picture,
    );

    final equivalentCells = [
      for (final cell in cells)
        TerminalCell(
          text: cell.text,
          foreground: cell.foreground,
          background: cell.background,
          flags: cell.flags,
        ),
    ];
    expect(
      cache.lookup(
        row: 0,
        cells: equivalentCells,
        cellOffset: 0,
        cellCount: equivalentCells.length,
        cursorTextColumn: -1,
      ),
      same(picture),
    );

    final changedCells = List<TerminalCell>.of(equivalentCells);
    changedCells[0] = const TerminalCell(
      text: 'C',
      foreground: terminalDefaultForeground,
      background: terminalDefaultBackground,
      flags: 0,
    );
    expect(
      cache.lookup(
        row: 0,
        cells: changedCells,
        cellOffset: 0,
        cellCount: changedCells.length,
        cursorTextColumn: -1,
      ),
      isNull,
    );
    expect(
      cache.lookup(
        row: 0,
        cells: equivalentCells,
        cellOffset: 0,
        cellCount: equivalentCells.length,
        cursorTextColumn: 0,
      ),
      isNull,
    );
  });

  test('cached terminal text renders identically to uncached text', () async {
    final snapshot = _snapshotFromCells([
      const TerminalCell(
        text: 'a',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: 0,
      ),
      const TerminalCell(
        text: 'B',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: terminalFlagBold,
      ),
      const TerminalCell(
        text: '表',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: terminalFlagWideChar,
      ),
      const TerminalCell(
        text: ' ',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: terminalFlagWideCharSpacer,
      ),
      const TerminalCell(
        text: 'e\u0301',
        foreground: terminalDefaultForeground,
        background: terminalDefaultBackground,
        flags: 0,
      ),
    ]);
    final textStyle = defaultTerminalConfig.font.textStyle();
    final metrics = TerminalMetrics.measure(textStyle);
    final cache = TerminalTextCache();
    addTearDown(cache.dispose);

    TerminalPainter painter(TerminalTextCache? textCache) => TerminalPainter(
      snapshot: snapshot,
      metrics: metrics,
      textStyle: textStyle,
      showCursor: false,
      composingText: null,
      selection: null,
      focused: false,
      theme: defaultTerminalTheme,
      textCache: textCache,
    );

    Future<List<int>> render(TerminalPainter terminalPainter) async {
      final width = (snapshot.columns * metrics.cellSize.width).ceil();
      final height = metrics.cellSize.height.ceil();
      final recorder = PictureRecorder();
      terminalPainter.paint(
        Canvas(recorder),
        Size(width.toDouble(), height.toDouble()),
      );
      final picture = recorder.endRecording();
      final image = await picture.toImage(width, height);
      final bytes = await image.toByteData(format: ImageByteFormat.rawRgba);
      picture.dispose();
      image.dispose();
      return bytes!.buffer.asUint8List();
    }

    final uncached = await render(painter(null));
    await render(painter(cache));
    final cached = await render(painter(cache));

    expect(cached, orderedEquals(uncached));
  });
}

TerminalSnapshot _snapshotFromLines(
  List<String> lines, {
  int? columns,
  int historyLines = 0,
  int displayOffset = 0,
}) {
  final resolvedColumns =
      columns ??
      lines.map((line) => line.length).reduce((a, b) => a > b ? a : b);
  final cells = <TerminalCell>[];

  for (final line in lines) {
    for (var column = 0; column < resolvedColumns; column++) {
      cells.add(
        TerminalCell(
          text: column < line.length ? line[column] : ' ',
          foreground: terminalDefaultForeground,
          background: terminalDefaultBackground,
          flags: 0,
        ),
      );
    }
  }

  return TerminalSnapshot(
    columns: resolvedColumns,
    rows: lines.length,
    historyLines: historyLines,
    displayOffset: displayOffset,
    cells: cells,
    cursor: const TerminalCursor(
      column: 0,
      row: 0,
      visible: true,
      shape: TerminalCursorShape.block,
      color: terminalDefaultCursor,
      blinking: false,
    ),
    keyboardMode: const TerminalKeyboardMode(),
  );
}

TerminalSnapshot _snapshotFromCells(
  List<TerminalCell> cells, {
  int cursorColumn = 0,
}) {
  return TerminalSnapshot(
    columns: cells.length,
    rows: 1,
    cells: cells,
    cursor: TerminalCursor(
      column: cursorColumn,
      row: 0,
      visible: true,
      shape: TerminalCursorShape.block,
      color: terminalDefaultCursor,
      blinking: false,
    ),
    keyboardMode: const TerminalKeyboardMode(),
  );
}
