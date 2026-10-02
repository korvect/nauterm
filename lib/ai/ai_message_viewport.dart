import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'ai_message_scroll_controller.dart';

class AiMessageViewport extends StatefulWidget {
  const AiMessageViewport({
    super.key,
    required this.controller,
    required this.streaming,
    required this.background,
    required this.foreground,
    required this.border,
    required this.scrollToBottomLabel,
    required this.child,
  });

  final AiMessageScrollController controller;
  final bool streaming;
  final Color background;
  final Color foreground;
  final Color border;
  final String scrollToBottomLabel;
  final Widget child;

  @override
  State<AiMessageViewport> createState() => _AiMessageViewportState();
}

class _AiMessageViewportState extends State<AiMessageViewport> {
  bool _showButton = false;
  bool _refreshPending = false;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_refresh);
    _refresh();
  }

  @override
  void didUpdateWidget(AiMessageViewport oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_refresh);
      widget.controller.addListener(_refresh);
    }
    _refresh();
  }

  void _refresh() {
    if (_refreshPending) return;
    _refreshPending = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _refreshPending = false;
      if (!mounted) return;
      final show =
          widget.controller.hasClients &&
          widget.controller.position.extentAfter > 2;
      if (show != _showButton) setState(() => _showButton = show);
    });
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  @override
  void dispose() {
    widget.controller.removeListener(_refresh);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      NotificationListener<ScrollMetricsNotification>(
        onNotification: (notification) {
          if (notification.depth == 0) _refresh();
          return false;
        },
        child: NotificationListener<ScrollNotification>(
          onNotification: widget.controller.handleScrollNotification,
          child: Stack(
            children: [
              Positioned.fill(child: widget.child),
              if (_showButton)
                Positioned(
                  left: 0,
                  right: 0,
                  bottom: 12,
                  child: Center(
                    child: Tooltip(
                      message: widget.scrollToBottomLabel,
                      child: Material(
                        color: widget.background,
                        elevation: 1,
                        shape: CircleBorder(
                          side: BorderSide(color: widget.border),
                        ),
                        clipBehavior: Clip.antiAlias,
                        child: InkWell(
                          key: const ValueKey('ai-scroll-to-bottom'),
                          customBorder: const CircleBorder(),
                          onTap: () =>
                              widget.controller.scrollToLatest(resume: true),
                          child: SizedBox.square(
                            dimension: 26,
                            child: Center(
                              child: widget.streaming
                                  ? _StreamingDots(color: widget.foreground)
                                  : Icon(
                                      LucideIcons.arrowDown,
                                      size: 13,
                                      color: widget.foreground,
                                    ),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
            ],
          ),
        ),
      );
}

class _StreamingDots extends StatefulWidget {
  const _StreamingDots({required this.color});
  final Color color;

  @override
  State<_StreamingDots> createState() => _StreamingDotsState();
}

class _StreamingDotsState extends State<_StreamingDots>
    with SingleTickerProviderStateMixin {
  late final AnimationController _animation = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1000),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (MediaQuery.disableAnimationsOf(context)) {
      _animation.stop();
      _animation.value = 0;
    } else {
      _animation.repeat();
    }
  }

  @override
  void dispose() {
    _animation.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: _animation,
    builder: (context, child) => Row(
      mainAxisSize: MainAxisSize.min,
      children: List.generate(3, (index) {
        final phase = (_animation.value - index * 0.15) % 1;
        final lift = MediaQuery.disableAnimationsOf(context)
            ? 0.0
            : -2.0 * math.max(0.0, math.sin(phase * math.pi * 2));
        return Transform.translate(
          offset: Offset(0, lift),
          child: Container(
            width: 3,
            height: 3,
            margin: const EdgeInsets.symmetric(horizontal: 1),
            decoration: BoxDecoration(
              color: widget.color,
              shape: BoxShape.circle,
            ),
          ),
        );
      }),
    ),
  );
}
