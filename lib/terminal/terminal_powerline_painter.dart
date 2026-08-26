part of 'terminal_widget.dart';

/// Paints the geometric Powerline sprites built into Ghostty.
///
/// Keeping these shapes independent of font fallback makes them fill the
/// terminal cell and join adjacent background colors without seams.
class _PowerlineGlyphPainter {
  const _PowerlineGlyphPainter({required this.strokeWidth});

  final double strokeWidth;

  bool supports(String text) => switch (text) {
    '\uE0B0' ||
    '\uE0B1' ||
    '\uE0B2' ||
    '\uE0B3' ||
    '\uE0B4' ||
    '\uE0B5' ||
    '\uE0B6' ||
    '\uE0B7' ||
    '\uE0B8' ||
    '\uE0B9' ||
    '\uE0BA' ||
    '\uE0BB' ||
    '\uE0BC' ||
    '\uE0BD' ||
    '\uE0BE' ||
    '\uE0BF' ||
    '\uE0D2' ||
    '\uE0D4' => true,
    _ => false,
  };

  bool paint(Canvas canvas, String text, Color color, Rect cell) {
    if (!supports(text)) return false;

    final fill = Paint()..color = color;
    final stroke = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = strokeWidth
      ..strokeCap = StrokeCap.butt
      ..strokeJoin = StrokeJoin.miter;

    switch (text) {
      case '\uE0B0':
        _paintTriangle(canvas, fill, cell, pointsRight: true);
      case '\uE0B1':
        _paintChevron(canvas, stroke, cell, pointsRight: true);
      case '\uE0B2':
        _paintTriangle(canvas, fill, cell, pointsRight: false);
      case '\uE0B3':
        _paintChevron(canvas, stroke, cell, pointsRight: false);
      case '\uE0B4':
        _paintRound(canvas, fill, cell, pointsRight: true);
      case '\uE0B5':
        _paintRoundOutline(canvas, stroke, cell, pointsRight: true);
      case '\uE0B6':
        _paintRound(canvas, fill, cell, pointsRight: false);
      case '\uE0B7':
        _paintRoundOutline(canvas, stroke, cell, pointsRight: false);
      case '\uE0B8':
        _paintCorner(canvas, fill, cell, corner: 0);
      case '\uE0B9':
      case '\uE0BF':
        canvas.drawLine(cell.topLeft, cell.bottomRight, stroke);
      case '\uE0BA':
        _paintCorner(canvas, fill, cell, corner: 1);
      case '\uE0BB':
      case '\uE0BD':
        canvas.drawLine(cell.topRight, cell.bottomLeft, stroke);
      case '\uE0BC':
        _paintCorner(canvas, fill, cell, corner: 2);
      case '\uE0BE':
        _paintCorner(canvas, fill, cell, corner: 3);
      case '\uE0D2':
        _paintFlame(canvas, fill, cell, pointsRight: true);
      case '\uE0D4':
        _paintFlame(canvas, fill, cell, pointsRight: false);
    }
    return true;
  }

  void _paintTriangle(
    Canvas canvas,
    Paint paint,
    Rect cell, {
    required bool pointsRight,
  }) {
    final flatX = pointsRight ? cell.left : cell.right;
    final tipX = pointsRight ? cell.right : cell.left;
    canvas.drawPath(
      Path()
        ..moveTo(flatX, cell.top)
        ..lineTo(tipX, cell.center.dy)
        ..lineTo(flatX, cell.bottom)
        ..close(),
      paint,
    );
  }

  void _paintChevron(
    Canvas canvas,
    Paint paint,
    Rect cell, {
    required bool pointsRight,
  }) {
    final flatX = pointsRight ? cell.left : cell.right;
    final tipX = pointsRight ? cell.right : cell.left;
    canvas.drawPath(
      Path()
        ..moveTo(flatX, cell.top)
        ..lineTo(tipX, cell.center.dy)
        ..lineTo(flatX, cell.bottom),
      paint,
    );
  }

  void _paintRound(
    Canvas canvas,
    Paint paint,
    Rect cell, {
    required bool pointsRight,
  }) {
    const circleControlPoint = 0.5522847498307936;
    final radius = math.min(cell.width, cell.height / 2);
    final direction = pointsRight ? 1.0 : -1.0;
    final flatX = pointsRight ? cell.left : cell.right;
    final tipX = flatX + direction * radius;
    final controlX = flatX + direction * radius * circleControlPoint;
    canvas.drawPath(
      Path()
        ..moveTo(flatX, cell.top)
        ..cubicTo(
          controlX,
          cell.top,
          tipX,
          cell.top + radius - radius * circleControlPoint,
          tipX,
          cell.top + radius,
        )
        ..lineTo(tipX, cell.bottom - radius)
        ..cubicTo(
          tipX,
          cell.bottom - radius + radius * circleControlPoint,
          controlX,
          cell.bottom,
          flatX,
          cell.bottom,
        )
        ..close(),
      paint,
    );
  }

  void _paintRoundOutline(
    Canvas canvas,
    Paint paint,
    Rect cell, {
    required bool pointsRight,
  }) {
    const circleControlPoint = 0.5522847498307936;
    final radius = math.min(cell.width, cell.height / 2);
    final direction = pointsRight ? 1.0 : -1.0;
    final flatX = pointsRight ? cell.left : cell.right;
    final tipX = flatX + direction * radius;
    final controlX = flatX + direction * radius * circleControlPoint;
    final endOffset = math.min(1.0, cell.width);
    canvas.drawPath(
      Path()
        ..moveTo(flatX, cell.top)
        ..lineTo(flatX + direction * endOffset, cell.top)
        ..cubicTo(
          controlX,
          cell.top,
          tipX,
          cell.top + radius - radius * circleControlPoint,
          tipX,
          cell.top + radius,
        )
        ..lineTo(tipX, cell.bottom - radius)
        ..cubicTo(
          tipX,
          cell.bottom - radius + radius * circleControlPoint,
          controlX,
          cell.bottom,
          flatX + direction * endOffset,
          cell.bottom,
        )
        ..lineTo(flatX, cell.bottom),
      paint,
    );
  }

  void _paintCorner(
    Canvas canvas,
    Paint paint,
    Rect cell, {
    required int corner,
  }) {
    final points = switch (corner) {
      0 => [cell.topLeft, cell.bottomRight, cell.bottomLeft],
      1 => [cell.topRight, cell.bottomRight, cell.bottomLeft],
      2 => [cell.topLeft, cell.topRight, cell.bottomLeft],
      _ => [cell.topLeft, cell.topRight, cell.bottomRight],
    };
    canvas.drawPath(
      Path()
        ..moveTo(points[0].dx, points[0].dy)
        ..lineTo(points[1].dx, points[1].dy)
        ..lineTo(points[2].dx, points[2].dy)
        ..close(),
      paint,
    );
  }

  void _paintFlame(
    Canvas canvas,
    Paint paint,
    Rect cell, {
    required bool pointsRight,
  }) {
    final flatX = pointsRight ? cell.left : cell.right;
    final tipX = pointsRight ? cell.right : cell.left;
    final innerX = cell.center.dx;
    final halfGap = strokeWidth / 2;
    final upperMiddle = cell.center.dy - halfGap;
    final lowerMiddle = cell.center.dy + halfGap;
    canvas
      ..drawPath(
        Path()
          ..moveTo(flatX, cell.top)
          ..lineTo(tipX, cell.top)
          ..lineTo(innerX, upperMiddle)
          ..lineTo(flatX, upperMiddle)
          ..close(),
        paint,
      )
      ..drawPath(
        Path()
          ..moveTo(flatX, cell.bottom)
          ..lineTo(tipX, cell.bottom)
          ..lineTo(innerX, lowerMiddle)
          ..lineTo(flatX, lowerMiddle)
          ..close(),
        paint,
      );
  }
}
