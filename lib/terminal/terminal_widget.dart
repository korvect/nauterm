import 'dart:async';
import 'dart:io';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';

import '../app/nauterm_localizations.dart';
import '../app/nauterm_log.dart';

import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'terminal_controller.dart';
import 'terminal_driver.dart';
import 'terminal_config.dart';
import 'terminal_key_encoder.dart';
import 'terminal_models.dart';
import 'terminal_open_target.dart';
import 'terminal_selection.dart';
import 'terminal_sensitive_input.dart';
import 'terminal_theme.dart';
import 'terminal_text_width.dart';
import '../ui/nauterm_context_menu.dart';
import '../ui/nauterm_overlay.dart';

part 'terminal_view_toolbar.dart';
part 'terminal_context_menu.dart';
part 'terminal_painter.dart';

enum TerminalSplitDirection { right, down }

typedef TerminalComposerSuggestionResolver = List<String> Function(
  String input,
  int limit,
);
typedef TerminalOpenTargetCallback = FutureOr<void> Function(
  TerminalOpenTarget target,
);

@visibleForTesting
String terminalCursorMoveSequence({
  required TerminalSnapshot snapshot,
  required TerminalCellPosition target,
}) {
  final current = TerminalCellPosition(
    row: snapshot.cursor.row,
    column: snapshot.cursor.column,
  ).clampTo(snapshot);
  final destination = target;
  final prefix = snapshot.keyboardMode.applicationCursor ? '\x1bO' : '\x1b[';
  String repeat(String finalByte, int count) =>
      List<String>.filled(count, '$prefix$finalByte').join();

  final delta =
      destination.toOffset(snapshot.columns) -
      current.toOffset(snapshot.columns);
  return delta < 0 ? repeat('D', -delta) : repeat('C', delta);
}

class TerminalWidgetController {
  _TerminalWidgetState? _state;

  void focus() {
    _state?._requestFocus();
  }

  void copySelection() {
    _state?._copySelection();
  }

  Future<void> pasteClipboard() async {
    await _state?._pasteClipboard();
  }

  void clear() {
    _state?._clearTerminal();
  }

  void reset() {
    _state?._resetTerminal();
  }

  void selectAll() {
    _state?._selectAllText();
  }

  void showSearch() {
    _state?._showSearch();
  }

  void _attach(_TerminalWidgetState state) {
    _state = state;
  }

  void _detach(_TerminalWidgetState state) {
    if (_state == state) {
      _state = null;
    }
  }
}

class _TerminalPointerRegion extends StatelessWidget {
  const _TerminalPointerRegion({
    required this.child,
    this.onPointerDown,
    this.onPointerMove,
    this.onPointerHover,
    this.onPointerUp,
    this.onPointerCancel,
    this.onPointerSignal,
    this.onPointerPanZoomStart,
    this.onPointerPanZoomUpdate,
    this.onPointerPanZoomEnd,
  });

  final Widget child;
  final ValueChanged<PointerDownEvent>? onPointerDown;
  final ValueChanged<PointerMoveEvent>? onPointerMove;
  final ValueChanged<PointerHoverEvent>? onPointerHover;
  final ValueChanged<PointerUpEvent>? onPointerUp;
  final ValueChanged<PointerCancelEvent>? onPointerCancel;
  final ValueChanged<PointerSignalEvent>? onPointerSignal;
  final ValueChanged<PointerPanZoomStartEvent>? onPointerPanZoomStart;
  final ValueChanged<PointerPanZoomUpdateEvent>? onPointerPanZoomUpdate;
  final ValueChanged<PointerPanZoomEndEvent>? onPointerPanZoomEnd;

  @override
  Widget build(BuildContext context) {
    return Listener(
      behavior: HitTestBehavior.opaque,
      onPointerDown: onPointerDown,
      onPointerMove: onPointerMove,
      onPointerHover: onPointerHover,
      onPointerUp: onPointerUp,
      onPointerCancel: onPointerCancel,
      onPointerSignal: onPointerSignal,
      onPointerPanZoomStart: onPointerPanZoomStart,
      onPointerPanZoomUpdate: onPointerPanZoomUpdate,
      onPointerPanZoomEnd: onPointerPanZoomEnd,
      child: child,
    );
  }
}

class _TerminalScrollViewport extends SingleChildRenderObjectWidget {
  const _TerminalScrollViewport({
    required this.position,
    required this.maxScrollExtent,
    required this.targetScrollOffset,
    required this.synchronizeScrollOffset,
    required this.onScrollOffsetCorrected,
    required super.child,
  });

  final ViewportOffset position;
  final double maxScrollExtent;
  final double targetScrollOffset;
  final bool synchronizeScrollOffset;
  final ValueChanged<double> onScrollOffsetCorrected;

  @override
  RenderObject createRenderObject(BuildContext context) {
    return _RenderTerminalScrollViewport(
      position,
      maxScrollExtent,
      targetScrollOffset,
      synchronizeScrollOffset,
      onScrollOffsetCorrected,
    );
  }

  @override
  void updateRenderObject(
    BuildContext context,
    _RenderTerminalScrollViewport renderObject,
  ) {
    renderObject
      ..position = position
      ..maxScrollExtent = maxScrollExtent
      ..targetScrollOffset = targetScrollOffset
      ..synchronizeScrollOffset = synchronizeScrollOffset
      ..onScrollOffsetCorrected = onScrollOffsetCorrected;
  }
}

class _RenderTerminalScrollViewport extends RenderProxyBox {
  _RenderTerminalScrollViewport(
    this._position,
    this._maxScrollExtent,
    this._targetScrollOffset,
    this._synchronizeScrollOffset,
    this._onScrollOffsetCorrected,
  );

  ViewportOffset _position;
  double _maxScrollExtent;
  double _targetScrollOffset;
  bool _synchronizeScrollOffset;
  ValueChanged<double> _onScrollOffsetCorrected;

  set position(ViewportOffset value) {
    if (_position == value) return;
    _position = value;
    markNeedsLayout();
  }

  set maxScrollExtent(double value) {
    if (_maxScrollExtent == value) return;
    _maxScrollExtent = value;
    markNeedsLayout();
  }

  set targetScrollOffset(double value) {
    if (_targetScrollOffset == value) return;
    _targetScrollOffset = value;
    markNeedsLayout();
  }

  set synchronizeScrollOffset(bool value) {
    if (_synchronizeScrollOffset == value) return;
    _synchronizeScrollOffset = value;
    markNeedsLayout();
  }

  set onScrollOffsetCorrected(ValueChanged<double> value) {
    _onScrollOffsetCorrected = value;
  }

  @override
  void performLayout() {
    super.performLayout();
    _position.applyViewportDimension(size.height);
    if (_synchronizeScrollOffset && _position.hasPixels) {
      final target = _targetScrollOffset.clamp(0, _maxScrollExtent).toDouble();
      final correction = target - _position.pixels;
      if (correction != 0) {
        _position.correctBy(correction);
        _onScrollOffsetCorrected(target);
      }
    }
    _position.applyContentDimensions(0, _maxScrollExtent);
  }
}

class _TerminalScrollPhysics extends ScrollPhysics {
  const _TerminalScrollPhysics({super.parent});

  @override
  _TerminalScrollPhysics applyTo(ScrollPhysics? ancestor) {
    return _TerminalScrollPhysics(parent: buildParent(ancestor));
  }

  @override
  double applyBoundaryConditions(ScrollMetrics position, double value) {
    if (value < position.pixels &&
        position.pixels <= position.minScrollExtent) {
      return value - position.pixels;
    }
    if (position.maxScrollExtent <= position.pixels &&
        position.pixels < value) {
      return value - position.pixels;
    }
    if (value < position.minScrollExtent &&
        position.minScrollExtent < position.pixels) {
      return value - position.minScrollExtent;
    }
    if (position.pixels < position.maxScrollExtent &&
        position.maxScrollExtent < value) {
      return value - position.maxScrollExtent;
    }
    return 0;
  }
}

class TerminalWidget extends StatefulWidget {
  const TerminalWidget({
    super.key,
    required this.controller,
    this.config,
    this.theme = defaultTerminalTheme,
    this.padding,
    this.readOnly = false,
    this.actionController,
    this.onContextMenuRequested,
    this.onOpenTarget,
  });

  final TerminalController controller;
  final TerminalConfig? config;
  final TerminalTheme theme;
  final EdgeInsets? padding;
  final bool readOnly;
  final TerminalWidgetController? actionController;
  final ValueChanged<Offset>? onContextMenuRequested;
  final TerminalOpenTargetCallback? onOpenTarget;

  @override
  State<TerminalWidget> createState() => _TerminalWidgetState();
}

@immutable
class TerminalViewTabToolbar {
  const TerminalViewTabToolbar({
    required this.tabs,
    this.sessionTitle,
    this.sessionTitleListenable,
    this.onSftpRequested,
    this.sftpAvailable = false,
  });

  final List<TerminalViewTabToolbarTab> tabs;
  final ValueGetter<String>? sessionTitle;
  final Listenable? sessionTitleListenable;
  final VoidCallback? onSftpRequested;
  final bool sftpAvailable;
}

@immutable
class TerminalViewTabToolbarTab {
  const TerminalViewTabToolbarTab({
    required this.title,
    this.selected = false,
    this.onSelected,
    this.onClose,
  });

  final String title;
  final bool selected;
  final VoidCallback? onSelected;
  final VoidCallback? onClose;
}

class TerminalView extends StatefulWidget {
  const TerminalView({
    super.key,
    required this.controller,
    this.config,
    this.theme = defaultTerminalTheme,
    this.padding,
    this.autofocusTerminal = false,
    this.readOnly = false,
    this.composerHistory = const [],
    this.composerSuggestions = const [],
    this.composerSuggestionResolver,
    this.composerVisible,
    this.onComposerVisibilityChanged,
    this.tabToolbar,
    this.onSplitRequested,
    this.onNewTabRequested,
    this.onSettingsRequested,
    this.onCloseRequested,
    this.onOpenTarget,
  });

  final TerminalController controller;
  final TerminalConfig? config;
  final TerminalTheme theme;
  final EdgeInsets? padding;
  final bool autofocusTerminal;
  final bool readOnly;
  final List<String> composerHistory;
  final List<String> composerSuggestions;
  final TerminalComposerSuggestionResolver? composerSuggestionResolver;
  final bool? composerVisible;
  final ValueChanged<bool>? onComposerVisibilityChanged;
  final TerminalViewTabToolbar? tabToolbar;
  final ValueChanged<TerminalSplitDirection>? onSplitRequested;
  final VoidCallback? onNewTabRequested;
  final VoidCallback? onSettingsRequested;
  final VoidCallback? onCloseRequested;
  final TerminalOpenTargetCallback? onOpenTarget;

  @override
  State<TerminalView> createState() => _TerminalViewState();
}

class _TerminalViewState extends State<TerminalView>
    with SingleTickerProviderStateMixin {
  final _TerminalComposerTextController _composerController =
      _TerminalComposerTextController();
  final FocusNode _composerFocusNode = FocusNode(
    debugLabel: 'terminal composer',
  );
  final LayerLink _composerLayerLink = LayerLink();
  final GlobalKey _composerKey = GlobalKey();
  final TerminalWidgetController _terminalWidgetController =
      TerminalWidgetController();
  final Object _composerOverlayToken = Object();
  final Object _contextMenuOverlayToken = Object();
  late final AnimationController _composerExpansionController;
  NautermTransientOverlayHandle? _composerSuggestionsOverlay;
  NautermTransientOverlayHandle? _contextMenuOverlay;
  NautermOverlayController? _overlayController;
  Offset _contextMenuPosition = Offset.zero;
  String? _composerSuggestion;
  List<String> _composerSuggestionCandidates = const [];
  int? _composerSuggestionIndex;
  String? _suppressedComposerCompletionInput;
  String? _pendingComposerCompletionActivationInput;
  int? _composerHistoryIndex;
  String? _composerHistoryDraft;
  bool _restoringComposerHistory = false;
  late bool _composerVisible;
  bool _composerExpanded = false;
  bool _composerSurfaceExpanded = false;
  late bool _autocompleteEnabled;

  TerminalConfig get _config => widget.config ?? widget.controller.config;
  EdgeInsets get _padding => widget.padding ?? terminalPadding;
  VoidCallback? get _onNewTabRequested =>
      terminalMultiTabEnabled ? widget.onNewTabRequested : null;

  @override
  void initState() {
    super.initState();
    _composerExpansionController = AnimationController(
      duration: const Duration(milliseconds: 180),
      reverseDuration: const Duration(milliseconds: 160),
      vsync: this,
    );
    _composerVisible = widget.composerVisible ?? _config.composer.enabled;
    _autocompleteEnabled = _config.composer.autocompleteEnabled;
    widget.controller.addListener(_handleControllerChanged);
    _composerController.addListener(_handleComposerChanged);
    _composerFocusNode.addListener(_handleComposerFocusChanged);
    terminalPaddingNotifier.addListener(_handlePaddingChanged);
    terminalConfigNotifier.addListener(_handleConfigChanged);
    if (widget.autofocusTerminal) {
      _focusTerminalAfterFrame();
    }
  }

  @override
  void didUpdateWidget(covariant TerminalView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_handleControllerChanged);
      widget.controller.addListener(_handleControllerChanged);
      _scheduleComposerSuggestionsOverlaySync();
    }
    if (widget.autofocusTerminal &&
        (!oldWidget.autofocusTerminal ||
            oldWidget.controller != widget.controller)) {
      _focusTerminalAfterFrame();
    }
    if (widget.composerVisible case final visible?
        when visible != _composerVisible) {
      _composerVisible = visible;
      if (!visible) {
        _composerFocusNode.unfocus();
        _removeComposerSuggestionsOverlay();
      }
    }
    if (oldWidget.config != widget.config ||
        oldWidget.controller != widget.controller) {
      _autocompleteEnabled = _config.composer.autocompleteEnabled;
    }
    if (oldWidget.theme != widget.theme) {
      _contextMenuOverlay?.markNeedsBuild();
    }
    _updateComposerSuggestion();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final nextController = NautermOverlayScope.maybeOf(context);
    if (identical(nextController, _overlayController)) {
      return;
    }
    _removeComposerSuggestionsOverlay();
    _closeContextMenu();
    _overlayController = nextController;
  }

  void _toggleComposer() {
    final visible = !_composerVisible;
    setState(() {
      _composerVisible = visible;
      if (!_composerVisible) {
        _composerFocusNode.unfocus();
        _removeComposerSuggestionsOverlay();
      }
    });
    widget.onComposerVisibilityChanged?.call(visible);
  }

  void _toggleComposerExpanded() {
    final expand = !_composerExpanded;
    setState(() {
      _composerExpanded = expand;
      if (expand) {
        _composerSurfaceExpanded = true;
      }
    });
    if (expand) {
      unawaited(_composerExpansionController.forward());
    } else {
      unawaited(
        _composerExpansionController.reverse().whenComplete(() {
          if (mounted && !_composerExpanded) {
            setState(() => _composerSurfaceExpanded = false);
          }
        }),
      );
    }
    _scheduleComposerSuggestionsOverlaySync();
    _composerFocusNode.requestFocus();
  }

  void _handleComposerFocusChanged() {
    _scheduleComposerSuggestionsOverlaySync();
  }

  void _handlePaddingChanged() {
    if (mounted) setState(() {});
  }

  void _handleConfigChanged() {
    if (!mounted) {
      return;
    }
    final autocompleteEnabled = terminalAutocompleteEnabled;
    if (_autocompleteEnabled == autocompleteEnabled) {
      setState(() {});
      return;
    }
    _autocompleteEnabled = autocompleteEnabled;
    _updateComposerSuggestion();
  }

  @override
  void dispose() {
    _removeComposerSuggestionsOverlay();
    _closeContextMenu();
    terminalPaddingNotifier.removeListener(_handlePaddingChanged);
    terminalConfigNotifier.removeListener(_handleConfigChanged);
    widget.controller.removeListener(_handleControllerChanged);
    _composerFocusNode.removeListener(_handleComposerFocusChanged);
    _composerController.removeListener(_handleComposerChanged);
    _composerController.dispose();
    _composerFocusNode.dispose();
    _composerExpansionController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final textStyle = _config.font.textStyle();
    final metrics = TerminalMetrics.measure(textStyle);
    final padding = _padding;

    return _buildTerminalSurface(textStyle, metrics, padding);
  }

  void _focusTerminalAfterFrame() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      _terminalWidgetController.focus();
    });
  }

  Widget _buildTerminalSurface(
    TextStyle textStyle,
    TerminalMetrics metrics,
    EdgeInsets padding,
  ) {
    final composerConfig = _config.composer;
    final terminalRenderer = KeyedSubtree(
      key: const ValueKey('terminal-renderer-region'),
      child: TerminalWidget(
        controller: widget.controller,
        config: widget.config,
        theme: widget.theme,
        padding: padding,
        readOnly: widget.readOnly,
        actionController: _terminalWidgetController,
        onContextMenuRequested: _showContextMenu,
        onOpenTarget: widget.onOpenTarget,
      ),
    );
    final Widget terminalBody;
    if (!_composerVisible) {
      terminalBody = terminalRenderer;
    } else {
      final singleLineComposerHeight = math.max(
        42.0,
        metrics.cellSize.height + 22,
      );
      final singleLineComposerReservedHeight = singleLineComposerHeight + 8;
      terminalBody = LayoutBuilder(
        builder: (context, constraints) {
          final collapsedWidth = math.min(
            960.0,
            math.max(0.0, constraints.maxWidth - 16),
          );
          final collapsedLeft = (constraints.maxWidth - collapsedWidth) / 2;
          final expandedLeft = math.min(12.0, constraints.maxWidth / 2);
          final expandedTop = math.min(
            44.0,
            math.max(0.0, constraints.maxHeight - 12),
          );
          final expandedWidth = math.max(
            0.0,
            constraints.maxWidth - expandedLeft * 2,
          );
          return Stack(
            children: [
              Positioned.fill(
                child: Column(
                  children: [
                    Expanded(child: terminalRenderer),
                    SizedBox(height: singleLineComposerReservedHeight),
                  ],
                ),
              ),
              ValueListenableBuilder<TextEditingValue>(
                valueListenable: _composerController,
                child: _buildComposer(composerConfig, textStyle),
                builder: (context, value, child) {
                  final lineCount = _composerVisualLineCount(
                    value.text,
                    textStyle,
                    collapsedWidth,
                    composerConfig,
                  );
                  final collapsedHeight = math.max(
                    42.0,
                    metrics.cellSize.height * lineCount + 22,
                  );
                  final collapsedTop = math.max(
                    0.0,
                    constraints.maxHeight - collapsedHeight - 8,
                  );
                  final expandedHeight = math.max(
                    collapsedHeight,
                    constraints.maxHeight - expandedTop - 12,
                  );
                  return AnimatedBuilder(
                    animation: _composerExpansionController,
                    child: child,
                    builder: (context, child) {
                      final progress = Curves.easeInOutCubic.transform(
                        _composerExpansionController.value,
                      );
                      return Positioned(
                        left:
                            collapsedLeft +
                            (expandedLeft - collapsedLeft) * progress,
                        top:
                            collapsedTop +
                            (expandedTop - collapsedTop) * progress,
                        width:
                            collapsedWidth +
                            (expandedWidth - collapsedWidth) * progress,
                        height:
                            collapsedHeight +
                            (expandedHeight - collapsedHeight) * progress,
                        child: child!,
                      );
                    },
                  );
                },
              ),
            ],
          );
        },
      );
    }
    return ColoredBox(
      color: widget.theme.primary.background,
      child: Column(
        children: [
          if (widget.tabToolbar != null)
            _TerminalViewTabToolbar(
              toolbar: widget.tabToolbar!,
              theme: widget.theme,
              composerEnabled: _composerVisible,
              onToggleComposer: _toggleComposer,
              onNewTab: _onNewTabRequested,
              onSplitRight: widget.onSplitRequested == null
                  ? null
                  : () =>
                        widget.onSplitRequested!(TerminalSplitDirection.right),
              onSplitDown: widget.onSplitRequested == null
                  ? null
                  : () => widget.onSplitRequested!(TerminalSplitDirection.down),
            ),
          Expanded(child: terminalBody),
        ],
      ),
    );
  }

  int _composerVisualLineCount(
    String text,
    TextStyle textStyle,
    double composerWidth,
    TerminalComposerConfig config,
  ) {
    if (terminalInputIsSensitive(widget.controller.snapshot)) {
      return 1;
    }
    final minLines = math.max(1, config.minLines);
    final maxLines = math.max(minLines, config.maxLines);
    final painter = TextPainter(
      text: TextSpan(text: text.isEmpty ? ' ' : text, style: textStyle),
      textDirection: Directionality.of(context),
      textScaler: MediaQuery.textScalerOf(context),
      maxLines: maxLines,
    )..layout(maxWidth: math.max(1.0, composerWidth - 116));
    final lineCount = painter.computeLineMetrics().length;
    painter.dispose();
    return lineCount.clamp(minLines, maxLines);
  }

  Widget _buildComposer(
    TerminalComposerConfig composerConfig,
    TextStyle textStyle,
  ) {
    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, _) {
        final sensitive = terminalInputIsSensitive(widget.controller.snapshot);
        return _TerminalComposer(
          controller: _composerController,
          composerKey: _composerKey,
          layerLink: _composerLayerLink,
          focusNode: _composerFocusNode,
          suggestion: sensitive ? null : _composerSuggestion,
          candidates: sensitive ? const [] : _composerSuggestionCandidates,
          selectedCandidateIndex:
              _pendingComposerCompletionActivationInput ==
                  _composerController.text
              ? _composerSuggestionIndex
              : null,
          completionActive:
              _pendingComposerCompletionActivationInput ==
              _composerController.text,
          sensitive: sensitive,
          expanded: _composerSurfaceExpanded,
          controlsExpanded: _composerExpanded,
          expansionAnimation: _composerExpansionController,
          config: composerConfig,
          theme: widget.theme,
          textStyle: textStyle,
          onToggleExpanded: _toggleComposerExpanded,
          onSubmitted: _submitComposer,
          onAcceptSuggestion: _acceptComposerSuggestion,
          onCancelCompletion: _cancelComposerCompletion,
          onHistoryPrevious: _showPreviousComposerHistory,
          onHistoryNext: _showNextComposerHistory,
          onSuggestionHighlighted: _highlightComposerSuggestion,
          onSuggestionSelected: _selectComposerSuggestion,
        );
      },
    );
  }

  void _submitComposer() {
    final sensitive = terminalInputIsSensitive(widget.controller.snapshot);
    final command = _composerController.text;
    if (!sensitive && command.trim().isEmpty) {
      return;
    }
    if (sensitive && command.isEmpty) {
      widget.controller.sendInput('\r', sensitive: true);
    } else {
      widget.controller.sendInput('$command\r', sensitive: sensitive);
    }
    _composerController.clear();
    _composerHistoryIndex = null;
    _composerHistoryDraft = null;
    _composerFocusNode.requestFocus();
  }

  void _handleComposerChanged() {
    if (_suppressedComposerCompletionInput != _composerController.text) {
      _suppressedComposerCompletionInput = null;
    }
    if (_pendingComposerCompletionActivationInput != _composerController.text) {
      _pendingComposerCompletionActivationInput = null;
    }
    if (!_restoringComposerHistory) {
      _composerHistoryIndex = null;
      _composerHistoryDraft = null;
    }
    _updateComposerSuggestion();
  }

  void _handleControllerChanged() {
    _updateComposerSuggestion();
  }

  void _updateComposerSuggestion() {
    final candidates = _findComposerSuggestions(_composerController.text);
    final index = candidates.isEmpty
        ? null
        : _composerSuggestionIndex == null
        ? null
        : math.min(_composerSuggestionIndex!, candidates.length - 1);
    final suggestion = index == null ? null : candidates[index];
    if (suggestion == _composerSuggestion &&
        listEquals(candidates, _composerSuggestionCandidates) &&
        index == _composerSuggestionIndex) {
      return;
    }
    setState(() {
      _composerSuggestion = suggestion;
      _composerSuggestionCandidates = candidates;
      _composerSuggestionIndex = index;
    });
    _scheduleComposerSuggestionsOverlaySync();
  }

  bool get _shouldShowComposerSuggestionsOverlay {
    return _composerVisible &&
        _composerFocusNode.hasFocus &&
        !terminalInputIsSensitive(widget.controller.snapshot) &&
        _composerSuggestionCandidates.isNotEmpty;
  }

  void _scheduleComposerSuggestionsOverlaySync() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      _syncComposerSuggestionsOverlay();
    });
  }

  void _syncComposerSuggestionsOverlay() {
    if (!_shouldShowComposerSuggestionsOverlay) {
      _removeComposerSuggestionsOverlay();
      return;
    }
    if (_composerSuggestionsOverlay == null) {
      _composerSuggestionsOverlay = showNautermTransientOverlay(
        context: context,
        token: _composerOverlayToken,
        builder: _buildComposerSuggestionsOverlay,
        onDismissed: () => _composerSuggestionsOverlay = null,
      );
      return;
    }
    _composerSuggestionsOverlay?.markNeedsBuild();
  }

  void _removeComposerSuggestionsOverlay() {
    final overlay = _composerSuggestionsOverlay;
    if (overlay == null) {
      return;
    }
    overlay.dismiss();
  }

  Widget _buildComposerSuggestionsOverlay(BuildContext context) {
    final renderBox =
        _composerKey.currentContext?.findRenderObject() as RenderBox?;
    final overlayBox =
        Overlay.maybeOf(context)?.context.findRenderObject() as RenderBox?;
    final composerSize = renderBox?.size ?? Size.zero;
    final rowHeight = _TerminalComposerSuggestionListState.rowHeight;
    final verticalPadding =
        _TerminalComposerSuggestionListState.verticalPadding;
    final preferredMenuHeight = math.min(
      176.0,
      _composerSuggestionCandidates.length * rowHeight + verticalPadding * 2,
    );
    final composerOffset = renderBox == null || overlayBox == null
        ? Offset.zero
        : renderBox.localToGlobal(Offset.zero, ancestor: overlayBox);
    final overlayHeight = overlayBox?.size.height ?? double.infinity;
    final spaceAbove = composerOffset.dy;
    final spaceBelow = overlayHeight - composerOffset.dy - composerSize.height;
    final opensAbove =
        spaceAbove >= preferredMenuHeight || spaceAbove >= spaceBelow;
    final availableHeight = overlayBox == null
        ? preferredMenuHeight
        : math.max(0.0, opensAbove ? spaceAbove : spaceBelow);
    final menuHeight = math.min(preferredMenuHeight, availableHeight);

    return Positioned.fill(
      child: CompositedTransformFollower(
        link: _composerLayerLink,
        showWhenUnlinked: false,
        targetAnchor: opensAbove ? Alignment.topLeft : Alignment.bottomLeft,
        followerAnchor: Alignment.topLeft,
        offset: opensAbove ? Offset(0, -menuHeight) : Offset.zero,
        child: Align(
          alignment: Alignment.topLeft,
          child: Material(
            color: Colors.transparent,
            child: SizedBox(
              width: composerSize.width,
              height: menuHeight,
              child: _TerminalComposerSuggestionList(
                candidates: _composerSuggestionCandidates,
                selectedIndex:
                    _pendingComposerCompletionActivationInput ==
                        _composerController.text
                    ? _composerSuggestionIndex
                    : null,
                theme: widget.theme,
                textStyle: _config.font.textStyle(),
                onHighlighted: _highlightComposerSuggestion,
                onSelected: _selectComposerSuggestion,
              ),
            ),
          ),
        ),
      ),
    );
  }

  List<String> _findComposerSuggestions(String input) {
    if (!_autocompleteEnabled ||
        terminalInputIsSensitive(widget.controller.snapshot) ||
        input.isEmpty ||
        input.contains('\n') ||
        _suppressedComposerCompletionInput == input) {
      return const [];
    }
    final normalizedInput = input.toLowerCase();
    final limit = math.max(1, _config.composer.maxSuggestions);
    final seen = <String>{};
    final suggestions = <String>[];
    final resolved =
        widget.composerSuggestionResolver?.call(input, limit) ?? const [];
    for (final command in resolved) {
      final trimmed = command.trim();
      if (trimmed.isEmpty || !seen.add(trimmed)) {
        continue;
      }
      suggestions.add(trimmed);
      if (suggestions.length >= limit) {
        return suggestions;
      }
    }
    for (final command in widget.composerSuggestions) {
      final trimmed = command.trim();
      if (trimmed.isEmpty || !seen.add(trimmed)) {
        continue;
      }
      if (trimmed.length > input.length &&
          trimmed.toLowerCase().startsWith(normalizedInput)) {
        suggestions.add(trimmed);
        if (suggestions.length >= limit) {
          break;
        }
      }
    }
    return suggestions;
  }

  List<String> _composerHistoryCommands() {
    final seen = <String>{};
    return [
      for (final command in widget.composerHistory)
        if (command.trim().isNotEmpty && seen.add(command.trim()))
          command.trim(),
    ];
  }

  bool _showPreviousComposerHistory() {
    if (terminalInputIsSensitive(widget.controller.snapshot)) {
      return false;
    }
    final history = _composerHistoryCommands();
    if (history.isEmpty) {
      return false;
    }

    final nextIndex = _composerHistoryIndex == null
        ? 0
        : math.min(_composerHistoryIndex! + 1, history.length - 1);
    _composerHistoryDraft ??= _composerController.text;
    _setComposerTextFromHistory(history[nextIndex]);
    _composerHistoryIndex = nextIndex;
    return true;
  }

  bool _showNextComposerHistory() {
    if (terminalInputIsSensitive(widget.controller.snapshot) ||
        _composerHistoryIndex == null) {
      return false;
    }
    final history = _composerHistoryCommands();
    if (history.isEmpty) {
      _composerHistoryIndex = null;
      _composerHistoryDraft = null;
      return false;
    }

    if (_composerHistoryIndex! > 0) {
      final nextIndex = _composerHistoryIndex! - 1;
      _setComposerTextFromHistory(history[nextIndex]);
      _composerHistoryIndex = nextIndex;
    } else {
      final draft = _composerHistoryDraft ?? '';
      _composerHistoryIndex = null;
      _composerHistoryDraft = null;
      _setComposerTextFromHistory(draft);
    }
    return true;
  }

  void _setComposerTextFromHistory(String command) {
    _restoringComposerHistory = true;
    _composerController.value = TextEditingValue(
      text: command,
      selection: TextSelection.collapsed(offset: command.length),
    );
    _restoringComposerHistory = false;
    _updateComposerSuggestion();
  }

  void _acceptComposerSuggestion() {
    final input = _composerController.text;
    if (_pendingComposerCompletionActivationInput != input) {
      setState(() {
        _suppressedComposerCompletionInput = null;
        _pendingComposerCompletionActivationInput = input;
        if (_composerSuggestionCandidates.isNotEmpty) {
          _composerSuggestionIndex = 0;
          _composerSuggestion = _composerSuggestionCandidates.first;
        }
      });
      _updateComposerSuggestion();
      _composerSuggestionsOverlay?.markNeedsBuild();
      return;
    }
    setState(() {
      _pendingComposerCompletionActivationInput = null;
    });
    final suggestion =
        _composerSuggestionCandidates.elementAtOrNull(
          _composerSuggestionIndex ?? 0,
        ) ??
        _composerSuggestion;
    if (suggestion == null) {
      return;
    }
    _composerController.value = TextEditingValue(
      text: suggestion,
      selection: TextSelection.collapsed(offset: suggestion.length),
    );
    _composerHistoryIndex = null;
    _composerHistoryDraft = null;
    _pendingComposerCompletionActivationInput = null;
    _composerFocusNode.requestFocus();
  }

  bool _cancelComposerCompletion() {
    final input = _composerController.text;
    final completionActive = _pendingComposerCompletionActivationInput == input;
    if (_composerSuggestionCandidates.isEmpty &&
        _composerSuggestion == null &&
        !completionActive) {
      return false;
    }
    setState(() {
      _suppressedComposerCompletionInput = input;
      _pendingComposerCompletionActivationInput = null;
      _composerSuggestion = null;
      _composerSuggestionCandidates = const [];
      _composerSuggestionIndex = null;
    });
    _scheduleComposerSuggestionsOverlaySync();
    return true;
  }

  void _highlightComposerSuggestion(int index) {
    if (_composerSuggestionCandidates.isEmpty) {
      return;
    }
    final nextIndex = index.clamp(0, _composerSuggestionCandidates.length - 1);
    if (nextIndex == _composerSuggestionIndex) {
      return;
    }
    setState(() {
      _composerSuggestionIndex = nextIndex;
      _composerSuggestion = _composerSuggestionCandidates[nextIndex];
    });
    _composerSuggestionsOverlay?.markNeedsBuild();
  }

  void _selectComposerSuggestion(String suggestion) {
    _composerController.value = TextEditingValue(
      text: suggestion,
      selection: TextSelection.collapsed(offset: suggestion.length),
    );
    _suppressedComposerCompletionInput = null;
    _composerFocusNode.requestFocus();
  }

  void _showContextMenu(Offset position) {
    _contextMenuPosition = position;
    final existingOverlay = _contextMenuOverlay;
    if (existingOverlay != null) {
      existingOverlay.dismiss();
      scheduleMicrotask(() {
        if (mounted && _contextMenuOverlay == null) {
          _showContextMenu(position);
        }
      });
      return;
    }

    _contextMenuOverlay = showNautermTransientOverlay(
      context: context,
      token: _contextMenuOverlayToken,
      dismissExisting: true,
      onDismissed: () => _contextMenuOverlay = null,
      builder: (context) => _TerminalContextMenuOverlay(
        position: _contextMenuPosition,
        newTabEnabled: _onNewTabRequested != null,
        readOnly: widget.readOnly,
        theme: widget.theme,
        onDismissed: _closeContextMenu,
        onAction: _handleContextMenuAction,
      ),
    );
  }

  void _closeContextMenu() {
    final overlay = _contextMenuOverlay;
    if (overlay == null) {
      return;
    }
    overlay.dismiss();
  }

  void _handleContextMenuAction(_TerminalContextAction action) {
    _closeContextMenu();
    if (!mounted) {
      return;
    }

    switch (action) {
      case _TerminalContextAction.copy:
        _terminalWidgetController.copySelection();
      case _TerminalContextAction.paste:
        if (!widget.readOnly) {
          unawaited(_terminalWidgetController.pasteClipboard());
        }
      case _TerminalContextAction.splitRight:
        widget.onSplitRequested?.call(TerminalSplitDirection.right);
      case _TerminalContextAction.splitDown:
        widget.onSplitRequested?.call(TerminalSplitDirection.down);
      case _TerminalContextAction.newTab:
        _onNewTabRequested?.call();
      case _TerminalContextAction.search:
        _terminalWidgetController.showSearch();
      case _TerminalContextAction.clear:
        if (!widget.readOnly) {
          widget.controller.sendInput('\x0c');
          _terminalWidgetController.focus();
        }
      case _TerminalContextAction.selectAll:
        _terminalWidgetController.selectAll();
      case _TerminalContextAction.close:
        widget.onCloseRequested?.call();
      case _TerminalContextAction.settings:
        widget.onSettingsRequested?.call();
    }
  }
}

class _TerminalWidgetState extends State<TerminalWidget> with TextInputClient {
  static const Duration _multiTapTimeout = Duration(milliseconds: 500);
  static const Duration _selectionAutoScrollInterval = Duration(
    milliseconds: 50,
  );
  static const int _maximumSelectionAutoScrollLines = 6;
  static const TextEditingValue _emptyImeValue = TextEditingValue(
    selection: TextSelection.collapsed(offset: 0),
  );

  final FocusNode _focusNode = FocusNode(debugLabel: 'terminal');
  final TextEditingController _searchController = TextEditingController();
  final FocusNode _searchFocusNode = FocusNode(debugLabel: 'terminal search');
  final GlobalKey _editableRegionKey = GlobalKey();
  final GlobalKey _searchOverlayKey = GlobalKey();
  final TerminalTextCache _textCache = TerminalTextCache();
  late final ScrollController _terminalScrollController;
  ScrollPosition? _terminalScrollPosition;
  double _terminalScrollLastPixels = 0;
  double _terminalScrollLineRemainder = 0;
  double _terminalScrollCellHeight = 1;
  bool _terminalScrollActive = false;
  bool _terminalScrollSyncScheduled = false;
  bool _terminalScrollSynchronizing = false;
  bool _trackpadScrollIgnored = false;
  Offset? _trackpadScrollLocalPosition;
  TerminalMetrics? _trackpadScrollMetrics;
  double _lastLayoutWidth = 0;
  double _lastLayoutHeight = 0;
  int _lastCellWidth = 0;
  int _lastCellHeight = 0;
  ({int columns, int rows, int cellWidth, int cellHeight})? _pendingResize;
  bool _resizeScheduled = false;
  int _lastCursorColumn = -1;
  int _lastCursorRow = -1;
  bool _cursorBlinkOn = true;
  Timer? _cursorBlinkTimer;
  TextInputConnection? _textInputConnection;
  TextEditingValue _imeValue = _emptyImeValue;
  bool _imeResetPending = false;
  String? _lastCommittedImeText;
  int _textEntryKeySerial = 0;
  int _lastCommittedImeTextEntrySerial = -1;
  TerminalSelection? _selection;
  TerminalSelection? _commandBlockSelection;
  TerminalOpenTarget? _hoveredOpenTarget;
  TerminalOpenTarget? _pressedOpenTarget;
  TerminalSnapshot? _openTargetCacheSnapshot;
  TerminalCellPosition? _openTargetCachePosition;
  TerminalOpenTarget? _openTargetCacheValue;
  bool _openTargetCacheAllowsLocalPaths = false;
  bool _openTargetCacheValid = false;
  int? _openTargetPointer;
  int? _cursorMovePointer;
  TerminalCellPosition? _cursorMovePressedPosition;
  bool _cursorMoveDragged = false;
  int? _ignoredOptionPointer;
  Offset? _lastHoverLocalPosition;
  TerminalMetrics? _lastHoverMetrics;
  bool _searchVisible = false;
  bool _searchHasResult = false;
  String? _searchError;
  int? _dragAnchorOffset;
  int? _dragPointer;
  int _dragTapCount = 0;
  bool _dragDidSelectText = false;
  bool _suppressCommandBlockForPointer = false;
  TerminalSelection? _commandBlockBeforePointerDown;
  Offset? _dragLocalPosition;
  TerminalMetrics? _dragMetrics;
  Timer? _selectionAutoScrollTimer;
  TerminalCellPosition? _lastTapPosition;
  DateTime? _lastTapTime;
  int _tapCount = 0;
  double _scrollLineRemainder = 0;
  final Map<int, ui.Image> _graphicImageCache = {};
  final Set<int> _graphicImageDecodes = {};
  Set<int> _visibleGraphicGenerations = const {};

  TerminalConfig get _config => widget.config ?? widget.controller.config;
  EdgeInsets get _padding => widget.padding ?? terminalPadding;

  @override
  void initState() {
    super.initState();
    _terminalScrollController = ScrollController(
      keepScrollOffset: false,
      debugLabel: 'terminal scroll input',
      onAttach: _attachTerminalScrollPosition,
      onDetach: _detachTerminalScrollPosition,
    );
    widget.actionController?._attach(this);
    _focusNode.addListener(_handleFocusChanged);
    _searchController.addListener(_handleSearchQueryChanged);
    terminalConfigNotifier.addListener(_handleConfigChanged);
    _configureCursorBlink();
  }

  @override
  void didUpdateWidget(covariant TerminalWidget oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.actionController != widget.actionController) {
      oldWidget.actionController?._detach(this);
      widget.actionController?._attach(this);
    }
    if (oldWidget.controller != widget.controller) {
      _lastLayoutWidth = 0;
      _lastLayoutHeight = 0;
      _lastCellWidth = 0;
      _lastCellHeight = 0;
    }
    if (!oldWidget.readOnly && widget.readOnly) {
      _closeTextInputConnection();
    }
    _configureCursorBlink();
    if (_textInputConnection?.attached ?? false) {
      _textInputConnection!.updateConfig(_textInputConfiguration());
    }
  }

  @override
  void dispose() {
    widget.actionController?._detach(this);
    _closeTextInputConnection();
    _focusNode.removeListener(_handleFocusChanged);
    _searchController.removeListener(_handleSearchQueryChanged);
    terminalConfigNotifier.removeListener(_handleConfigChanged);
    _focusNode.dispose();
    _searchController.dispose();
    _searchFocusNode.dispose();
    _cursorBlinkTimer?.cancel();
    _selectionAutoScrollTimer?.cancel();
    _terminalScrollController.dispose();
    _textCache.dispose();
    for (final image in _graphicImageCache.values) {
      image.dispose();
    }
    super.dispose();
  }

  void _handleConfigChanged() {
    if (!mounted) return;
    if (!terminalPointerConfig.commandClickOpensFilenameOrUrl) {
      _hoveredOpenTarget = null;
    }
    if (!terminalSelectCommandBlockOnClick && _commandBlockSelection != null) {
      _setCommandBlockSelection(null, block: null);
      return;
    }
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final textStyle = _config.font.textStyle();
    final metrics = TerminalMetrics.measure(textStyle);
    final padding = _padding;

    return ColoredBox(
      color: widget.theme.primary.background,
      child: _buildTerminalViewport(textStyle, metrics, padding),
    );
  }

  Widget _buildTerminalViewport(
    TextStyle textStyle,
    TerminalMetrics metrics,
    EdgeInsets padding,
  ) {
    _terminalScrollCellHeight = metrics.cellSize.height;
    final scrollBehavior = ScrollConfiguration.of(context);
    final terminalScrollView = ScrollConfiguration(
      behavior: scrollBehavior.copyWith(scrollbars: false, overscroll: false),
      child: Scrollable(
        controller: _terminalScrollController,
        axisDirection: AxisDirection.down,
        physics: const _TerminalScrollPhysics(),
        viewportBuilder: (context, position) => AnimatedBuilder(
          animation: widget.controller,
          builder: (context, child) {
            final snapshot = widget.controller.snapshot;
            return _TerminalScrollViewport(
              position: position,
              maxScrollExtent: snapshot.historyLines * metrics.cellSize.height,
              targetScrollOffset:
                  (snapshot.historyLines - snapshot.displayOffset) *
                  metrics.cellSize.height,
              synchronizeScrollOffset: !_terminalScrollActive,
              onScrollOffsetCorrected: (pixels) {
                _terminalScrollLastPixels = pixels;
              },
              child: child,
            );
          },
          child: MouseRegion(
            key: const ValueKey('terminal-text-region'),
            cursor: _hoveredOpenTarget == null
                ? SystemMouseCursors.text
                : SystemMouseCursors.click,
            onExit: (_) => _clearOpenTargetHover(),
            child: Focus(
              focusNode: _focusNode,
              autofocus: true,
              onKeyEvent: _handleKeyEvent,
              child: _TerminalPointerRegion(
                onPointerDown: (event) => _handlePointerDown(event, metrics),
                onPointerMove: (event) => _handlePointerMove(event, metrics),
                onPointerHover: (event) => _handlePointerHover(event, metrics),
                onPointerUp: (event) => _handlePointerUp(event, metrics),
                onPointerCancel: _handlePointerCancel,
                onPointerSignal: (event) =>
                    _handlePointerSignal(event, metrics),
                onPointerPanZoomStart: _handleTrackpadPanZoomStart,
                onPointerPanZoomUpdate: (event) =>
                    _handleTrackpadPanZoomUpdate(event, metrics),
                onPointerPanZoomEnd: _handleTrackpadPanZoomEnd,
                child: Stack(
                  key: _editableRegionKey,
                  children: [
                    LayoutBuilder(
                      builder: (context, constraints) {
                        final drawableWidth = math.max(
                          0.0,
                          constraints.maxWidth - padding.horizontal,
                        );
                        final drawableHeight = math.max(
                          0.0,
                          constraints.maxHeight - padding.vertical,
                        );
                        final columns = math.max(
                          2,
                          drawableWidth ~/ metrics.cellSize.width,
                        );
                        final rows = math.max(
                          1,
                          drawableHeight ~/ metrics.cellSize.height,
                        );
                        _scheduleResize(
                          columns,
                          rows,
                          drawableWidth,
                          drawableHeight,
                          metrics,
                        );

                        return Stack(
                          fit: StackFit.expand,
                          children: [
                            Padding(
                              padding: padding,
                              child: AnimatedBuilder(
                                animation: widget.controller,
                                builder: (context, _) {
                                  final snapshot = widget.controller.snapshot;
                                  _synchronizeGraphicImages(snapshot);
                                  final cursor = snapshot.cursor;
                                  _scheduleTextInputGeometryUpdate(
                                    metrics,
                                    padding,
                                  );
                                  if (cursor.column != _lastCursorColumn ||
                                      cursor.row != _lastCursorRow) {
                                    _lastCursorColumn = cursor.column;
                                    _lastCursorRow = cursor.row;
                                    _cursorBlinkOn = true;
                                    _configureCursorBlink();
                                  }
                                  return CustomPaint(
                                    painter: TerminalPainter(
                                      snapshot: snapshot,
                                      metrics: metrics,
                                      textStyle: textStyle,
                                      showCursor:
                                          widget
                                              .controller
                                              .localPrediction
                                              .isEmpty &&
                                          _showCursor(snapshot),
                                      composingText: snapshot.displayOffset == 0
                                          ? _imeComposingText
                                          : null,
                                      predictedText:
                                          widget.controller.localPrediction,
                                      selection: _selection,
                                      commandBlockSelection:
                                          _commandBlockSelection,
                                      paintCommandBlockVerticalBorders: false,
                                      openTargetSelection:
                                          _hoveredOpenTarget?.selection,
                                      focused: _focusNode.hasFocus,
                                      theme: widget.theme,
                                      weight: _config.font.weight,
                                      boldWeight: _config.font.boldWeight,
                                      graphicImages: Map.unmodifiable(
                                        _graphicImageCache,
                                      ),
                                      textCache: _textCache,
                                    ),
                                    size: Size.infinite,
                                  );
                                },
                              ),
                            ),
                            if (_commandBlockSelection != null)
                              Positioned.fill(
                                child: IgnorePointer(
                                  child: AnimatedBuilder(
                                    animation: widget.controller,
                                    builder: (context, _) => CustomPaint(
                                      painter: TerminalCommandBlockFocusPainter(
                                        snapshot: widget.controller.snapshot,
                                        metrics: metrics,
                                        selection: _commandBlockSelection!,
                                        theme: widget.theme,
                                        contentPadding: padding,
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                          ],
                        );
                      },
                    ),
                    Positioned(
                      top: 10,
                      right: 18,
                      child: AnimatedBuilder(
                        animation: widget.controller,
                        builder: (context, _) => _buildMoshNetworkStatus(),
                      ),
                    ),
                    if (_searchVisible) _buildSearchOverlay(),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
    if (!terminalScrollbarEnabled) {
      return terminalScrollView;
    }
    final foreground = widget.theme.primary.foreground;
    final thumbAlpha = widget.theme.type == TerminalThemeType.dark
        ? 0.62
        : 0.52;
    return ScrollbarTheme(
      data: ScrollbarThemeData(
        thickness: const WidgetStatePropertyAll(6),
        radius: const Radius.circular(999),
        thumbColor: WidgetStatePropertyAll(
          foreground.withValues(alpha: thumbAlpha),
        ),
        trackVisibility: WidgetStateProperty.resolveWith(
          (states) =>
              states.contains(WidgetState.hovered) ||
              states.contains(WidgetState.dragged),
        ),
        trackColor: WidgetStatePropertyAll(foreground.withValues(alpha: 0.08)),
        trackBorderColor: const WidgetStatePropertyAll(Colors.transparent),
        crossAxisMargin: 2,
        mainAxisMargin: 3,
        minThumbLength: 28,
        interactive: true,
      ),
      child: Scrollbar(
        key: const ValueKey('terminal-scrollbar'),
        controller: _terminalScrollController,
        interactive: true,
        scrollbarOrientation: ScrollbarOrientation.right,
        child: terminalScrollView,
      ),
    );
  }

  void _synchronizeGraphicImages(TerminalSnapshot snapshot) {
    final visible = snapshot.graphicImages
        .map((image) => image.generation)
        .toSet();
    _visibleGraphicGenerations = visible;
    final removed = _graphicImageCache.keys
        .where((generation) => !visible.contains(generation))
        .toList(growable: false);
    for (final generation in removed) {
      _graphicImageCache.remove(generation)?.dispose();
    }
    for (final image in snapshot.graphicImages) {
      if (_graphicImageCache.containsKey(image.generation) ||
          !_graphicImageDecodes.add(image.generation) ||
          image.rgba.length != image.width * image.height * 4) {
        continue;
      }
      unawaited(_decodeGraphicImage(image));
    }
  }

  Future<void> _decodeGraphicImage(TerminalGraphicImage graphic) async {
    ui.ImmutableBuffer? buffer;
    ui.ImageDescriptor? descriptor;
    ui.Codec? codec;
    try {
      buffer = await ui.ImmutableBuffer.fromUint8List(graphic.rgba);
      descriptor = ui.ImageDescriptor.raw(
        buffer,
        width: graphic.width,
        height: graphic.height,
        pixelFormat: ui.PixelFormat.rgba8888,
      );
      codec = await descriptor.instantiateCodec();
      final frame = await codec.getNextFrame();
      if (!mounted ||
          !_visibleGraphicGenerations.contains(graphic.generation)) {
        frame.image.dispose();
        return;
      }
      _graphicImageCache.remove(graphic.generation)?.dispose();
      _graphicImageCache[graphic.generation] = frame.image;
      setState(() {});
    } catch (error, stackTrace) {
      NautermLog.warning(
        'terminal',
        'Unable to decode terminal graphic image.',
        error: error,
        stackTrace: stackTrace,
      );
    } finally {
      _graphicImageDecodes.remove(graphic.generation);
      codec?.dispose();
      descriptor?.dispose();
      buffer?.dispose();
    }
  }

  Widget _buildMoshNetworkStatus() {
    final state = widget.controller.moshNetworkState;
    if (!widget.controller.isMoshSession || state == MoshNetworkState.stable) {
      return const SizedBox.shrink();
    }
    final restored = state == MoshNetworkState.restored;
    final label = switch (state) {
      MoshNetworkState.switching => 'Switching network…',
      MoshNetworkState.degraded => 'Connection interrupted · input queued',
      MoshNetworkState.restored => 'Connection restored',
      MoshNetworkState.stable => '',
    };
    final foreground = restored
        ? widget.theme.normal.green
        : widget.theme.primary.foreground;
    final background = Color.alphaBlend(
      foreground.withValues(alpha: 0.11),
      widget.theme.primary.background,
    );

    return IgnorePointer(
      child: AnimatedSwitcher(
        duration: const Duration(milliseconds: 160),
        child: Container(
          key: ValueKey(state),
          constraints: const BoxConstraints(maxWidth: 300),
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
          decoration: BoxDecoration(
            color: background,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: foreground.withValues(alpha: 0.24)),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.08),
                blurRadius: 10,
                offset: const Offset(0, 3),
              ),
            ],
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                restored ? LucideIcons.circleCheck : LucideIcons.radioTower,
                size: 14,
                color: foreground,
              ),
              const SizedBox(width: 7),
              Flexible(
                child: Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: foreground,
                    fontSize: 11.5,
                    fontWeight: FontWeight.w500,
                    height: 1.1,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildSearchOverlay() {
    final query = _searchController.text;
    final theme = widget.theme;
    final foreground = theme.primary.foreground;
    final dark = theme.type == TerminalThemeType.dark;
    final background = Color.lerp(
      theme.primary.background,
      foreground,
      dark ? 0.045 : 0.025,
    )!;
    final mutedForeground = foreground.withValues(alpha: 0.58);
    final statusText = query.isEmpty
        ? ''
        : _searchHasResult
        ? 'Found'
        : (_searchError?.isNotEmpty ?? false)
        ? 'Error'
        : 'No results';
    final statusColor = _searchHasResult
        ? theme.primary.accent
        : theme.normal.red;

    return Positioned(
      top: 10,
      left: 10,
      right: 10,
      child: Align(
        alignment: Alignment.topRight,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 356),
          child: Focus(
            onKeyEvent: _handleSearchOverlayKeyEvent,
            child: Material(
              color: Colors.transparent,
              child: KeyedSubtree(
                key: const ValueKey('terminal-search-overlay'),
                child: Container(
                  key: _searchOverlayKey,
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 7,
                  ),
                  decoration: BoxDecoration(
                    color: background,
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(
                      color: foreground.withValues(alpha: dark ? 0.18 : 0.14),
                    ),
                    boxShadow: [
                      BoxShadow(
                        color: Colors.black.withValues(
                          alpha: dark ? 0.28 : 0.14,
                        ),
                        blurRadius: 18,
                        offset: const Offset(0, 8),
                      ),
                    ],
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                      Icon(
                        Icons.search_rounded,
                        size: 18,
                        color: mutedForeground,
                      ),
                      const SizedBox(width: 7),
                      Expanded(
                        child: SizedBox(
                          height: 28,
                          child: Stack(
                            alignment: Alignment.centerLeft,
                            children: [
                              Positioned.fill(
                                child: IgnorePointer(
                                  child: Opacity(
                                    opacity: query.isEmpty ? 1 : 0,
                                    child: Align(
                                      alignment: Alignment.centerLeft,
                                      child: Text(
                                        tr(
                                          'terminal.label.searchTerminal',
                                          fallback: 'Search terminal',
                                        ),
                                        key: const ValueKey(
                                          'terminal-search-placeholder',
                                        ),
                                        style: TextStyle(
                                          color: foreground.withValues(
                                            alpha: 0.48,
                                          ),
                                          fontSize: 13,
                                          height: 1,
                                          leadingDistribution:
                                              TextLeadingDistribution.even,
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                              Align(
                                alignment: Alignment.centerLeft,
                                child: Semantics(
                                  label: tr(
                                    'terminal.label.searchTerminal',
                                    fallback: 'Search terminal',
                                  ),
                                  textField: true,
                                  child: SizedBox(
                                    height: 13,
                                    child: EditableText(
                                      key: const ValueKey(
                                        'terminal-search-field',
                                      ),
                                      controller: _searchController,
                                      focusNode: _searchFocusNode,
                                      maxLines: 1,
                                      textInputAction: TextInputAction.search,
                                      onSubmitted: (_) => _searchNext(),
                                      cursorColor: theme.primary.accent,
                                      backgroundCursorColor: foreground
                                          .withValues(alpha: 0.28),
                                      selectionColor: theme.primary.accent
                                          .withValues(alpha: 0.28),
                                      cursorHeight: 13,
                                      style: TextStyle(
                                        color: foreground,
                                        fontSize: 13,
                                        height: 1,
                                        leadingDistribution:
                                            TextLeadingDistribution.even,
                                      ),
                                      strutStyle: const StrutStyle(
                                        fontSize: 13,
                                        height: 1,
                                        forceStrutHeight: true,
                                        leadingDistribution:
                                            TextLeadingDistribution.even,
                                      ),
                                      scrollPadding: EdgeInsets.zero,
                                    ),
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                      if (statusText.isNotEmpty) ...[
                        const SizedBox(width: 8),
                        ConstrainedBox(
                          constraints: const BoxConstraints(maxWidth: 74),
                          child: Text(
                            statusText,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: statusColor,
                              fontSize: 11,
                              height: 1,
                              leadingDistribution: TextLeadingDistribution.even,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ),
                      ],
                      const SizedBox(width: 4),
                      _TerminalSearchButton(
                        icon: Icons.keyboard_arrow_up_rounded,
                        tooltip: tr(
                          'common.action.previous',
                          fallback: 'Previous',
                        ),
                        theme: theme,
                        background: background,
                        onPressed: _searchHasResult ? _searchPrevious : null,
                      ),
                      _TerminalSearchButton(
                        icon: Icons.keyboard_arrow_down_rounded,
                        tooltip: tr('common.action.next', fallback: 'Next'),
                        theme: theme,
                        background: background,
                        onPressed: _searchHasResult ? _searchNext : null,
                      ),
                      _TerminalSearchButton(
                        icon: Icons.close_rounded,
                        tooltip: tr('common.action.close', fallback: 'Close'),
                        theme: theme,
                        background: background,
                        onPressed: _hideSearch,
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  KeyEventResult _handleKeyEvent(FocusNode node, KeyEvent event) {
    _refreshOpenTargetHover();
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }

    if (_handleSearchShortcut(event)) {
      return KeyEventResult.handled;
    }
    if (_searchFocusNode.hasFocus) {
      return KeyEventResult.ignored;
    }

    if (event is KeyDownEvent &&
        event.logicalKey == LogicalKeyboardKey.escape &&
        _effectiveSelection != null) {
      _clearSelection();
      return KeyEventResult.handled;
    }

    if (_isWorkspaceShortcut(event)) {
      return KeyEventResult.ignored;
    }

    final modifiers = TerminalKeyboardModifiers.fromHardwareKeyboard(
      HardwareKeyboard.instance,
    );
    if (_shouldDeferKeyToImeComposition(event, modifiers)) {
      return KeyEventResult.ignored;
    }
    if (_handleScrollKey(event, modifiers)) {
      return KeyEventResult.handled;
    }

    final input = TerminalKeyEncoder(
      config: _config.keyboard,
      mode: widget.controller.snapshot.keyboardMode,
    ).encodeEvent(event, modifiers);
    if (widget.readOnly) {
      final action = input.action;
      if (event is KeyDownEvent &&
          (action == TerminalKeyboardAction.copy ||
              action == TerminalKeyboardAction.selectAll)) {
        _handleKeyboardAction(action!);
      }
      return input.isIgnored ? KeyEventResult.ignored : KeyEventResult.handled;
    }
    if (input.isIgnored) {
      return KeyEventResult.ignored;
    }

    final action = input.action;
    if (action != null) {
      if (event is KeyDownEvent) {
        _handleKeyboardAction(action);
      }
      return KeyEventResult.handled;
    }

    final sequence = input.sequence;
    if (sequence == null || sequence.isEmpty) {
      return KeyEventResult.ignored;
    }
    if (_shouldDeferTextEntryToIme(event, modifiers)) {
      return KeyEventResult.ignored;
    }

    _clearSelection();
    widget.controller.scrollToBottom();
    widget.controller.sendInput(sequence);
    return KeyEventResult.handled;
  }

  bool _handleSearchShortcut(KeyEvent event) {
    final keyboard = HardwareKeyboard.instance;
    final isMacOS = defaultTargetPlatform == TargetPlatform.macOS;
    final platformModifierPressed = isMacOS
        ? keyboard.isMetaPressed && !keyboard.isControlPressed
        : keyboard.isControlPressed &&
              keyboard.isShiftPressed &&
              !keyboard.isMetaPressed;
    if (!platformModifierPressed) {
      return false;
    }
    final matches = isMacOS
        ? terminalShortcutConfig.matchesSearch(
            event.logicalKey,
            shift: keyboard.isShiftPressed,
            alt: keyboard.isAltPressed,
          )
        : terminalShortcutConfig.matchesSearchKey(
            event.logicalKey,
            alt: keyboard.isAltPressed,
          );
    if (!matches) return false;

    if (event is KeyDownEvent) {
      _showSearch();
    }
    return true;
  }

  KeyEventResult _handleSearchOverlayKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      _hideSearch();
      return KeyEventResult.handled;
    }
    if (event.logicalKey == LogicalKeyboardKey.enter ||
        event.logicalKey == LogicalKeyboardKey.numpadEnter) {
      if (HardwareKeyboard.instance.isShiftPressed) {
        _searchPrevious();
      } else {
        _searchNext();
      }
      return KeyEventResult.handled;
    }

    return KeyEventResult.ignored;
  }

  void _showSearch() {
    if (!_searchVisible) {
      setState(() {
        _searchVisible = true;
      });
    }

    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      _searchFocusNode.requestFocus();
      _searchController.selection = TextSelection(
        baseOffset: 0,
        extentOffset: _searchController.text.length,
      );
    });

    if (_searchController.text.isNotEmpty) {
      _runSearch(
        direction: TerminalSearchDirection.next,
        fromCurrentSelection: false,
      );
    }
  }

  void _hideSearch() {
    if (!_searchVisible) {
      return;
    }

    setState(() {
      _searchVisible = false;
      _searchHasResult = false;
      _searchError = null;
      _selection = null;
    });
    _requestFocus();
  }

  void _handleSearchQueryChanged() {
    if (!_searchVisible) {
      return;
    }
    _runSearch(
      direction: TerminalSearchDirection.next,
      fromCurrentSelection: false,
    );
  }

  void _searchNext() {
    _runSearch(
      direction: TerminalSearchDirection.next,
      fromCurrentSelection: true,
    );
  }

  void _searchPrevious() {
    _runSearch(
      direction: TerminalSearchDirection.previous,
      fromCurrentSelection: true,
    );
  }

  void _runSearch({
    required TerminalSearchDirection direction,
    required bool fromCurrentSelection,
  }) {
    final query = _searchController.text;
    if (query.isEmpty) {
      setState(() {
        _searchHasResult = false;
        _searchError = null;
        _selection = null;
      });
      return;
    }

    final result = widget.controller.search(
      query,
      direction: direction,
      origin: _searchOrigin(
        direction: direction,
        fromCurrentSelection: fromCurrentSelection,
      ),
    );
    if (!mounted) {
      return;
    }

    setState(() {
      _searchHasResult = result.found;
      _searchError = result.error;
      _selection = result.selection;
    });
  }

  TerminalCellPosition _searchOrigin({
    required TerminalSearchDirection direction,
    required bool fromCurrentSelection,
  }) {
    final snapshot = widget.controller.snapshot;
    final totalCells = snapshot.columns * snapshot.rows;
    if (totalCells <= 0) {
      return const TerminalCellPosition(row: 0, column: 0);
    }

    final selection = fromCurrentSelection ? _selection : null;
    final viewportStart = -snapshot.displayOffset * snapshot.columns;
    final int offset;
    if (selection == null || selection.isCollapsed) {
      offset = direction == TerminalSearchDirection.next ? 0 : totalCells - 1;
    } else if (direction == TerminalSearchDirection.next) {
      offset = (selection.end - viewportStart).clamp(0, totalCells - 1).toInt();
    } else {
      offset = (selection.start - viewportStart - 1)
          .clamp(0, totalCells - 1)
          .toInt();
    }

    return TerminalCellPosition(
      row: offset ~/ snapshot.columns,
      column: offset % snapshot.columns,
    );
  }

  bool _handleScrollKey(KeyEvent event, TerminalKeyboardModifiers modifiers) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return false;
    }
    if (modifiers.alt || modifiers.control || modifiers.meta) {
      return false;
    }
    final snapshot = widget.controller.snapshot;
    if (!_config.keyboard.navigationKeysScrollOutsideInteractiveApps ||
        snapshot.alternateScreen) {
      return false;
    }
    if (event.logicalKey == LogicalKeyboardKey.pageUp) {
      widget.controller.scrollPageUp();
      return true;
    }
    if (event.logicalKey == LogicalKeyboardKey.pageDown) {
      widget.controller.scrollPageDown();
      return true;
    }
    if (event.logicalKey == LogicalKeyboardKey.home) {
      widget.controller.scrollLines(
        snapshot.historyLines - snapshot.displayOffset,
      );
      return true;
    }
    if (event.logicalKey == LogicalKeyboardKey.end) {
      widget.controller.scrollToBottom();
      return true;
    }
    return false;
  }

  bool _shouldDeferTextEntryToIme(
    KeyEvent event,
    TerminalKeyboardModifiers modifiers,
  ) {
    if (!(_textInputConnection?.attached ?? false)) {
      return false;
    }
    if (modifiers.control || modifiers.meta) {
      return false;
    }
    if (modifiers.alt && _config.keyboard.useOptionAsMetaKey) {
      return false;
    }
    final character = event.character;
    if (character == null || character.isEmpty) {
      return false;
    }

    final deferToIme = !_isNonTextEntryKey(event.logicalKey);
    if (deferToIme) {
      _textEntryKeySerial += 1;
    }
    return deferToIme;
  }

  bool _shouldDeferKeyToImeComposition(
    KeyEvent event,
    TerminalKeyboardModifiers modifiers,
  ) {
    if (!_hasActiveImeComposition) {
      return false;
    }
    if (modifiers.control || modifiers.meta) {
      return false;
    }
    if (modifiers.alt && _config.keyboard.useOptionAsMetaKey) {
      return false;
    }

    return switch (event.logicalKey) {
      LogicalKeyboardKey.backspace ||
      LogicalKeyboardKey.delete ||
      LogicalKeyboardKey.arrowLeft ||
      LogicalKeyboardKey.arrowRight ||
      LogicalKeyboardKey.arrowUp ||
      LogicalKeyboardKey.arrowDown ||
      LogicalKeyboardKey.home ||
      LogicalKeyboardKey.end ||
      LogicalKeyboardKey.enter ||
      LogicalKeyboardKey.numpadEnter ||
      LogicalKeyboardKey.escape ||
      LogicalKeyboardKey.tab ||
      LogicalKeyboardKey.pageUp ||
      LogicalKeyboardKey.pageDown => true,
      _ => false,
    };
  }

  bool get _hasActiveImeComposition => _hasImeComposition(_imeValue);

  bool _hasImeComposition(TextEditingValue value) {
    return value.composing.isValid && !value.composing.isCollapsed;
  }

  String? get _imeComposingText {
    if (!_hasActiveImeComposition) {
      return null;
    }

    final start = _imeValue.composing.start.clamp(0, _imeValue.text.length);
    final end = _imeValue.composing.end.clamp(0, _imeValue.text.length);
    if (start >= end) {
      return null;
    }

    return _imeValue.text.substring(start, end);
  }

  bool _isNonTextEntryKey(LogicalKeyboardKey key) {
    return switch (key) {
      LogicalKeyboardKey.enter ||
      LogicalKeyboardKey.numpadEnter ||
      LogicalKeyboardKey.backspace ||
      LogicalKeyboardKey.tab ||
      LogicalKeyboardKey.escape ||
      LogicalKeyboardKey.arrowUp ||
      LogicalKeyboardKey.arrowDown ||
      LogicalKeyboardKey.arrowRight ||
      LogicalKeyboardKey.arrowLeft ||
      LogicalKeyboardKey.home ||
      LogicalKeyboardKey.end ||
      LogicalKeyboardKey.insert ||
      LogicalKeyboardKey.delete ||
      LogicalKeyboardKey.pageUp ||
      LogicalKeyboardKey.pageDown ||
      LogicalKeyboardKey.f1 ||
      LogicalKeyboardKey.f2 ||
      LogicalKeyboardKey.f3 ||
      LogicalKeyboardKey.f4 ||
      LogicalKeyboardKey.f5 ||
      LogicalKeyboardKey.f6 ||
      LogicalKeyboardKey.f7 ||
      LogicalKeyboardKey.f8 ||
      LogicalKeyboardKey.f9 ||
      LogicalKeyboardKey.f10 ||
      LogicalKeyboardKey.f11 ||
      LogicalKeyboardKey.f12 => true,
      _ => false,
    };
  }

  void _scheduleResize(
    int columns,
    int rows,
    double width,
    double height,
    TerminalMetrics metrics,
  ) {
    final cellWidth = metrics.cellSize.width.round().clamp(1, 0xffffffff);
    final cellHeight = metrics.cellSize.height.round().clamp(1, 0xffffffff);
    if (width == _lastLayoutWidth &&
        height == _lastLayoutHeight &&
        cellWidth == _lastCellWidth &&
        cellHeight == _lastCellHeight) {
      return;
    }
    _lastLayoutWidth = width;
    _lastLayoutHeight = height;
    _lastCellWidth = cellWidth;
    _lastCellHeight = cellHeight;
    _pendingResize = (
      columns: columns,
      rows: rows,
      cellWidth: cellWidth,
      cellHeight: cellHeight,
    );
    if (_resizeScheduled) {
      return;
    }
    _resizeScheduled = true;

    WidgetsBinding.instance.addPostFrameCallback((_) {
      _resizeScheduled = false;
      final resize = _pendingResize;
      _pendingResize = null;
      if (!mounted || resize == null) {
        return;
      }
      _clearSelection();
      widget.controller.resize(
        resize.columns,
        resize.rows,
        cellWidth: resize.cellWidth,
        cellHeight: resize.cellHeight,
      );
    });
  }

  void _handleFocusChanged() {
    if (_focusNode.hasFocus && !widget.readOnly) {
      _openTextInputConnection();
    } else {
      _closeTextInputConnection();
    }

    final mode = widget.controller.snapshot.keyboardMode;
    if (mode.focusEvents && !widget.readOnly) {
      widget.controller.sendInput(_focusNode.hasFocus ? '\x1b[I' : '\x1b[O');
    }
    if (mounted) {
      _cursorBlinkOn = true;
      setState(() {});
    }
  }

  void _openTextInputConnection() {
    if (widget.readOnly) return;
    if (_textInputConnection?.attached ?? false) {
      _textInputConnection!.show();
      return;
    }

    _imeValue = _emptyImeValue;
    _imeResetPending = false;
    _lastCommittedImeText = null;
    _lastCommittedImeTextEntrySerial = -1;
    _textInputConnection = TextInput.attach(this, _textInputConfiguration());
    _textInputConnection!
      ..setEditingState(_imeValue)
      ..show();
  }

  void _closeTextInputConnection() {
    final connection = _textInputConnection;
    if (connection != null && connection.attached) {
      connection.close();
    }
    _textInputConnection = null;
    _imeValue = _emptyImeValue;
    _imeResetPending = false;
    _lastCommittedImeText = null;
    _lastCommittedImeTextEntrySerial = -1;
  }

  TextInputConfiguration _textInputConfiguration() {
    return TextInputConfiguration(
      viewId: View.of(context).viewId,
      inputType: TextInputType.text,
      inputAction: TextInputAction.none,
      autocorrect: false,
      enableSuggestions: false,
      smartDashesType: SmartDashesType.disabled,
      smartQuotesType: SmartQuotesType.disabled,
    );
  }

  void _scheduleTextInputGeometryUpdate(
    TerminalMetrics metrics,
    EdgeInsets padding,
  ) {
    if (!(_textInputConnection?.attached ?? false)) {
      return;
    }

    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      final connection = _textInputConnection;
      final renderObject = _editableRegionKey.currentContext
          ?.findRenderObject();
      if (connection == null ||
          !connection.attached ||
          renderObject is! RenderBox ||
          !renderObject.hasSize) {
        return;
      }

      final cursor = widget.controller.snapshot.cursor;
      final caretRect = Rect.fromLTWH(
        padding.left + cursor.column * metrics.cellSize.width,
        padding.top + cursor.row * metrics.cellSize.height,
        math.max(1.0, metrics.cellSize.width),
        metrics.cellSize.height,
      );

      connection
        ..setEditableSizeAndTransform(
          renderObject.size,
          renderObject.getTransformTo(null),
        )
        ..setCaretRect(caretRect)
        ..setComposingRect(caretRect);
    });
  }

  void _configureCursorBlink() {
    _cursorBlinkTimer?.cancel();
    _cursorBlinkTimer = null;
    _cursorBlinkOn = true;

    if (!_config.cursor.blink) {
      return;
    }

    _cursorBlinkTimer = Timer.periodic(_config.cursor.blinkInterval, (_) {
      if (mounted) {
        setState(() {
          _cursorBlinkOn = !_cursorBlinkOn;
        });
      }
    });
  }

  bool _showCursor(TerminalSnapshot snapshot) {
    if (snapshot.displayOffset != 0) {
      return false;
    }
    final cursor = snapshot.cursor;
    if (!_config.cursor.visible) {
      return false;
    }
    if (!cursor.visible) {
      return false;
    }
    if (!cursor.blinking || !_config.cursor.blink || !_focusNode.hasFocus) {
      return true;
    }

    return _cursorBlinkOn;
  }

  @override
  TextEditingValue? get currentTextEditingValue => _imeValue;

  @override
  AutofillScope? get currentAutofillScope => null;

  @override
  void updateEditingValue(TextEditingValue value) {
    if (_shouldIgnorePendingImeReset(value)) {
      return;
    }

    _imeResetPending = false;
    _lastCommittedImeText = null;
    _lastCommittedImeTextEntrySerial = -1;
    _imeValue = value;
    if (_hasImeComposition(value)) {
      if (mounted) {
        setState(() {});
      }
      return;
    }

    final text = value.text;
    if (text.isNotEmpty) {
      _lastCommittedImeText = text;
      _lastCommittedImeTextEntrySerial = _textEntryKeySerial;
      _commitTextInput(text);
    }
    _resetTextInputEditingState(waitForPlatformReset: text.isNotEmpty);
  }

  @override
  void performAction(TextInputAction action) {
    if (action != TextInputAction.none) {
      _commitTextInput('\r');
    }
    _resetTextInputEditingState();
  }

  @override
  void connectionClosed() {
    _textInputConnection = null;
    _imeValue = _emptyImeValue;
    _imeResetPending = false;
    _lastCommittedImeText = null;
    _lastCommittedImeTextEntrySerial = -1;
  }

  @override
  void performPrivateCommand(String action, Map<String, dynamic> data) {}

  @override
  void updateFloatingCursor(RawFloatingCursorPoint point) {}

  @override
  void showAutocorrectionPromptRect(int start, int end) {}

  void _commitTextInput(String text) {
    if (widget.readOnly) return;
    final input = text.replaceAll('\r\n', '\n').replaceAll('\n', '\r');
    if (input.isEmpty) {
      return;
    }

    _clearSelection();
    widget.controller.scrollToBottom();
    widget.controller.sendInput(input);
  }

  bool _shouldIgnorePendingImeReset(TextEditingValue value) {
    if (!_imeResetPending) {
      return false;
    }

    if (value.text.isEmpty) {
      _imeResetPending = false;
      _lastCommittedImeText = null;
      _lastCommittedImeTextEntrySerial = -1;
      _imeValue = _emptyImeValue;
      if (mounted) {
        setState(() {});
      }
      return true;
    }

    if (!_hasImeComposition(value) &&
        value.text == _lastCommittedImeText &&
        _textEntryKeySerial == _lastCommittedImeTextEntrySerial) {
      _resetTextInputEditingState(waitForPlatformReset: true);
      return true;
    }

    return false;
  }

  void _resetTextInputEditingState({bool waitForPlatformReset = false}) {
    _imeValue = _emptyImeValue;
    _imeResetPending = waitForPlatformReset;
    if (!waitForPlatformReset) {
      _lastCommittedImeText = null;
      _lastCommittedImeTextEntrySerial = -1;
    }
    final connection = _textInputConnection;
    if (connection != null && connection.attached) {
      connection.setEditingState(_imeValue);
    }
    if (mounted) {
      setState(() {});
    }
  }

  void _handlePointerDown(PointerDownEvent event, TerminalMetrics metrics) {
    _stopSelectionAutoScroll();
    if (_isPointerInsideSearchOverlay(event.position)) {
      return;
    }
    final terminalHadFocus = _focusNode.hasFocus;
    _focusNode.requestFocus();
    final position = _cellPositionForOffset(event.localPosition, metrics);
    if ((event.buttons & kPrimaryMouseButton) != 0 &&
        _isOpenTargetModifierPressed()) {
      final target = _openTargetAt(position);
      if (target != null) {
        _pressedOpenTarget = target;
        _openTargetPointer = event.pointer;
        return;
      }
    }
    if ((event.buttons & kPrimaryMouseButton) != 0 &&
        _isOptionCursorMoveModifierPressed()) {
      if (_canMoveCursorWithPointer() && _effectiveSelection == null) {
        _cursorMovePointer = event.pointer;
        _cursorMovePressedPosition = position;
        _cursorMoveDragged = false;
      } else {
        _ignoredOptionPointer = event.pointer;
      }
      return;
    }

    if (_sendMouseReport(
      position: position,
      button: _mouseButtonCode(event.buttons),
    )) {
      _clearSelection();
      return;
    }

    if ((event.buttons & kSecondaryMouseButton) != 0) {
      widget.onContextMenuRequested?.call(event.position);
      return;
    }

    if ((event.buttons & kPrimaryMouseButton) == 0) {
      return;
    }

    final snapshot = widget.controller.snapshot;
    _dragAnchorOffset = terminalCellOffset(snapshot, position);
    _dragPointer = event.pointer;
    _dragLocalPosition = event.localPosition;
    _dragMetrics = metrics;
    final tapCount = _registerTap(position);
    _dragTapCount = tapCount;
    _dragDidSelectText = false;
    _suppressCommandBlockForPointer = !terminalHadFocus;
    _commandBlockBeforePointerDown = _commandBlockSelection;

    switch (tapCount) {
      case 2:
        _setSelection(terminalWordSelectionAt(snapshot, position));
      case 3:
        _setSelection(
          TerminalSelection.line(row: position.row, snapshot: snapshot),
        );
      default:
        if (!_suppressCommandBlockForPointer) {
          _clearSelection();
        }
    }
  }

  void _handlePointerMove(PointerMoveEvent event, TerminalMetrics metrics) {
    if (event.pointer == _cursorMovePointer) {
      if ((event.buttons & kPrimaryMouseButton) != 0) {
        _cursorMoveDragged =
            _cursorMoveDragged ||
            _cellPositionForOffset(event.localPosition, metrics) !=
                _cursorMovePressedPosition;
      }
      return;
    }
    if (event.pointer == _openTargetPointer ||
        event.pointer == _ignoredOptionPointer) {
      return;
    }
    if (_isPointerInsideSearchOverlay(event.position)) {
      _stopSelectionAutoScroll();
      return;
    }
    if (event.pointer != _dragPointer) {
      return;
    }

    final position = _cellPositionForOffset(event.localPosition, metrics);
    if (_sendMouseMotionReport(event, position)) {
      return;
    }

    if ((event.buttons & kPrimaryMouseButton) == 0) {
      return;
    }

    final anchor = _dragAnchorOffset;
    if (anchor == null) {
      return;
    }

    _dragLocalPosition = event.localPosition;
    _dragMetrics = metrics;
    final extent = terminalCellOffset(widget.controller.snapshot, position);
    _dragDidSelectText = _dragDidSelectText || extent != anchor;

    _setSelection(
      TerminalSelection.fromOffsets(anchor: anchor, extent: extent),
    );
    _updateSelectionAutoScroll(event.localPosition, metrics);
  }

  void _handlePointerUp(PointerUpEvent event, TerminalMetrics metrics) {
    if (event.pointer == _openTargetPointer) {
      final pressed = _pressedOpenTarget;
      final released = _openTargetAt(
        _cellPositionForOffset(event.localPosition, metrics),
      );
      _pressedOpenTarget = null;
      _openTargetPointer = null;
      if (pressed != null && released?.uri == pressed.uri) {
        _openTerminalTarget(pressed);
      }
      return;
    }
    if (event.pointer == _cursorMovePointer) {
      _cursorMovePointer = null;
      _cursorMovePressedPosition = null;
      final dragged = _cursorMoveDragged;
      _cursorMoveDragged = false;
      if (!dragged && _effectiveSelection == null) {
        _moveCursorForPromptClick(
          _cellPositionForOffset(event.localPosition, metrics),
        );
      }
      return;
    }
    if (event.pointer == _ignoredOptionPointer) {
      _ignoredOptionPointer = null;
      return;
    }
    if (event.pointer != _dragPointer) {
      return;
    }
    if (_isPointerInsideSearchOverlay(event.position)) {
      _endDragSelection();
      return;
    }
    final position = _cellPositionForOffset(event.localPosition, metrics);
    _sendMouseReport(position: position, button: 3, release: true);
    final anchor = _dragAnchorOffset;
    final tapCount = _dragTapCount;
    final didSelectText = _dragDidSelectText;
    final suppressCommandBlock = _suppressCommandBlockForPointer;
    final previousCommandBlock = _commandBlockBeforePointerDown;
    _endDragSelection();
    if (anchor != null &&
        tapCount == 1 &&
        !didSelectText &&
        suppressCommandBlock) {
      return;
    }
    if (anchor != null &&
        tapCount == 1 &&
        !didSelectText &&
        !suppressCommandBlock &&
        terminalSelectCommandBlockOnClick) {
      final block = widget.controller.commandBlockAt(position);
      if (block != null && block.selection == previousCommandBlock) {
        _setCommandBlockSelection(null, block: null);
      } else {
        _setCommandBlockSelection(block?.selection, block: block);
      }
    }
    if (anchor != null &&
        (_config.copyOnSelect || terminalCopyOnSelect) &&
        _effectiveSelection != null &&
        !_effectiveSelection!.isCollapsed) {
      _copySelection();
    }
  }

  void _handlePointerCancel(PointerCancelEvent event) {
    if (event.pointer == _openTargetPointer) {
      _pressedOpenTarget = null;
      _openTargetPointer = null;
      return;
    }
    if (event.pointer == _cursorMovePointer) {
      _cursorMovePointer = null;
      _cursorMovePressedPosition = null;
      _cursorMoveDragged = false;
      return;
    }
    if (event.pointer == _ignoredOptionPointer) {
      _ignoredOptionPointer = null;
      return;
    }
    if (event.pointer == _dragPointer) {
      _endDragSelection();
    }
  }

  void _endDragSelection() {
    _stopSelectionAutoScroll();
    _dragAnchorOffset = null;
    _dragPointer = null;
    _dragLocalPosition = null;
    _dragMetrics = null;
    _dragTapCount = 0;
    _dragDidSelectText = false;
    _suppressCommandBlockForPointer = false;
    _commandBlockBeforePointerDown = null;
  }

  void _updateSelectionAutoScroll(
    Offset localPosition,
    TerminalMetrics metrics,
  ) {
    _dragLocalPosition = localPosition;
    _dragMetrics = metrics;
    if (_selectionAutoScrollLines(localPosition, metrics) == 0) {
      _stopSelectionAutoScroll();
      return;
    }

    _selectionAutoScrollTimer ??= Timer.periodic(
      _selectionAutoScrollInterval,
      (_) => _autoScrollSelection(),
    );
  }

  int _selectionAutoScrollLines(Offset localPosition, TerminalMetrics metrics) {
    final contentTop = _padding.top;
    final contentBottom = contentTop + _lastLayoutHeight;
    final double overflow;
    final int direction;
    if (localPosition.dy < contentTop) {
      overflow = contentTop - localPosition.dy;
      direction = 1;
    } else if (localPosition.dy > contentBottom) {
      overflow = localPosition.dy - contentBottom;
      direction = -1;
    } else {
      return 0;
    }

    final speed = math.max(1, (overflow / metrics.cellSize.height).ceil());
    return direction * math.min(speed, _maximumSelectionAutoScrollLines);
  }

  void _autoScrollSelection() {
    final anchor = _dragAnchorOffset;
    final localPosition = _dragLocalPosition;
    final metrics = _dragMetrics;
    if (!mounted ||
        anchor == null ||
        localPosition == null ||
        metrics == null) {
      _stopSelectionAutoScroll();
      return;
    }

    final lines = _selectionAutoScrollLines(localPosition, metrics);
    if (lines == 0) {
      _stopSelectionAutoScroll();
      return;
    }

    final beforeOffset = widget.controller.snapshot.displayOffset;
    widget.controller.scrollLines(lines);
    final snapshot = widget.controller.snapshot;
    final scrolledLines = snapshot.displayOffset - beforeOffset;
    if (scrolledLines == 0) {
      _stopSelectionAutoScroll();
      return;
    }

    _setSelection(
      TerminalSelection.fromOffsets(
        anchor: anchor,
        extent: terminalCellOffset(
          snapshot,
          _cellPositionForOffset(localPosition, metrics),
        ),
      ),
    );
  }

  void _stopSelectionAutoScroll() {
    _selectionAutoScrollTimer?.cancel();
    _selectionAutoScrollTimer = null;
  }

  void _handlePointerSignal(PointerSignalEvent event, TerminalMetrics metrics) {
    if (_isPointerInsideSearchOverlay(event.position)) {
      return;
    }

    if (event is! PointerScrollEvent) {
      return;
    }

    GestureBinding.instance.pointerSignalResolver.register(event, (
      resolvedEvent,
    ) {
      if (resolvedEvent is PointerScrollEvent) {
        _handleResolvedPointerScroll(resolvedEvent, metrics);
      }
    });
  }

  void _handlePointerHover(PointerHoverEvent event, TerminalMetrics metrics) {
    _lastHoverLocalPosition = event.localPosition;
    _lastHoverMetrics = metrics;
    _refreshOpenTargetHover();
  }

  void _refreshOpenTargetHover() {
    final localPosition = _lastHoverLocalPosition;
    final metrics = _lastHoverMetrics;
    final target =
        localPosition == null ||
            metrics == null ||
            !_isOpenTargetModifierPressed()
        ? null
        : _openTargetAt(_cellPositionForOffset(localPosition, metrics));
    if (_hoveredOpenTarget?.uri == target?.uri &&
        _hoveredOpenTarget?.selection == target?.selection) {
      return;
    }
    setState(() => _hoveredOpenTarget = target);
  }

  void _clearOpenTargetHover() {
    _lastHoverLocalPosition = null;
    _lastHoverMetrics = null;
    if (_hoveredOpenTarget != null) {
      setState(() => _hoveredOpenTarget = null);
    }
  }

  bool _isOpenTargetModifierPressed() {
    if (!terminalPointerConfig.commandClickOpensFilenameOrUrl) return false;
    final keyboard = HardwareKeyboard.instance;
    return Platform.isMacOS
        ? keyboard.isMetaPressed
        : keyboard.isControlPressed;
  }

  bool _isOptionCursorMoveModifierPressed() {
    final keyboard = HardwareKeyboard.instance;
    return terminalPointerConfig.optionClickMovesCursor &&
        keyboard.isAltPressed &&
        !keyboard.isControlPressed &&
        !keyboard.isMetaPressed;
  }

  bool _canMoveCursorWithPointer() {
    final snapshot = widget.controller.snapshot;
    return !widget.readOnly &&
        snapshot.cursor.visible &&
        !snapshot.alternateScreen &&
        !snapshot.keyboardMode.mouseReporting &&
        snapshot.displayOffset == 0;
  }

  void _moveCursorForPromptClick(TerminalCellPosition target) {
    if (!_canMoveCursorWithPointer()) return;
    final movement = widget.controller.promptClickMove(target);
    if (movement == null) return;
    if (movement.isEmpty) return;
    final snapshot = widget.controller.snapshot;
    final prefix = snapshot.keyboardMode.applicationCursor ? '\x1bO' : '\x1b[';
    final sequence = movement.left > 0
        ? List.filled(movement.left, '${prefix}D').join()
        : List.filled(movement.right, '${prefix}C').join();
    _clearSelection();
    widget.controller.sendInput(sequence);
  }

  TerminalOpenTarget? _openTargetAt(TerminalCellPosition position) {
    final snapshot = widget.controller.snapshot;
    final allowLocalPaths =
        !widget.readOnly && widget.controller.isLocalTerminal;
    if (_openTargetCacheValid &&
        identical(_openTargetCacheSnapshot, snapshot) &&
        _openTargetCachePosition == position &&
        _openTargetCacheAllowsLocalPaths == allowLocalPaths) {
      return _openTargetCacheValue;
    }
    String? commandPromptText;
    String? commandWorkingDirectory;
    if (allowLocalPaths) {
      final block = widget.controller.commandBlockAt(position);
      if (block != null) {
        commandWorkingDirectory = block.workingDirectory;
        commandPromptText = widget.controller.selectionText(
          TerminalSelection(
            start: block.selection.start,
            end: block.selection.start + snapshot.columns,
          ),
        );
      }
    }
    final target = terminalOpenTargetAt(
      snapshot,
      position,
      allowLocalPaths: allowLocalPaths,
      commandPromptText: commandPromptText,
      commandWorkingDirectory: commandWorkingDirectory,
    );
    _openTargetCacheSnapshot = snapshot;
    _openTargetCachePosition = position;
    _openTargetCacheValue = target;
    _openTargetCacheAllowsLocalPaths = allowLocalPaths;
    _openTargetCacheValid = true;
    return target;
  }

  void _openTerminalTarget(TerminalOpenTarget target) {
    final callback = widget.onOpenTarget;
    unawaited(
      Future<void>.sync(() async {
        if (callback == null) {
          await openTerminalTarget(target);
        } else {
          await callback(target);
        }
      }).catchError((Object error, StackTrace stackTrace) {
        NautermLog.warning(
          'terminal',
          'Unable to open terminal target.',
          error: error,
          stackTrace: stackTrace,
        );
      }),
    );
  }

  void _handleResolvedPointerScroll(
    PointerScrollEvent event,
    TerminalMetrics metrics,
  ) {
    _handleTerminalScroll(
      localPosition: event.localPosition,
      scrollDeltaY: event.scrollDelta.dy,
      metrics: metrics,
    );
  }

  void _attachTerminalScrollPosition(ScrollPosition position) {
    _terminalScrollPosition = position;
    _terminalScrollLastPixels = position.pixels;
    position.addListener(_handleTerminalScrollPositionChanged);
    position.isScrollingNotifier.addListener(
      _handleTerminalScrollActivityChanged,
    );
    _scheduleTerminalScrollSync();
  }

  void _detachTerminalScrollPosition(ScrollPosition position) {
    position.removeListener(_handleTerminalScrollPositionChanged);
    position.isScrollingNotifier.removeListener(
      _handleTerminalScrollActivityChanged,
    );
    if (identical(_terminalScrollPosition, position)) {
      _terminalScrollPosition = null;
    }
  }

  void _handleTerminalScrollActivityChanged() {
    final position = _terminalScrollPosition;
    if (position == null) return;
    _terminalScrollActive = position.isScrollingNotifier.value;
    if (_terminalScrollActive) {
      _terminalScrollLineRemainder = 0;
      _terminalScrollLastPixels = position.pixels;
      return;
    }
    _terminalScrollLineRemainder = 0;
    _trackpadScrollIgnored = false;
    _scheduleTerminalScrollSync();
  }

  void _handleTerminalScrollPositionChanged() {
    final position = _terminalScrollPosition;
    if (position == null) return;
    final pixels = position.pixels;
    final delta = pixels - _terminalScrollLastPixels;
    _terminalScrollLastPixels = pixels;
    if (_terminalScrollSynchronizing ||
        _trackpadScrollIgnored ||
        _terminalReportsMouseEvents ||
        delta == 0) {
      return;
    }

    _terminalScrollLineRemainder += -delta / _terminalScrollCellHeight;
    final wholeLines = _terminalScrollLineRemainder.truncate();
    if (wholeLines == 0) return;
    _terminalScrollLineRemainder -= wholeLines;
    _scrollTerminalLines(
      wholeLines,
      _dragLocalPosition ?? _trackpadScrollLocalPosition ?? Offset.zero,
      _dragMetrics ??
          _trackpadScrollMetrics ??
          TerminalMetrics.measure(_config.font.textStyle()),
    );
  }

  bool get _terminalReportsMouseEvents {
    return !widget.readOnly &&
        _config.keyboard.reportMouseEvents &&
        widget.controller.snapshot.keyboardMode.mouseReporting;
  }

  void _scheduleTerminalScrollSync() {
    if (_terminalScrollActive || _terminalScrollSyncScheduled) return;
    _terminalScrollSyncScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _terminalScrollSyncScheduled = false;
      _syncTerminalScrollPosition();
    });
  }

  void _syncTerminalScrollPosition() {
    final position = _terminalScrollPosition;
    if (!mounted ||
        _terminalScrollActive ||
        position == null ||
        !position.hasContentDimensions) {
      return;
    }
    final snapshot = widget.controller.snapshot;
    final target =
        (snapshot.historyLines - snapshot.displayOffset) *
        _terminalScrollCellHeight;
    final clampedTarget = target
        .clamp(position.minScrollExtent, position.maxScrollExtent)
        .toDouble();
    if (position.pixels == clampedTarget) return;
    _terminalScrollSynchronizing = true;
    position.jumpTo(clampedTarget);
    _terminalScrollLastPixels = clampedTarget;
    _terminalScrollSynchronizing = false;
  }

  void _handleTrackpadPanZoomStart(PointerPanZoomStartEvent event) {
    _trackpadScrollIgnored = _isPointerInsideSearchOverlay(event.position);
    _trackpadScrollLocalPosition = event.localPosition;
    _terminalScrollLineRemainder = 0;
    final position = _terminalScrollPosition;
    if (position != null) {
      _terminalScrollLastPixels = position.pixels;
    }
  }

  void _handleTrackpadPanZoomUpdate(
    PointerPanZoomUpdateEvent event,
    TerminalMetrics metrics,
  ) {
    final scrollDeltaY = -event.localPanDelta.dy;
    _trackpadScrollLocalPosition = event.localPosition;
    _trackpadScrollMetrics = metrics;
    if (_trackpadScrollIgnored || scrollDeltaY == 0) {
      return;
    }
    final position = _cellPositionForOffset(event.localPosition, metrics);
    final scrollButton = scrollDeltaY < 0 ? 64 : 65;
    if (_sendMouseReport(position: position, button: scrollButton)) {
      return;
    }
  }

  void _handleTrackpadPanZoomEnd(PointerPanZoomEndEvent _) {
    if (_terminalScrollPosition?.isScrollingNotifier.value ?? false) return;
    _terminalScrollLineRemainder = 0;
    _trackpadScrollIgnored = false;
    _scheduleTerminalScrollSync();
  }

  void _handleTerminalScroll({
    required Offset localPosition,
    required double scrollDeltaY,
    required TerminalMetrics metrics,
  }) {
    final position = _cellPositionForOffset(localPosition, metrics);
    final scrollButton = scrollDeltaY < 0 ? 64 : 65;
    if (_sendMouseReport(position: position, button: scrollButton)) {
      return;
    }

    _scrollLineRemainder += -scrollDeltaY / metrics.cellSize.height;
    final wholeLines = _scrollLineRemainder.truncate();
    if (wholeLines == 0) {
      return;
    }

    _scrollLineRemainder -= wholeLines;
    _scrollTerminalLines(wholeLines, localPosition, metrics);
  }

  void _scrollTerminalLines(
    int lines,
    Offset localPosition,
    TerminalMetrics metrics,
  ) {
    final beforeOffset = widget.controller.snapshot.displayOffset;
    widget.controller.scrollLines(lines);
    final snapshot = widget.controller.snapshot;
    final scrolledLines = snapshot.displayOffset - beforeOffset;
    final anchor = _dragAnchorOffset;
    if (anchor != null && scrolledLines != 0) {
      _setSelection(
        TerminalSelection.fromOffsets(
          anchor: anchor,
          extent: terminalCellOffset(
            snapshot,
            _cellPositionForOffset(localPosition, metrics),
          ),
        ),
      );
    }
  }

  TerminalCellPosition _cellPositionForOffset(
    Offset localPosition,
    TerminalMetrics metrics,
  ) {
    final snapshot = widget.controller.snapshot;
    final padding = _padding;
    final x = localPosition.dx - padding.left;
    final y = localPosition.dy - padding.top;
    final column = (x / metrics.cellSize.width)
        .floor()
        .clamp(0, snapshot.columns - 1)
        .toInt();
    final row = (y / metrics.cellSize.height)
        .floor()
        .clamp(0, snapshot.rows - 1)
        .toInt();

    return TerminalCellPosition(row: row, column: column);
  }

  bool _isPointerInsideSearchOverlay(Offset globalPosition) {
    if (!_searchVisible) {
      return false;
    }

    final renderObject = _searchOverlayKey.currentContext?.findRenderObject();
    if (renderObject is! RenderBox || !renderObject.hasSize) {
      return false;
    }

    final localPosition = renderObject.globalToLocal(globalPosition);
    return (Offset.zero & renderObject.size).contains(localPosition);
  }

  bool _sendMouseMotionReport(
    PointerMoveEvent event,
    TerminalCellPosition position,
  ) {
    final mode = widget.controller.snapshot.keyboardMode;
    if (!mode.mouseMotion &&
        !(mode.mouseDrag && event.buttons != 0) &&
        !mode.mouseReportClick) {
      return false;
    }

    final button = _mouseButtonCode(event.buttons) ?? 3;
    return _sendMouseReport(position: position, button: button, motion: true);
  }

  bool _sendMouseReport({
    required TerminalCellPosition position,
    required int? button,
    bool motion = false,
    bool release = false,
  }) {
    if (widget.readOnly || !_config.keyboard.reportMouseEvents) return false;
    final mode = widget.controller.snapshot.keyboardMode;
    if (!mode.mouseReporting || button == null) {
      return false;
    }
    if (motion && !mode.mouseMotion && !mode.mouseDrag) {
      return false;
    }

    var code = release ? 3 : button;
    if (motion) {
      code += 32;
    }
    code += _mouseModifierMask();

    final column = position.column + 1;
    final row = position.row + 1;
    final suffix = release ? 'm' : 'M';
    final sequence = mode.sgrMouse
        ? '\x1b[<$code;$column;$row$suffix'
        : _x10MouseSequence(code, column, row);
    widget.controller.sendInput(sequence);
    return true;
  }

  String _x10MouseSequence(int code, int column, int row) {
    final encodedCode = (code + 32).clamp(32, 255).toInt();
    final encodedColumn = (column + 32).clamp(32, 255).toInt();
    final encodedRow = (row + 32).clamp(32, 255).toInt();
    return String.fromCharCodes([
      0x1b,
      0x5b,
      0x4d,
      encodedCode,
      encodedColumn,
      encodedRow,
    ]);
  }

  int? _mouseButtonCode(int buttons) {
    if ((buttons & kPrimaryMouseButton) != 0) {
      return 0;
    }
    if ((buttons & kMiddleMouseButton) != 0) {
      return 1;
    }
    if ((buttons & kSecondaryMouseButton) != 0) {
      return 2;
    }
    return null;
  }

  int _mouseModifierMask() {
    final keyboard = HardwareKeyboard.instance;
    var mask = 0;
    if (keyboard.isShiftPressed) {
      mask += 4;
    }
    if (keyboard.isAltPressed) {
      mask += 8;
    }
    if (keyboard.isControlPressed) {
      mask += 16;
    }
    return mask;
  }

  int _registerTap(TerminalCellPosition position) {
    final now = DateTime.now();
    final previousTapTime = _lastTapTime;
    final previousTapPosition = _lastTapPosition;
    final withinTimeout =
        previousTapTime != null &&
        now.difference(previousTapTime) <= _multiTapTimeout;
    final repeatTap =
        previousTapPosition != null &&
        previousTapPosition.row == position.row &&
        (previousTapPosition.column - position.column).abs() <= 1;

    if (withinTimeout && repeatTap) {
      _tapCount = _tapCount >= 3 ? 1 : _tapCount + 1;
    } else {
      _tapCount = 1;
    }

    _lastTapTime = now;
    _lastTapPosition = position;
    return _tapCount;
  }

  void _handleKeyboardAction(TerminalKeyboardAction action) {
    switch (action) {
      case TerminalKeyboardAction.copy:
        _copySelection();
      case TerminalKeyboardAction.paste:
        unawaited(_pasteClipboard());
      case TerminalKeyboardAction.selectAll:
        _selectAllText();
    }
  }

  void _requestFocus() {
    _focusNode.requestFocus();
  }

  void _clearTerminal() {
    if (widget.readOnly) return;
    _clearSelection();
    widget.controller.clear();
  }

  void _resetTerminal() {
    if (widget.readOnly) return;
    _clearSelection();
    widget.controller.reset();
  }

  void _selectAllText() {
    _setSelection(TerminalSelection.all(widget.controller.snapshot));
  }

  void _copySelection() {
    final selection = _effectiveSelection;
    final text = selection == null
        ? ''
        : widget.controller.selectionText(selection);
    if (text.isEmpty) {
      return;
    }

    unawaited(Clipboard.setData(ClipboardData(text: text)));
  }

  Future<void> _pasteClipboard() async {
    if (widget.readOnly) return;
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    if (!mounted) {
      return;
    }

    final text = data?.text;
    if (text == null || text.isEmpty) {
      return;
    }

    _clearSelection();
    widget.controller.sendInput(
      terminalPasteSequence(text, widget.controller.snapshot.keyboardMode),
    );
  }

  void _setSelection(TerminalSelection? selection) {
    if (_selection == selection && _commandBlockSelection == null) {
      return;
    }

    setState(() {
      _selection = selection;
      _commandBlockSelection = null;
    });
    widget.controller.updateSelectedText(
      selection == null ? '' : widget.controller.selectionText(selection),
    );
    widget.controller.updateSelectedCommandBlock(null);
  }

  void _clearSelection() {
    if (_selection == null && _commandBlockSelection == null) {
      return;
    }

    setState(() {
      _selection = null;
      _commandBlockSelection = null;
    });
    widget.controller.updateSelectedText(tr(''));
    widget.controller.updateSelectedCommandBlock(null);
  }

  TerminalSelection? get _effectiveSelection =>
      _selection ?? _commandBlockSelection;

  void _setCommandBlockSelection(
    TerminalSelection? selection, {
    required TerminalCommandBlock? block,
  }) {
    if (_commandBlockSelection == selection && _selection == null) {
      return;
    }
    setState(() {
      _selection = null;
      _commandBlockSelection = selection;
    });
    widget.controller.updateSelectedText(
      selection == null ? '' : widget.controller.selectionText(selection),
    );
    widget.controller.updateSelectedCommandBlock(block);
  }
}

class _TerminalSearchButton extends StatelessWidget {
  const _TerminalSearchButton({
    required this.icon,
    required this.tooltip,
    required this.theme,
    required this.background,
    required this.onPressed,
  });

  final IconData icon;
  final String tooltip;
  final TerminalTheme theme;
  final Color background;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final foreground = theme.primary.foreground;
    return Tooltip(
      message: tr(tooltip),
      waitDuration: const Duration(milliseconds: 450),
      child: SizedBox(
        width: 28,
        height: 28,
        child: IconButton(
          onPressed: onPressed,
          icon: Icon(icon, size: 18),
          color: foreground.withValues(alpha: 0.78),
          disabledColor: foreground.withValues(alpha: 0.28),
          padding: EdgeInsets.zero,
          splashRadius: 16,
          style: IconButton.styleFrom(
            backgroundColor: Colors.transparent,
            hoverColor: Color.alphaBlend(
              foreground.withValues(alpha: 0.08),
              background,
            ),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(6),
            ),
          ),
        ),
      ),
    );
  }
}
