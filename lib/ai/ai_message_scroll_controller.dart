import 'package:flutter/rendering.dart' show ScrollDirection;
import 'package:flutter/widgets.dart';

/// Follows streaming messages until the user scrolls away from the bottom.
class AiMessageScrollController extends ScrollController {
  bool _following = true;
  bool _scheduled = false;
  bool _disposed = false;

  bool handleScrollNotification(ScrollNotification notification) {
    if (notification.depth != 0) return false;
    if (notification is ScrollStartNotification &&
        notification.dragDetails != null) {
      _following = false;
    } else if (notification is UserScrollNotification) {
      if (notification.direction == ScrollDirection.forward) {
        _following = false;
      } else if (notification.metrics.extentAfter <= 2) {
        _following = true;
      }
    } else if (notification is ScrollUpdateNotification &&
        (notification.dragDetails != null ||
            (hasClients &&
                position.userScrollDirection != ScrollDirection.idle))) {
      if ((notification.scrollDelta ?? 0) < 0) {
        _following = false;
      } else if (notification.metrics.extentAfter <= 2) {
        _following = true;
      }
    }
    return false;
  }

  void scrollToLatest({bool resume = false}) {
    if (_disposed) return;
    if (resume) _following = true;
    if (!_following || _scheduled) return;
    _scheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _scheduled = false;
      if (_disposed || !_following || !hasClients) return;
      animateTo(
        position.maxScrollExtent,
        duration: const Duration(milliseconds: 120),
        curve: Curves.easeOutCubic,
      );
    });
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
