part of 'nauterm_workspace.dart';

class _TerminalReconnectOverlay extends StatefulWidget {
  const _TerminalReconnectOverlay({
    super.key,
    required this.controller,
    required this.theme,
    this.onCloseRequested,
  });

  final TerminalController controller;
  final TerminalTheme theme;
  final VoidCallback? onCloseRequested;

  @override
  State<_TerminalReconnectOverlay> createState() =>
      _TerminalReconnectOverlayState();
}

class _TerminalReconnectOverlayState extends State<_TerminalReconnectOverlay> {
  Timer? _countdownTimer;
  TerminalConnectionPhase? _lastPhase;
  int _attempt = 0;
  int _delaySeconds = 3;
  int? _remainingSeconds;
  bool _dismissed = false;

  @override
  void initState() {
    super.initState();
    _lastPhase = widget.controller.connectionStatus.phase;
    widget.controller.addListener(_handleControllerChanged);
    if (_isDisconnected(_lastPhase!)) {
      _startCountdown(notify: false);
    }
  }

  @override
  void didUpdateWidget(covariant _TerminalReconnectOverlay oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (identical(oldWidget.controller, widget.controller)) {
      return;
    }
    oldWidget.controller.removeListener(_handleControllerChanged);
    _countdownTimer?.cancel();
    _attempt = 0;
    _dismissed = false;
    _lastPhase = widget.controller.connectionStatus.phase;
    widget.controller.addListener(_handleControllerChanged);
    if (_isDisconnected(_lastPhase!)) {
      _startCountdown(notify: false);
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_handleControllerChanged);
    _countdownTimer?.cancel();
    super.dispose();
  }

  bool _isDisconnected(TerminalConnectionPhase phase) {
    return phase == TerminalConnectionPhase.failed ||
        phase == TerminalConnectionPhase.exited;
  }

  void _handleControllerChanged() {
    if (!mounted) {
      return;
    }
    final phase = widget.controller.connectionStatus.phase;
    if (phase == _lastPhase) {
      return;
    }
    _lastPhase = phase;
    if (_isDisconnected(phase)) {
      _startCountdown();
      return;
    }
    _countdownTimer?.cancel();
    _countdownTimer = null;
    setState(() => _remainingSeconds = null);
  }

  void _startCountdown({bool notify = true}) {
    _countdownTimer?.cancel();
    _delaySeconds = math.min(3 * (1 << math.min(_attempt, 2)), 15);
    _remainingSeconds = _delaySeconds;
    _countdownTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
      if (!mounted) {
        timer.cancel();
        return;
      }
      final remaining = _remainingSeconds ?? 0;
      if (remaining <= 1) {
        timer.cancel();
        _reconnectNow();
        return;
      }
      setState(() => _remainingSeconds = remaining - 1);
    });
    if (notify) {
      setState(() {});
    }
  }

  void _reconnectNow() {
    _countdownTimer?.cancel();
    _countdownTimer = null;
    _attempt++;
    setState(() {
      _dismissed = false;
      _remainingSeconds = null;
    });
    widget.controller.reconnectSsh();
  }

  @override
  Widget build(BuildContext context) {
    if (_dismissed) {
      return const SizedBox.shrink();
    }

    final theme = widget.theme;
    final background = theme.primary.background;
    final foreground = theme.primary.foreground;
    final accent = theme.primary.accent;
    final danger = theme.normal.red;
    final dark = background.computeLuminance() < 0.35;
    final surface = Color.alphaBlend(
      foreground.withValues(alpha: dark ? 0.09 : 0.07),
      background,
    );
    final muted = Color.lerp(surface, foreground, 0.64)!;
    final border = Color.lerp(surface, foreground, dark ? 0.18 : 0.13)!;
    final phase = widget.controller.connectionStatus.phase;
    final disconnected = _isDisconnected(phase);
    final remaining = _remainingSeconds;
    final detail = disconnected && remaining != null
        ? 'Reconnecting in ${remaining}s...'
        : phase == TerminalConnectionPhase.connecting
        ? 'Reconnecting...'
        : widget.controller.connectionStatus.message ?? 'Connection lost.';

    return Positioned(
      left: 12,
      right: 12,
      bottom: 12,
      child: Align(
        alignment: Alignment.bottomLeft,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 244),
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: surface,
              borderRadius: BorderRadius.circular(9),
              border: Border.all(color: border),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: dark ? 0.34 : 0.14),
                  blurRadius: 18,
                  spreadRadius: -6,
                  offset: const Offset(0, 7),
                ),
              ],
            ),
            child: Padding(
              padding: const EdgeInsets.fromLTRB(10, 9, 7, 9),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                      _ReconnectProgress(
                        remainingSeconds: remaining,
                        delaySeconds: _delaySeconds,
                        color: danger,
                        trackColor: muted.withValues(alpha: 0.24),
                      ),
                      const SizedBox(width: 9),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              disconnected ? 'Connection lost' : 'Reconnecting',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                color: foreground,
                                fontSize: NautermFontSizes.labelMedium,
                                fontWeight: NautermFontWeights.semibold,
                              ),
                            ),
                            const SizedBox(height: 1),
                            Text(
                              detail,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                color: muted,
                                fontSize: NautermFontSizes.labelSmall,
                                fontWeight: NautermFontWeights.regular,
                              ),
                            ),
                          ],
                        ),
                      ),
                      const SizedBox(width: 5),
                      IconButton(
                        tooltip: tr(
                          'workspace.label.dismissReconnectStatus',
                          fallback: 'Dismiss reconnect status',
                        ),
                        visualDensity: VisualDensity.compact,
                        constraints: const BoxConstraints.tightFor(
                          width: 30,
                          height: 30,
                        ),
                        padding: EdgeInsets.zero,
                        iconSize: 17,
                        color: muted,
                        onPressed: () {
                          _countdownTimer?.cancel();
                          _countdownTimer = null;
                          setState(() {
                            _dismissed = true;
                            _remainingSeconds = null;
                          });
                        },
                        icon: const Icon(Icons.close_rounded),
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Align(
                    alignment: Alignment.centerLeft,
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        _ReconnectButton(
                          label: 'Reconnect',
                          foreground: background,
                          background: accent,
                          border: accent,
                          onPressed: _reconnectNow,
                        ),
                        if (widget.onCloseRequested != null) ...[
                          const SizedBox(width: 6),
                          _ReconnectButton(
                            label: 'Close terminal',
                            foreground: danger,
                            background: danger.withValues(alpha: 0.08),
                            border: danger.withValues(alpha: 0.36),
                            onPressed: widget.onCloseRequested!,
                          ),
                        ],
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _ReconnectProgress extends StatelessWidget {
  const _ReconnectProgress({
    required this.remainingSeconds,
    required this.delaySeconds,
    required this.color,
    required this.trackColor,
  });

  final int? remainingSeconds;
  final int delaySeconds;
  final Color color;
  final Color trackColor;

  @override
  Widget build(BuildContext context) {
    final remaining = remainingSeconds;
    return SizedBox.square(
      dimension: 32,
      child: Stack(
        fit: StackFit.expand,
        children: [
          CircularProgressIndicator(
            value: remaining == null
                ? null
                : remaining / math.max(1, delaySeconds),
            strokeWidth: 2.5,
            strokeCap: StrokeCap.round,
            color: color,
            backgroundColor: trackColor,
          ),
          Center(
            child: remaining == null
                ? Icon(Icons.sync_rounded, size: 15, color: color)
                : Text(
                    tr('$remaining'),
                    style: TextStyle(
                      color: color,
                      fontSize: 14,
                      fontWeight: NautermFontWeights.semibold,
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}

class _ReconnectButton extends StatelessWidget {
  const _ReconnectButton({
    required this.label,
    required this.foreground,
    required this.background,
    required this.border,
    required this.onPressed,
  });

  final String label;
  final Color foreground;
  final Color background;
  final Color border;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 26,
      child: TextButton(
        onPressed: onPressed,
        style: TextButton.styleFrom(
          foregroundColor: foreground,
          backgroundColor: background,
          minimumSize: Size.zero,
          padding: const EdgeInsets.symmetric(horizontal: 7),
          tapTargetSize: MaterialTapTargetSize.shrinkWrap,
          visualDensity: VisualDensity.compact,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(6),
            side: BorderSide(color: border),
          ),
          textStyle: TextStyle(
            fontSize: NautermFontSizes.labelMedium,
            fontWeight: NautermFontWeights.semibold,
          ),
        ),
        child: Text(label),
      ),
    );
  }
}
