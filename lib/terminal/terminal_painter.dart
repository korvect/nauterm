part of 'terminal_widget.dart';

class TerminalMetrics {
  const TerminalMetrics({
    required this.cellSize,
    required this.textOffset,
    required this.decorationThickness,
    required this.underlineY,
    required this.strikeoutY,
  });

  final Size cellSize;
  final Offset textOffset;
  final double decorationThickness;
  final double underlineY;
  final double strikeoutY;

  static TerminalMetrics measure(TextStyle style) {
    final painter = TextPainter(
      text: TextSpan(text: 'W', style: style),
      textDirection: TextDirection.ltr,
    )..layout();

    final line = painter.computeLineMetrics().single;
    final cellWidth = painter.width.ceilToDouble();
    final cellHeight = painter.height.ceilToDouble();
    final textOffset = Offset(
      0,
      ((cellHeight - painter.height) / 2).floorToDouble(),
    );
    final thickness = math.max(
      1.0,
      ((style.fontSize ?? 14) / 14).roundToDouble(),
    );
    final baseline = textOffset.dy + line.baseline;

    return TerminalMetrics(
      cellSize: Size(cellWidth, cellHeight),
      textOffset: textOffset,
      decorationThickness: thickness,
      underlineY: math.min(cellHeight - thickness, baseline + 2),
      strikeoutY: math.max(thickness, baseline - line.ascent * 0.36),
    );
  }
}

enum _TerminalGraphicLayer { belowBackground, belowText, aboveText }

class TerminalPainter extends CustomPainter {
  TerminalPainter({
    required this.snapshot,
    required this.metrics,
    required this.textStyle,
    required this.showCursor,
    required this.composingText,
    required this.selection,
    this.commandBlockSelection,
    this.paintCommandBlockFocus = true,
    this.paintCommandBlockVerticalBorders = true,
    this.openTargetSelection,
    required this.focused,
    required this.theme,
    this.predictedText = '',
    this.weight = 400,
    this.boldWeight = 700,
    this.graphicImages = const {},
    this.textCache,
  });

  final TerminalSnapshot snapshot;
  final TerminalMetrics metrics;
  final TextStyle textStyle;
  final bool showCursor;
  final String? composingText;
  final TerminalSelection? selection;
  final TerminalSelection? commandBlockSelection;
  final bool paintCommandBlockFocus;
  final bool paintCommandBlockVerticalBorders;
  final TerminalSelection? openTargetSelection;
  final bool focused;
  final TerminalTheme theme;
  final String predictedText;
  final int weight;
  final int boldWeight;
  final Map<int, ui.Image> graphicImages;
  final TerminalTextCache? textCache;

  @override
  void paint(Canvas canvas, Size size) {
    final cellWidth = metrics.cellSize.width;
    final cellHeight = metrics.cellSize.height;
    final viewport = Offset.zero & size;

    canvas
      ..clipRect(viewport)
      ..drawRect(viewport, Paint()..color = theme.primary.background);

    final cursorRect = _cursorRect(cellWidth, cellHeight);
    _paintGraphics(
      canvas,
      cellWidth,
      cellHeight,
      _TerminalGraphicLayer.belowBackground,
    );
    _paintCellBackgrounds(canvas, cellWidth, cellHeight);
    _paintGraphics(
      canvas,
      cellWidth,
      cellHeight,
      _TerminalGraphicLayer.belowText,
    );
    _paintSelectionBackgrounds(canvas, cellWidth, cellHeight);
    _paintBlockCursorFill(canvas, cursorRect);
    _paintText(canvas, cellWidth, cellHeight);
    _paintCellDecorations(canvas, cellWidth, cellHeight);
    _paintGraphics(
      canvas,
      cellWidth,
      cellHeight,
      _TerminalGraphicLayer.aboveText,
    );
    _paintPredictedText(canvas, cellWidth, cellHeight);
    _paintImeComposingText(canvas, cellWidth, cellHeight);
    _paintCursor(canvas, cellWidth, cellHeight);
    if (paintCommandBlockFocus) {
      _paintCommandBlockFocus(canvas, cellWidth, cellHeight);
    }
    _paintOpenTargetUnderline(canvas, cellWidth, cellHeight);
  }

  void _paintGraphics(
    Canvas canvas,
    double cellWidth,
    double cellHeight,
    _TerminalGraphicLayer layer,
  ) {
    if (snapshot.graphicPlacements.isEmpty || graphicImages.isEmpty) {
      return;
    }
    final imagesById = {
      for (final image in snapshot.graphicImages) image.id: image,
    };
    for (final placement in snapshot.graphicPlacements) {
      final placementLayer = placement.zIndex < -0x40000000
          ? _TerminalGraphicLayer.belowBackground
          : placement.zIndex < 0
          ? _TerminalGraphicLayer.belowText
          : _TerminalGraphicLayer.aboveText;
      if (placementLayer != layer) continue;
      final metadata = imagesById[placement.imageId];
      if (metadata == null) continue;
      final image = graphicImages[metadata.generation];
      if (image == null) continue;
      final sourceWidth = placement.sourceWidth == 0
          ? metadata.width
          : placement.sourceWidth;
      final sourceHeight = placement.sourceHeight == 0
          ? metadata.height
          : placement.sourceHeight;
      final source =
          Rect.fromLTWH(
            placement.sourceX.toDouble(),
            placement.sourceY.toDouble(),
            sourceWidth.toDouble(),
            sourceHeight.toDouble(),
          ).intersect(
            Rect.fromLTWH(
              0,
              0,
              metadata.width.toDouble(),
              metadata.height.toDouble(),
            ),
          );
      if (source.isEmpty) continue;
      final destination = Rect.fromLTWH(
        placement.viewportColumn * cellWidth,
        placement.viewportRow * cellHeight,
        placement.columns * cellWidth,
        placement.rows * cellHeight,
      );
      if (destination.isEmpty) continue;
      canvas.drawImageRect(
        image,
        source,
        destination,
        Paint()..filterQuality = FilterQuality.medium,
      );
    }
  }

  void _paintOpenTargetUnderline(
    Canvas canvas,
    double cellWidth,
    double cellHeight,
  ) {
    final target = openTargetSelection;
    if (target == null || target.isCollapsed) {
      return;
    }
    final viewportStart = -snapshot.displayOffset * snapshot.columns;
    final totalCells = snapshot.rows * snapshot.columns;
    final start = (target.start - viewportStart).clamp(0, totalCells).toInt();
    final end = (target.end - viewportStart).clamp(0, totalCells).toInt();
    if (start >= end) {
      return;
    }
    final startRow = start ~/ snapshot.columns;
    final endRow = (end - 1) ~/ snapshot.columns;
    final paint = Paint()
      ..color = theme.primary.foreground
      ..strokeWidth = math.max(1, metrics.decorationThickness);
    for (var row = startRow; row <= endRow; row++) {
      final rowStart = row * snapshot.columns;
      final startColumn = row == startRow ? start - rowStart : 0;
      final endColumn = row == endRow ? end - rowStart : snapshot.columns;
      final y = row * cellHeight + metrics.underlineY;
      canvas.drawLine(
        Offset(startColumn * cellWidth, y),
        Offset(endColumn * cellWidth, y),
        paint,
      );
    }
  }

  void _paintCommandBlockFocus(
    Canvas canvas,
    double cellWidth,
    double cellHeight,
  ) {
    final block = commandBlockSelection;
    if (block == null || block.isCollapsed) {
      return;
    }
    final viewportStart = -snapshot.displayOffset * snapshot.columns;
    final totalCells = snapshot.rows * snapshot.columns;
    final start = (block.start - viewportStart).clamp(0, totalCells).toInt();
    final end = (block.end - viewportStart).clamp(0, totalCells).toInt();
    final width = snapshot.columns * cellWidth;
    final height = snapshot.rows * cellHeight;
    final blockTop = (start ~/ snapshot.columns) * cellHeight;
    final blockBottom =
        ((end + snapshot.columns - 1) ~/ snapshot.columns) * cellHeight;
    final dimOpacity = theme.primary.background.computeLuminance() > 0.5
        ? 0.20
        : 0.30;
    final dim = Paint()..color = Colors.black.withValues(alpha: dimOpacity);
    if (blockTop > 0) {
      canvas.drawRect(Rect.fromLTWH(0, 0, width, blockTop), dim);
    }
    if (blockBottom < height) {
      canvas.drawRect(
        Rect.fromLTWH(0, blockBottom, width, height - blockBottom),
        dim,
      );
    }
    final border = Paint()
      ..color = theme.primary.accent.withValues(alpha: 0.55)
      ..style = PaintingStyle.fill;
    const borderWidth = 1.0;
    final visibleBlockTop = blockTop.clamp(0, height).toDouble();
    final visibleBlockBottom = blockBottom.clamp(0, height).toDouble();
    if (block.start >= viewportStart && blockTop < height) {
      canvas.drawRect(Rect.fromLTWH(0, blockTop, width, borderWidth), border);
    }
    if (block.end <= viewportStart + totalCells && blockBottom > 0) {
      canvas.drawRect(
        Rect.fromLTWH(
          0,
          math.max(0, blockBottom - borderWidth),
          width,
          borderWidth,
        ),
        border,
      );
    }
    if (paintCommandBlockVerticalBorders &&
        visibleBlockTop < visibleBlockBottom) {
      canvas
        ..drawRect(
          Rect.fromLTWH(
            0,
            visibleBlockTop,
            borderWidth,
            visibleBlockBottom - visibleBlockTop,
          ),
          border,
        )
        ..drawRect(
          Rect.fromLTWH(
            math.max(0, width - borderWidth),
            visibleBlockTop,
            borderWidth,
            visibleBlockBottom - visibleBlockTop,
          ),
          border,
        );
    }
  }

  void _paintPredictedText(Canvas canvas, double cellWidth, double cellHeight) {
    if (predictedText.isEmpty || snapshot.displayOffset != 0) {
      return;
    }
    var column = _cursorTextColumn() ?? snapshot.cursor.column;
    var row = snapshot.cursor.row;
    final painter = TextPainter(textDirection: TextDirection.ltr, maxLines: 1);
    final style = textStyle.copyWith(
      color: theme.primary.foreground.withValues(alpha: 0.72),
      fontWeight:
          FontWeight.values[((weight / 100).round() - 1).clamp(0, 8).toInt()],
    );
    final underline = Paint()
      ..color = theme.primary.foreground.withValues(alpha: 0.42)
      ..strokeWidth = metrics.decorationThickness;

    canvas.save();
    canvas.clipRect(
      Rect.fromLTWH(
        0,
        0,
        snapshot.columns * cellWidth,
        snapshot.rows * cellHeight,
      ),
    );
    for (final text in predictedText.characters) {
      if (row >= snapshot.rows) {
        break;
      }
      final span = terminalGraphemeCellWidth(text);
      if (span == 0) {
        continue;
      }
      if (column + span > snapshot.columns) {
        column = 0;
        row++;
        if (row >= snapshot.rows) {
          break;
        }
      }
      painter
        ..text = TextSpan(text: text, style: style)
        ..layout();
      painter.paint(
        canvas,
        Offset(column * cellWidth, row * cellHeight + metrics.textOffset.dy),
      );
      final underlineY = row * cellHeight + metrics.underlineY;
      canvas.drawLine(
        Offset(column * cellWidth, underlineY),
        Offset((column + span) * cellWidth, underlineY),
        underline,
      );
      column += span;
      if (column >= snapshot.columns) {
        column = 0;
        row++;
      }
    }
    if (row < snapshot.rows && focused) {
      canvas.drawRect(
        Rect.fromLTWH(
          column * cellWidth,
          row * cellHeight + 2,
          math.max(1, metrics.decorationThickness),
          math.max(0, cellHeight - 4),
        ),
        Paint()..color = theme.cursor.cursor.withValues(alpha: 0.9),
      );
    }
    canvas.restore();
  }

  @visibleForTesting
  Rect? cursorRectForTesting() {
    return _cursorRect(metrics.cellSize.width, metrics.cellSize.height);
  }

  void _paintCellBackgrounds(
    Canvas canvas,
    double cellWidth,
    double cellHeight,
  ) {
    final paint = Paint();
    final columns = snapshot.columns;
    final cells = snapshot.cells;
    for (var row = 0; row < snapshot.rows; row++) {
      final rowOffset = row * columns;
      var runStart = -1;
      var runColor = theme.primary.background;

      void flushRun(int endColumn) {
        if (runStart < 0) {
          return;
        }

        paint.color = runColor;
        canvas.drawRect(
          Rect.fromLTWH(
            runStart * cellWidth,
            row * cellHeight,
            (endColumn - runStart) * cellWidth,
            cellHeight,
          ),
          paint,
        );
        runStart = -1;
      }

      for (var column = 0; column <= columns; column++) {
        final background = column < columns
            ? _resolveCellBackground(cells[rowOffset + column])
            : theme.primary.background;
        if (background == theme.primary.background) {
          flushRun(column);
          continue;
        }

        if (runStart < 0) {
          runStart = column;
          runColor = background;
          continue;
        }

        if (background != runColor) {
          flushRun(column);
          runStart = column;
          runColor = background;
        }
      }
    }
  }

  void _paintSelectionBackgrounds(
    Canvas canvas,
    double cellWidth,
    double cellHeight,
  ) {
    final currentSelection = selection;
    if (currentSelection == null || currentSelection.isCollapsed) {
      return;
    }

    final viewportStart = -snapshot.displayOffset * snapshot.columns;
    final totalCells = snapshot.columns * snapshot.rows;
    final start = (currentSelection.start - viewportStart)
        .clamp(0, totalCells)
        .toInt();
    final end = (currentSelection.end - viewportStart)
        .clamp(0, totalCells)
        .toInt();
    if (start >= end) {
      return;
    }

    final paint = Paint()..color = theme.selection.background;
    final startRow = start ~/ snapshot.columns;
    final endRow = (end - 1) ~/ snapshot.columns;
    for (var row = startRow; row <= endRow; row++) {
      final rowStart = row * snapshot.columns;
      final startColumn = row == startRow ? start - rowStart : 0;
      final endColumn = row == endRow ? end - rowStart : snapshot.columns;
      if (startColumn >= endColumn) {
        continue;
      }

      canvas.drawRect(
        Rect.fromLTWH(
          startColumn * cellWidth,
          row * cellHeight,
          (endColumn - startColumn) * cellWidth,
          cellHeight,
        ),
        paint,
      );
    }
  }

  void _paintText(Canvas canvas, double cellWidth, double cellHeight) {
    final styleCache = <_CellStyleKey, TextStyle>{};
    final runStyleCache = <_CellStyleKey, TextStyle>{};
    final columns = snapshot.columns;
    final cells = snapshot.cells;
    final cursor = snapshot.cursor;
    final cursorTextColumn = _cursorTextColumn();
    final overrideCursorCell =
        snapshot.displayOffset == 0 &&
        focused &&
        showCursor &&
        cursor.visible &&
        cursor.shape == TerminalCursorShape.block &&
        cursor.row >= 0 &&
        cursor.row < snapshot.rows &&
        cursorTextColumn != null;
    final painter = TextPainter(textDirection: TextDirection.ltr, maxLines: 1);
    textCache?.prepare(
      rowCount: snapshot.rows,
      configuration: (
        columns: snapshot.columns,
        displayOffset: snapshot.displayOffset,
        textStyle: textStyle,
        theme: theme,
        cellSize: metrics.cellSize,
        textOffset: metrics.textOffset,
        selection: selection,
        weight: weight,
        boldWeight: boldWeight,
      ),
    );

    TextStyle resolvedStyle(_CellStyleKey style) {
      return styleCache.putIfAbsent(
        style,
        () => style.resolve(textStyle, weight: weight, boldWeight: boldWeight),
      );
    }

    TextStyle resolvedRunStyle(_CellStyleKey style) {
      return runStyleCache.putIfAbsent(style, () {
        final resolved = resolvedStyle(style);
        final probe = TextPainter(
          text: TextSpan(text: 'W', style: resolved),
          textDirection: TextDirection.ltr,
          maxLines: 1,
        )..layout();
        final spacingCorrection = cellWidth - probe.width;
        if (spacingCorrection.abs() < 0.001) {
          return resolved;
        }
        return resolved.copyWith(
          letterSpacing: (resolved.letterSpacing ?? 0) + spacingCorrection,
        );
      });
    }

    void paintText(
      Canvas targetCanvas,
      String text,
      _CellStyleKey style,
      int column,
      double top, {
      required bool isRun,
    }) {
      painter
        ..text = TextSpan(
          text: text,
          style: isRun ? resolvedRunStyle(style) : resolvedStyle(style),
        )
        ..layout();
      painter.paint(
        targetCanvas,
        Offset(column * cellWidth, top + metrics.textOffset.dy),
      );
    }

    for (var row = 0; row < snapshot.rows; row++) {
      final rowOffset = row * columns;
      final rowCursorTextColumn = overrideCursorCell && row == cursor.row
          ? cursorTextColumn
          : -1;
      final cachedPicture = textCache?.lookup(
        row: row,
        cells: cells,
        cellOffset: rowOffset,
        cellCount: columns,
        cursorTextColumn: rowCursorTextColumn,
      );
      if (cachedPicture != null) {
        canvas
          ..save()
          ..translate(0, row * cellHeight)
          ..drawPicture(cachedPicture)
          ..restore();
        continue;
      }

      final recorder = textCache == null ? null : ui.PictureRecorder();
      final rowCanvas = recorder == null ? canvas : Canvas(recorder);
      final rowTop = recorder == null ? row * cellHeight : 0.0;
      var runStart = -1;
      _CellStyleKey? runStyle;
      final runText = StringBuffer();

      void flushRun() {
        if (runStart < 0 || runStyle == null || runText.isEmpty) {
          return;
        }
        paintText(
          rowCanvas,
          runText.toString(),
          runStyle!,
          runStart,
          rowTop,
          isRun: true,
        );
        runStart = -1;
        runStyle = null;
        runText.clear();
      }

      rowCanvas.save();
      rowCanvas.clipRect(
        Rect.fromLTWH(0, rowTop, columns * cellWidth, cellHeight),
      );
      for (var column = 0; column < columns; column++) {
        final cell = cells[rowOffset + column];
        if (cell.wideCharSpacer || cell.leadingWideCharSpacer) {
          flushRun();
          continue;
        }

        final text = cell.hidden || cell.text.isEmpty ? ' ' : cell.text;
        final isCursorCell =
            overrideCursorCell &&
            row == cursor.row &&
            column == cursorTextColumn;
        final isSelected = _isSelectedCell(row, column);
        if (!isCursorCell && _isBlankText(text)) {
          if (runStart >= 0) {
            runText.write(' ');
          }
          continue;
        }

        final style = _textStyleForCell(
          cell: cell,
          isCursorCell: isCursorCell,
          isSelected: isSelected,
        );
        final canJoinRun = !cell.wideChar && _isSingleWidthAscii(text);
        if (!canJoinRun) {
          flushRun();
          paintText(rowCanvas, text, style, column, rowTop, isRun: false);
          continue;
        }

        if (runStart >= 0 && runStyle != style) {
          flushRun();
        }
        if (runStart < 0) {
          runStart = column;
          runStyle = style;
        }
        runText.write(text);
      }

      flushRun();
      rowCanvas.restore();

      if (recorder != null) {
        final picture = recorder.endRecording();
        textCache!.store(
          row: row,
          cells: cells,
          cellOffset: rowOffset,
          cellCount: columns,
          cursorTextColumn: rowCursorTextColumn,
          picture: picture,
        );
        canvas
          ..save()
          ..translate(0, row * cellHeight)
          ..drawPicture(picture)
          ..restore();
      }
    }
  }

  _CellStyleKey _textStyleForCell({
    required TerminalCell cell,
    required bool isCursorCell,
    required bool isSelected,
  }) {
    if (isCursorCell) {
      return _CellStyleKey.cursor(theme.cursor.text, cell);
    }
    if (isSelected) {
      return _CellStyleKey.selection(theme.selection.text, cell);
    }

    return _CellStyleKey.fromCell(cell, _resolveCellForeground(cell));
  }

  bool _isSelectedCell(int row, int column) {
    return selection?.containsViewportCell(
          row: row,
          column: column,
          snapshot: snapshot,
        ) ??
        false;
  }

  void _paintCellDecorations(
    Canvas canvas,
    double cellWidth,
    double cellHeight,
  ) {
    final columns = snapshot.columns;
    final cells = snapshot.cells;

    for (var row = 0; row < snapshot.rows; row++) {
      final rowOffset = row * columns;
      var runStart = -1;
      _CellDecorationKey? runStyle;

      void flushRun(int endColumn) {
        final style = runStyle;
        if (runStart < 0 || style == null || !style.hasDecoration) {
          runStart = -1;
          runStyle = null;
          return;
        }

        final left = runStart * cellWidth;
        final right = endColumn * cellWidth;
        final baseTop = row * cellHeight;
        if (style.underline != _UnderlineStyle.none) {
          _paintUnderline(canvas, style, left, right, baseTop);
        }
        if (style.strikeout) {
          _paintStraightLine(
            canvas,
            style.color,
            left,
            right,
            baseTop + metrics.strikeoutY,
          );
        }

        runStart = -1;
        runStyle = null;
      }

      for (var column = 0; column <= columns; column++) {
        final style = column < columns
            ? _CellDecorationKey.fromCell(
                cells[rowOffset + column],
                _resolveCellForeground(cells[rowOffset + column]),
              )
            : const _CellDecorationKey.none();

        if (!style.hasDecoration) {
          flushRun(column);
          continue;
        }

        if (runStart < 0) {
          runStart = column;
          runStyle = style;
          continue;
        }

        if (style != runStyle) {
          flushRun(column);
          runStart = column;
          runStyle = style;
        }
      }
    }
  }

  void _paintUnderline(
    Canvas canvas,
    _CellDecorationKey style,
    double left,
    double right,
    double baseTop,
  ) {
    final y = baseTop + metrics.underlineY;
    switch (style.underline) {
      case _UnderlineStyle.none:
        return;
      case _UnderlineStyle.single:
        _paintStraightLine(canvas, style.color, left, right, y);
      case _UnderlineStyle.double:
        final gap = math.max(2.0, metrics.decorationThickness * 2);
        _paintStraightLine(canvas, style.color, left, right, y - gap / 2);
        _paintStraightLine(canvas, style.color, left, right, y + gap / 2);
      case _UnderlineStyle.curly:
        _paintUndercurl(canvas, style.color, left, right, y);
      case _UnderlineStyle.dotted:
        _paintDottedLine(canvas, style.color, left, right, y);
      case _UnderlineStyle.dashed:
        _paintDashedLine(canvas, style.color, left, right, y);
    }
  }

  void _paintStraightLine(
    Canvas canvas,
    Color color,
    double left,
    double right,
    double y,
  ) {
    final paint = Paint()
      ..color = color
      ..strokeWidth = metrics.decorationThickness
      ..strokeCap = StrokeCap.square;
    canvas.drawLine(Offset(left, y), Offset(right, y), paint);
  }

  void _paintDottedLine(
    Canvas canvas,
    Color color,
    double left,
    double right,
    double y,
  ) {
    final paint = Paint()..color = color;
    final radius = metrics.decorationThickness * 0.55;
    final step = math.max(radius * 3, metrics.cellSize.width * 0.45);
    for (var x = left + radius; x < right; x += step) {
      canvas.drawCircle(Offset(x, y), radius, paint);
    }
  }

  void _paintDashedLine(
    Canvas canvas,
    Color color,
    double left,
    double right,
    double y,
  ) {
    final paint = Paint()
      ..color = color
      ..strokeWidth = metrics.decorationThickness
      ..strokeCap = StrokeCap.square;
    final dash = math.max(3.0, metrics.cellSize.width * 0.7);
    final gap = math.max(2.0, metrics.cellSize.width * 0.35);
    for (var x = left; x < right; x += dash + gap) {
      canvas.drawLine(
        Offset(x, y),
        Offset(math.min(x + dash, right), y),
        paint,
      );
    }
  }

  void _paintUndercurl(
    Canvas canvas,
    Color color,
    double left,
    double right,
    double y,
  ) {
    if (right <= left) {
      return;
    }

    final paint = Paint()
      ..color = color
      ..strokeWidth = metrics.decorationThickness
      ..style = PaintingStyle.stroke;
    final amplitude = math.max(1.0, metrics.decorationThickness * 1.3);
    final wavelength = math.max(6.0, metrics.cellSize.width * 1.5);
    final path = Path()..moveTo(left, y);
    for (var x = left; x <= right; x += 1.5) {
      final phase = ((x - left) / wavelength) * math.pi * 2;
      path.lineTo(x, y + math.sin(phase) * amplitude);
    }
    canvas.drawPath(path, paint);
  }

  void _paintBlockCursorFill(Canvas canvas, Rect? cursorRect) {
    final cursor = snapshot.cursor;
    if (cursorRect == null ||
        !focused ||
        !showCursor ||
        cursor.shape != TerminalCursorShape.block) {
      return;
    }

    canvas.drawRect(cursorRect, Paint()..color = _resolveColor(cursor.color));
  }

  void _paintCursor(Canvas canvas, double cellWidth, double cellHeight) {
    final cursor = snapshot.cursor;
    final rect = _cursorRect(cellWidth, cellHeight);
    if (rect == null || !showCursor) {
      return;
    }

    final cursorColor = _resolveColor(cursor.color);
    final color = focused ? cursorColor : cursorColor.withValues(alpha: 0.60);
    switch (cursor.shape) {
      case TerminalCursorShape.block:
        if (!focused) {
          _paintHollowCursor(canvas, rect, color);
        }
      case TerminalCursorShape.hollowBlock:
        _paintHollowCursor(canvas, rect, color);
      case TerminalCursorShape.underline:
        final height = math.max(1.0, math.min(3.0, cellHeight * 0.16));
        canvas.drawRect(
          Rect.fromLTWH(rect.left, rect.bottom - height, rect.width, height),
          Paint()..color = color,
        );
      case TerminalCursorShape.beam:
        final width = math.max(1.0, math.min(2.0, cellWidth * 0.18));
        canvas.drawRect(
          Rect.fromLTWH(rect.left, rect.top, width, rect.height),
          Paint()..color = color,
        );
    }
  }

  void _paintImeComposingText(
    Canvas canvas,
    double cellWidth,
    double cellHeight,
  ) {
    final text = composingText;
    if (snapshot.displayOffset != 0 || text == null || text.isEmpty) {
      return;
    }

    final cursor = snapshot.cursor;
    if (cursor.row < 0 ||
        cursor.row >= snapshot.rows ||
        cursor.column < 0 ||
        cursor.column >= snapshot.columns) {
      return;
    }

    final cursorColumn = _cursorTextColumn() ?? cursor.column;
    final remainingWidth = (snapshot.columns - cursorColumn) * cellWidth;
    if (remainingWidth <= 0) {
      return;
    }

    final painter = TextPainter(
      text: TextSpan(
        text: text,
        style: textStyle.copyWith(color: theme.primary.foreground),
      ),
      textDirection: TextDirection.ltr,
      maxLines: 1,
    )..layout(maxWidth: remainingWidth);

    final left = cursorColumn * cellWidth;
    final top = cursor.row * cellHeight;
    final width = math.min(
      remainingWidth,
      math.max(cellWidth, painter.width.ceilToDouble()),
    );

    canvas.save();
    canvas.clipRect(Rect.fromLTWH(left, top, remainingWidth, cellHeight));
    canvas.drawRect(
      Rect.fromLTWH(left, top, width, cellHeight),
      Paint()..color = theme.primary.background,
    );
    painter.paint(canvas, Offset(left, top + metrics.textOffset.dy));
    _paintStraightLine(
      canvas,
      theme.primary.accent,
      left,
      left + width,
      top + metrics.underlineY,
    );
    canvas.restore();
  }

  Rect? _cursorRect(double cellWidth, double cellHeight) {
    final cursor = snapshot.cursor;
    if (snapshot.displayOffset != 0 ||
        cursor.column < 0 ||
        cursor.row < 0 ||
        cursor.column >= snapshot.columns ||
        cursor.row >= snapshot.rows) {
      return null;
    }

    final cursorColumn = _cursorTextColumn() ?? cursor.column;
    final cell = snapshot.cellAt(cursor.row, cursorColumn);
    final cellSpan = cell.wideChar ? 2 : 1;
    final rightColumn = math.min(snapshot.columns, cursorColumn + cellSpan);

    return Rect.fromLTWH(
      cursorColumn * cellWidth,
      cursor.row * cellHeight,
      (rightColumn - cursorColumn) * cellWidth,
      cellHeight,
    );
  }

  int? _cursorTextColumn() {
    final cursor = snapshot.cursor;
    if (snapshot.displayOffset != 0 ||
        cursor.column < 0 ||
        cursor.row < 0 ||
        cursor.column >= snapshot.columns ||
        cursor.row >= snapshot.rows) {
      return null;
    }

    final cell = snapshot.cellAt(cursor.row, cursor.column);
    if (cell.wideCharSpacer && cursor.column > 0) {
      return cursor.column - 1;
    }
    if (cell.leadingWideCharSpacer && cursor.column + 1 < snapshot.columns) {
      return cursor.column + 1;
    }
    return cursor.column;
  }

  void _paintHollowCursor(Canvas canvas, Rect rect, Color color) {
    final strokeWidth = math.max(1.0, metrics.decorationThickness);
    final paint = Paint()
      ..color = color
      ..strokeWidth = strokeWidth
      ..style = PaintingStyle.stroke;
    canvas.drawRect(rect.deflate(strokeWidth / 2), paint);
  }

  Color _resolveCellForeground(TerminalCell cell) {
    return _resolveColor(cell.effectiveForeground);
  }

  Color _resolveCellBackground(TerminalCell cell) {
    return _resolveColor(cell.effectiveBackground);
  }

  Color _resolveColor(Color color) {
    if (color == terminalDefaultForeground) {
      return theme.primary.foreground;
    }
    if (color == terminalDefaultBackground) {
      return theme.primary.background;
    }
    if (color == terminalDefaultCursor) {
      return theme.cursor.cursor;
    }

    final themedAnsi = _resolveAnsiColor(color);
    return themedAnsi ?? color;
  }

  Color? _resolveAnsiColor(Color color) {
    final defaultTheme = defaultTerminalTheme;
    if (color == defaultTheme.normal.black) {
      return theme.normal.black;
    }
    if (color == defaultTheme.normal.red) {
      return theme.normal.red;
    }
    if (color == defaultTheme.normal.green) {
      return theme.normal.green;
    }
    if (color == defaultTheme.normal.yellow) {
      return theme.normal.yellow;
    }
    if (color == defaultTheme.normal.blue) {
      return theme.normal.blue;
    }
    if (color == defaultTheme.normal.magenta) {
      return theme.normal.magenta;
    }
    if (color == defaultTheme.normal.cyan) {
      return theme.normal.cyan;
    }
    if (color == defaultTheme.normal.white) {
      return theme.normal.white;
    }
    if (color == defaultTheme.bright.black) {
      return theme.bright.black;
    }
    if (color == defaultTheme.bright.red) {
      return theme.bright.red;
    }
    if (color == defaultTheme.bright.green) {
      return theme.bright.green;
    }
    if (color == defaultTheme.bright.yellow) {
      return theme.bright.yellow;
    }
    if (color == defaultTheme.bright.blue) {
      return theme.bright.blue;
    }
    if (color == defaultTheme.bright.magenta) {
      return theme.bright.magenta;
    }
    if (color == defaultTheme.bright.cyan) {
      return theme.bright.cyan;
    }
    if (color == defaultTheme.bright.white) {
      return theme.bright.white;
    }
    return null;
  }

  @visibleForTesting
  Color resolveColorForTesting(Color color) => _resolveColor(color);

  @override
  bool shouldRepaint(covariant TerminalPainter oldDelegate) {
    return oldDelegate.snapshot != snapshot ||
        oldDelegate.showCursor != showCursor ||
        oldDelegate.composingText != composingText ||
        oldDelegate.predictedText != predictedText ||
        oldDelegate.selection != selection ||
        oldDelegate.commandBlockSelection != commandBlockSelection ||
        oldDelegate.paintCommandBlockFocus != paintCommandBlockFocus ||
        oldDelegate.paintCommandBlockVerticalBorders !=
            paintCommandBlockVerticalBorders ||
        oldDelegate.openTargetSelection != openTargetSelection ||
        oldDelegate.focused != focused ||
        oldDelegate.theme != theme ||
        oldDelegate.metrics.cellSize != metrics.cellSize ||
        oldDelegate.metrics.textOffset != metrics.textOffset ||
        oldDelegate.metrics.decorationThickness !=
            metrics.decorationThickness ||
        oldDelegate.metrics.underlineY != metrics.underlineY ||
        oldDelegate.metrics.strikeoutY != metrics.strikeoutY ||
        oldDelegate.weight != weight ||
        oldDelegate.boldWeight != boldWeight ||
        oldDelegate.graphicImages != graphicImages ||
        oldDelegate.textCache != textCache;
  }
}

class TerminalCommandBlockFocusPainter extends CustomPainter {
  const TerminalCommandBlockFocusPainter({
    required this.snapshot,
    required this.metrics,
    required this.selection,
    required this.theme,
    required this.contentPadding,
  });

  final TerminalSnapshot snapshot;
  final TerminalMetrics metrics;
  final TerminalSelection selection;
  final TerminalTheme theme;
  final EdgeInsets contentPadding;

  Rect focusRectForTesting(Size size) => _focusRect(size);

  Rect _focusRect(Size size) {
    final viewportStart = -snapshot.displayOffset * snapshot.columns;
    final totalCells = snapshot.rows * snapshot.columns;
    final start = (selection.start - viewportStart)
        .clamp(0, totalCells)
        .toInt();
    final end = (selection.end - viewportStart).clamp(0, totalCells).toInt();
    final blockTop =
        contentPadding.top +
        (start ~/ snapshot.columns) * metrics.cellSize.height;
    final blockBottom =
        contentPadding.top +
        ((end + snapshot.columns - 1) ~/ snapshot.columns) *
            metrics.cellSize.height;
    return Rect.fromLTRB(
      0,
      blockTop.clamp(0, size.height).toDouble(),
      size.width,
      blockBottom.clamp(0, size.height).toDouble(),
    );
  }

  @override
  void paint(Canvas canvas, Size size) {
    if (selection.isCollapsed) return;
    final focusRect = _focusRect(size);
    if (focusRect.isEmpty) return;

    final leftPadding = contentPadding.left.clamp(0, size.width).toDouble();
    final rightExtensionStart = math.min(
      size.width,
      leftPadding + snapshot.columns * metrics.cellSize.width,
    );
    final contentTop = contentPadding.top.clamp(0, size.height).toDouble();
    final contentBottom = math.min(
      size.height,
      contentTop + snapshot.rows * metrics.cellSize.height,
    );
    final dimOpacity = theme.primary.background.computeLuminance() > 0.5
        ? 0.20
        : 0.30;
    final dim = Paint()..color = Colors.black.withValues(alpha: dimOpacity);
    if (focusRect.top > contentTop) {
      if (leftPadding > 0) {
        canvas.drawRect(
          Rect.fromLTRB(0, contentTop, leftPadding, focusRect.top),
          dim,
        );
      }
      if (rightExtensionStart < size.width) {
        canvas.drawRect(
          Rect.fromLTRB(
            rightExtensionStart,
            contentTop,
            size.width,
            focusRect.top,
          ),
          dim,
        );
      }
    }
    if (focusRect.bottom < contentBottom) {
      if (leftPadding > 0) {
        canvas.drawRect(
          Rect.fromLTRB(0, focusRect.bottom, leftPadding, contentBottom),
          dim,
        );
      }
      if (rightExtensionStart < size.width) {
        canvas.drawRect(
          Rect.fromLTRB(
            rightExtensionStart,
            focusRect.bottom,
            size.width,
            contentBottom,
          ),
          dim,
        );
      }
    }

    final border = Paint()
      ..color = theme.primary.accent.withValues(alpha: 0.55)
      ..style = PaintingStyle.fill;
    const borderWidth = 1.0;
    final viewportStart = -snapshot.displayOffset * snapshot.columns;
    final viewportEnd = viewportStart + snapshot.rows * snapshot.columns;
    if (selection.start >= viewportStart) {
      if (leftPadding > 0) {
        canvas.drawRect(
          Rect.fromLTWH(
            0,
            focusRect.top,
            math.min(size.width, leftPadding + borderWidth),
            borderWidth,
          ),
          border,
        );
      }
      if (rightExtensionStart < size.width) {
        final extensionLeft = math.max(0.0, rightExtensionStart - borderWidth);
        canvas.drawRect(
          Rect.fromLTWH(
            extensionLeft,
            focusRect.top,
            size.width - extensionLeft,
            borderWidth,
          ),
          border,
        );
      }
    }
    if (selection.end <= viewportEnd) {
      if (leftPadding > 0) {
        canvas.drawRect(
          Rect.fromLTWH(
            0,
            math.max(0.0, focusRect.bottom - borderWidth),
            math.min(size.width, leftPadding + borderWidth),
            borderWidth,
          ),
          border,
        );
      }
      if (rightExtensionStart < size.width) {
        final extensionLeft = math.max(0.0, rightExtensionStart - borderWidth);
        canvas.drawRect(
          Rect.fromLTWH(
            extensionLeft,
            math.max(0.0, focusRect.bottom - borderWidth),
            size.width - extensionLeft,
            borderWidth,
          ),
          border,
        );
      }
    }
    canvas
      ..drawRect(
        Rect.fromLTWH(0, focusRect.top, borderWidth, focusRect.height),
        border,
      )
      ..drawRect(
        Rect.fromLTWH(
          math.max(0, size.width - borderWidth),
          focusRect.top,
          borderWidth,
          focusRect.height,
        ),
        border,
      );
  }

  @override
  bool shouldRepaint(covariant TerminalCommandBlockFocusPainter oldDelegate) {
    return oldDelegate.snapshot != snapshot ||
        oldDelegate.metrics.cellSize != metrics.cellSize ||
        oldDelegate.selection != selection ||
        oldDelegate.theme != theme ||
        oldDelegate.contentPadding != contentPadding;
  }
}

@immutable
class _CellStyleKey {
  const _CellStyleKey({
    required this.foreground,
    required this.bold,
    required this.italic,
  });

  factory _CellStyleKey.fromCell(TerminalCell cell, Color foreground) {
    return _CellStyleKey(
      foreground: foreground,
      bold: cell.bold,
      italic: cell.italic,
    );
  }

  factory _CellStyleKey.cursor(Color foreground, TerminalCell cell) {
    return _CellStyleKey(
      foreground: foreground,
      bold: cell.bold,
      italic: cell.italic,
    );
  }

  factory _CellStyleKey.selection(Color foreground, TerminalCell cell) {
    return _CellStyleKey(
      foreground: foreground,
      bold: cell.bold,
      italic: cell.italic,
    );
  }

  final Color foreground;
  final bool bold;
  final bool italic;

  TextStyle resolve(TextStyle base, {int boldWeight = 700, int weight = 400}) {
    return base.copyWith(
      color: foreground,
      fontWeight: bold ? FontWeight(boldWeight) : FontWeight(weight),
      fontStyle: italic ? FontStyle.italic : FontStyle.normal,
      decoration: TextDecoration.none,
      decorationColor: foreground,
    );
  }

  @override
  bool operator ==(Object other) {
    return other is _CellStyleKey &&
        other.foreground == foreground &&
        other.bold == bold &&
        other.italic == italic;
  }

  @override
  int get hashCode => Object.hash(foreground, bold, italic);
}

enum _UnderlineStyle { none, single, double, curly, dotted, dashed }

@immutable
class _CellDecorationKey {
  const _CellDecorationKey({
    required this.color,
    required this.underline,
    required this.strikeout,
  });

  const _CellDecorationKey.none()
    : color = terminalDefaultForeground,
      underline = _UnderlineStyle.none,
      strikeout = false;

  factory _CellDecorationKey.fromCell(TerminalCell cell, Color color) {
    if (cell.hidden) {
      return const _CellDecorationKey.none();
    }

    return _CellDecorationKey(
      color: color,
      underline: _underlineStyleFromCell(cell),
      strikeout: cell.strikeout,
    );
  }

  final Color color;
  final _UnderlineStyle underline;
  final bool strikeout;

  bool get hasDecoration => underline != _UnderlineStyle.none || strikeout;

  @override
  bool operator ==(Object other) {
    return other is _CellDecorationKey &&
        other.color == color &&
        other.underline == underline &&
        other.strikeout == strikeout;
  }

  @override
  int get hashCode => Object.hash(color, underline, strikeout);
}

_UnderlineStyle _underlineStyleFromCell(TerminalCell cell) {
  if (cell.undercurl) {
    return _UnderlineStyle.curly;
  }
  if (cell.dottedUnderline) {
    return _UnderlineStyle.dotted;
  }
  if (cell.dashedUnderline) {
    return _UnderlineStyle.dashed;
  }
  if (cell.doubleUnderlined) {
    return _UnderlineStyle.double;
  }
  if (cell.underlined) {
    return _UnderlineStyle.single;
  }

  return _UnderlineStyle.none;
}

bool _isBlankText(String text) {
  for (var index = 0; index < text.length; index++) {
    if (text.codeUnitAt(index) > 0x20) {
      return false;
    }
  }

  return true;
}

bool _isSingleWidthAscii(String text) {
  return text.length == 1 &&
      text.codeUnitAt(0) >= 0x20 &&
      text.codeUnitAt(0) < 0x7f;
}

class TerminalTextCache {
  Object? _configuration;
  List<_TerminalTextRowCacheEntry?> _rows = const [];

  void prepare({required int rowCount, required Object configuration}) {
    if (_configuration == configuration && _rows.length == rowCount) {
      return;
    }
    clear();
    _configuration = configuration;
    _rows = List<_TerminalTextRowCacheEntry?>.filled(rowCount, null);
  }

  ui.Picture? lookup({
    required int row,
    required List<TerminalCell> cells,
    required int cellOffset,
    required int cellCount,
    required int cursorTextColumn,
  }) {
    if (row < 0 || row >= _rows.length) {
      return null;
    }
    final entry = _rows[row];
    if (entry == null ||
        entry.cursorTextColumn != cursorTextColumn ||
        !_sameTerminalCells(entry.cells, cells, cellOffset, cellCount)) {
      return null;
    }
    return entry.picture;
  }

  void store({
    required int row,
    required List<TerminalCell> cells,
    required int cellOffset,
    required int cellCount,
    required int cursorTextColumn,
    required ui.Picture picture,
  }) {
    if (row < 0 || row >= _rows.length) {
      picture.dispose();
      return;
    }
    _rows[row]?.picture.dispose();
    _rows[row] = _TerminalTextRowCacheEntry(
      cells: List<TerminalCell>.of(
        cells.getRange(cellOffset, cellOffset + cellCount),
        growable: false,
      ),
      cursorTextColumn: cursorTextColumn,
      picture: picture,
    );
  }

  void clear() {
    for (final row in _rows) {
      row?.picture.dispose();
    }
    _rows = const [];
    _configuration = null;
  }

  void dispose() => clear();
}

class _TerminalTextRowCacheEntry {
  const _TerminalTextRowCacheEntry({
    required this.cells,
    required this.cursorTextColumn,
    required this.picture,
  });

  final List<TerminalCell> cells;
  final int cursorTextColumn;
  final ui.Picture picture;
}

bool _sameTerminalCells(
  List<TerminalCell> previous,
  List<TerminalCell> current,
  int currentOffset,
  int count,
) {
  if (previous.length != count || currentOffset < 0 || count < 0) {
    return false;
  }
  if (currentOffset + count > current.length) {
    return false;
  }
  for (var index = 0; index < count; index++) {
    final left = previous[index];
    final right = current[currentOffset + index];
    if (left.text != right.text ||
        left.foreground != right.foreground ||
        left.background != right.background ||
        left.flags != right.flags) {
      return false;
    }
  }
  return true;
}
