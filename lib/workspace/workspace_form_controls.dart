// ignore_for_file: unused_element_parameter, unused_field

part of 'nauterm_workspace.dart';

const double _workspaceFieldAffixWidth = 16;
const double _workspaceInputAffixGap = 4;
const double _workspaceSelectAffixGap = 4;
const double _workspaceTextareaScrollbarThickness = 6;
const double _workspaceBaseControlFontSize = 14;
const double _workspaceSelectOptionHorizontalPadding = 12;

enum _WorkspaceControlSize {
  tiny(24, 32, 8),
  small(24, 32, 9),
  medium(32, 32, 12),
  large(40, 32, 14);

  const _WorkspaceControlSize(
    this.fieldHeight,
    this.optionHeight,
    this.horizontalPadding,
  );

  final double fieldHeight;
  final double optionHeight;
  final double horizontalPadding;

  double get inputHeight => switch (this) {
    tiny => 16,
    small => 24,
    medium => 32,
    large => 40,
  };

  double get inputFontSize => switch (this) {
    tiny => 12,
    small || medium || large => _workspaceBaseControlFontSize,
  };

  double get inputLineHeight => (inputFontSize + 8) / inputFontSize;

  double get inputHorizontalPadding => switch (this) {
    // InputDecorator adds another 4 logical pixels around its child.
    // These values keep the rendered insets at Ant Design's 7 / 11 px.
    tiny || small => 3,
    medium || large => 7,
  };

  double get inputVerticalPadding => switch (this) {
    tiny || small => 0,
    medium => 4,
    large => 7,
  };
}

class _WorkspaceControlSizeScope extends InheritedWidget {
  const _WorkspaceControlSizeScope({required this.size, required super.child});

  final _WorkspaceControlSize size;

  static _WorkspaceControlSize? maybeOf(BuildContext context) {
    return context
        .dependOnInheritedWidgetOfExactType<_WorkspaceControlSizeScope>()
        ?.size;
  }

  static _WorkspaceControlSize? get(BuildContext context) {
    return context
        .getInheritedWidgetOfExactType<_WorkspaceControlSizeScope>()
        ?.size;
  }

  @override
  bool updateShouldNotify(_WorkspaceControlSizeScope oldWidget) {
    return size != oldWidget.size;
  }
}

enum _WorkspaceButtonVariant { solid, outlined, dashed, filled, text, link }

enum _WorkspaceButtonType {
  defaultType,
  primary,
  info,
  success,
  warning,
  error,
}

class _WorkspaceButton extends StatelessWidget {
  const _WorkspaceButton({
    required this.onPressed,
    this.label,
    this.icon,
    this.child,
    this.variant = _WorkspaceButtonVariant.filled,
    this.type = _WorkspaceButtonType.defaultType,
    this.size = _WorkspaceControlSize.medium,
    this.color,
    this.shape,
    this.fullWidth = false,
    this.iconGap = 6,
    this.horizontalPadding,
    this.height,
    this.width,
    this.minWidth,
    this.tooltip,
  }) : assert(label != null || icon != null || child != null);

  final VoidCallback? onPressed;
  final String? label;
  final IconData? icon;
  final Widget? child;
  final _WorkspaceButtonVariant variant;
  final _WorkspaceButtonType type;
  final _WorkspaceControlSize size;
  final Color? color;
  final OutlinedBorder? shape;
  final bool fullWidth;
  final double iconGap;
  final double? horizontalPadding;
  final double? height;
  final double? width;
  final double? minWidth;
  final String? tooltip;

  bool get _iconOnly => label == null && child == null && icon != null;

  @override
  Widget build(BuildContext context) {
    final enabled = onPressed != null;
    final terminalColors = _WorkspaceDialogThemeScope.maybeOf(context);
    final palette = _WorkspaceButtonPalette.resolve(
      type: type,
      variant: variant,
      color: color,
      enabled: enabled,
      terminalColors: terminalColors,
    );
    final buttonShape =
        shape ?? RoundedRectangleBorder(borderRadius: BorderRadius.circular(7));
    final side = switch (variant) {
      _WorkspaceButtonVariant.outlined => BorderSide(
        color: palette.border,
        width: 1,
      ),
      _WorkspaceButtonVariant.dashed => BorderSide(
        color: palette.border,
        width: 1,
      ),
      _ => BorderSide.none,
    };
    final effectiveShape = buttonShape.copyWith(side: side);
    final buttonHeight = height ?? size.fieldHeight;
    final buttonWidth = fullWidth
        ? double.infinity
        : width ?? (_iconOnly ? buttonHeight : null);
    final buttonMinWidth = width == null ? minWidth ?? 0.0 : 0.0;
    final padding = EdgeInsets.symmetric(
      horizontal: _iconOnly ? 0 : horizontalPadding ?? size.horizontalPadding,
    );

    final buttonTextStyle = TextStyle(
      color: palette.foreground,
      fontSize: size.inputFontSize,
      fontWeight: NautermFontWeights.semibold,
      letterSpacing: 0,
      decoration: variant == _WorkspaceButtonVariant.link
          ? TextDecoration.underline
          : TextDecoration.none,
      decorationColor: palette.foreground,
    );
    final labelText = label == null
        ? null
        : Text(
            tr(label!),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            textAlign: TextAlign.center,
            style: buttonTextStyle,
          );

    final iconSize = size == _WorkspaceControlSize.tiny ? 16.0 : 18.0;
    Widget content = DefaultTextStyle.merge(
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: buttonTextStyle,
      child:
          child ??
          (icon == null
              ? labelText!
              : LayoutBuilder(
                  builder: (context, constraints) {
                    final boundedWidth = constraints.hasBoundedWidth
                        ? constraints.maxWidth
                        : double.infinity;
                    if (boundedWidth < iconSize) {
                      return const SizedBox.shrink();
                    }
                    final showLabel =
                        labelText != null &&
                        boundedWidth >= iconSize + iconGap + 1;
                    return Row(
                      mainAxisSize: MainAxisSize.min,
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(icon, size: iconSize, color: palette.foreground),
                        if (showLabel) ...[
                          SizedBox(width: iconGap),
                          Flexible(child: labelText),
                        ],
                      ],
                    );
                  },
                )),
    );

    content = IconTheme.merge(
      data: IconThemeData(color: palette.foreground, size: iconSize),
      child: content,
    );

    final button = SizedBox(
      width: buttonWidth,
      height: buttonHeight,
      child: ConstrainedBox(
        constraints: BoxConstraints(minWidth: buttonMinWidth),
        child: Material(
          color: palette.background,
          shape: effectiveShape,
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            splashFactory: InkRipple.splashFactory,
            customBorder: effectiveShape,
            hoverColor: palette.hover,
            splashColor: palette.splash,
            highlightColor: palette.splash,
            onTap: onPressed,
            child: Padding(
              padding: padding,
              child: Center(widthFactor: fullWidth ? null : 1, child: content),
            ),
          ),
        ),
      ),
    );

    final painted = variant == _WorkspaceButtonVariant.dashed
        ? CustomPaint(
            foregroundPainter: _DashedShapeBorderPainter(
              shape: effectiveShape,
              color: palette.border,
            ),
            child: button,
          )
        : button;

    if (tooltip == null) {
      return painted;
    }

    return Tooltip(
      message: tr(tooltip!),
      waitDuration: const Duration(milliseconds: 250),
      child: painted,
    );
  }
}

class _WorkspaceButtonPalette {
  const _WorkspaceButtonPalette({
    required this.background,
    required this.foreground,
    required this.border,
    required this.hover,
    required this.splash,
  });

  final Color background;
  final Color foreground;
  final Color border;
  final Color hover;
  final Color splash;

  static _WorkspaceButtonPalette resolve({
    required _WorkspaceButtonType type,
    required _WorkspaceButtonVariant variant,
    required Color? color,
    required bool enabled,
    _AiAssistantColors? terminalColors,
  }) {
    if (terminalColors != null) {
      return _resolveTerminal(
        type: type,
        variant: variant,
        color: color,
        enabled: enabled,
        colors: terminalColors,
      );
    }
    final base = color ?? _buttonTypeColor(type);
    final foregroundBase = type == _WorkspaceButtonType.defaultType
        ? _text
        : base;
    final disabledForeground = _mutedText;
    final disabledBackground = _workspaceDark
        ? const Color(0xff222b35)
        : const Color(0xffe2eaec);
    final tonalBackground = _workspaceDark
        ? type == _WorkspaceButtonType.defaultType
              ? _sidebarHover
              : base.withValues(alpha: 0.16)
        : const Color(0xffe0e4e6);
    final hover = _workspaceDark
        ? type == _WorkspaceButtonType.defaultType
              ? _sidebarPressed
              : base.withValues(alpha: 0.22)
        : _blend(Colors.white, base, 0.14);
    final splash = _workspaceDark
        ? type == _WorkspaceButtonType.defaultType
              ? _workspaceMenuPressed
              : base.withValues(alpha: 0.28)
        : _blend(Colors.white, base, 0.22);

    if (!enabled) {
      return _WorkspaceButtonPalette(
        background: switch (variant) {
          _WorkspaceButtonVariant.solid ||
          _WorkspaceButtonVariant.filled => disabledBackground,
          _ => Colors.transparent,
        },
        foreground: disabledForeground,
        border: _workspaceDark ? _sidebarDivider : const Color(0xffd2dee1),
        hover: Colors.transparent,
        splash: Colors.transparent,
      );
    }

    return switch (variant) {
      _WorkspaceButtonVariant.solid => _WorkspaceButtonPalette(
        background: base,
        foreground: Colors.white,
        border: base,
        hover: _blend(base, Colors.white, 0.08),
        splash: _blend(base, Colors.black, 0.08),
      ),
      _WorkspaceButtonVariant.filled => _WorkspaceButtonPalette(
        background: tonalBackground,
        foreground: foregroundBase,
        border: Colors.transparent,
        hover: hover,
        splash: splash,
      ),
      _WorkspaceButtonVariant.outlined => _WorkspaceButtonPalette(
        background: Colors.transparent,
        foreground: foregroundBase,
        border: _workspaceDark
            ? type == _WorkspaceButtonType.defaultType
                  ? _sidebarDivider
                  : base.withValues(alpha: 0.48)
            : _blend(Colors.white, base, 0.46),
        hover: hover,
        splash: splash,
      ),
      _WorkspaceButtonVariant.dashed => _WorkspaceButtonPalette(
        background: Colors.transparent,
        foreground: foregroundBase,
        border: _workspaceDark
            ? type == _WorkspaceButtonType.defaultType
                  ? _sidebarDivider
                  : base.withValues(alpha: 0.48)
            : _blend(Colors.white, base, 0.56),
        hover: hover,
        splash: splash,
      ),
      _WorkspaceButtonVariant.text => _WorkspaceButtonPalette(
        background: Colors.transparent,
        foreground: foregroundBase,
        border: Colors.transparent,
        hover: hover,
        splash: splash,
      ),
      _WorkspaceButtonVariant.link => _WorkspaceButtonPalette(
        background: Colors.transparent,
        foreground: foregroundBase,
        border: Colors.transparent,
        hover: Colors.transparent,
        splash: splash,
      ),
    };
  }

  static _WorkspaceButtonPalette _resolveTerminal({
    required _WorkspaceButtonType type,
    required _WorkspaceButtonVariant variant,
    required Color? color,
    required bool enabled,
    required _AiAssistantColors colors,
  }) {
    final semantic = switch (type) {
      _WorkspaceButtonType.error => const Color(0xffef4444),
      _WorkspaceButtonType.warning => const Color(0xfff59e0b),
      _WorkspaceButtonType.success => const Color(0xff22c55e),
      _ => colors.accent,
    };
    final base = color ?? semantic;
    final foreground = type == _WorkspaceButtonType.defaultType
        ? colors.foreground
        : base;
    if (!enabled) {
      return _WorkspaceButtonPalette(
        background:
            variant == _WorkspaceButtonVariant.solid ||
                variant == _WorkspaceButtonVariant.filled
            ? colors.inputBackground
            : Colors.transparent,
        foreground: colors.muted.withValues(alpha: 0.62),
        border: colors.border,
        hover: Colors.transparent,
        splash: Colors.transparent,
      );
    }
    final hover = Color.lerp(Colors.transparent, colors.foreground, 0.08)!;
    final splash = Color.lerp(Colors.transparent, colors.foreground, 0.14)!;
    return switch (variant) {
      _WorkspaceButtonVariant.solid => _WorkspaceButtonPalette(
        background: base,
        foreground: Colors.white,
        border: base,
        hover: Color.lerp(base, Colors.white, 0.09)!,
        splash: Color.lerp(base, Colors.black, 0.10)!,
      ),
      _WorkspaceButtonVariant.filled => _WorkspaceButtonPalette(
        background: colors.inputBackground,
        foreground: foreground,
        border: Colors.transparent,
        hover: hover,
        splash: splash,
      ),
      _WorkspaceButtonVariant.outlined ||
      _WorkspaceButtonVariant.dashed => _WorkspaceButtonPalette(
        background: Colors.transparent,
        foreground: foreground,
        border: colors.border,
        hover: hover,
        splash: splash,
      ),
      _WorkspaceButtonVariant.text ||
      _WorkspaceButtonVariant.link => _WorkspaceButtonPalette(
        background: Colors.transparent,
        foreground: foreground,
        border: Colors.transparent,
        hover: variant == _WorkspaceButtonVariant.link
            ? Colors.transparent
            : hover,
        splash: splash,
      ),
    };
  }

  static Color _buttonTypeColor(_WorkspaceButtonType type) {
    return switch (type) {
      _WorkspaceButtonType.defaultType => const Color(0xff62747b),
      _WorkspaceButtonType.primary => const Color(0xff075e92),
      _WorkspaceButtonType.info => const Color(0xff168df2),
      _WorkspaceButtonType.success => _green,
      _WorkspaceButtonType.warning => const Color(0xfff59e0b),
      _WorkspaceButtonType.error => const Color(0xffef4444),
    };
  }
}

class _DashedShapeBorderPainter extends CustomPainter {
  const _DashedShapeBorderPainter({required this.shape, required this.color});

  final OutlinedBorder shape;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    if ((color.a * 255.0).round().clamp(0, 255) == 0) {
      return;
    }

    final rect = Offset.zero & size;
    final path = shape.getOuterPath(rect.deflate(0.5));
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1;

    for (final metric in path.computeMetrics()) {
      var distance = 0.0;
      const dash = 4.0;
      const gap = 3.0;
      while (distance < metric.length) {
        final next = math.min(distance + dash, metric.length);
        canvas.drawPath(metric.extractPath(distance, next), paint);
        distance = next + gap;
      }
    }
  }

  @override
  bool shouldRepaint(covariant _DashedShapeBorderPainter oldDelegate) {
    return oldDelegate.shape != shape || oldDelegate.color != color;
  }
}

class _WorkspaceFormSection extends StatelessWidget {
  const _WorkspaceFormSection({required this.title, required this.children});

  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: _card,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              tr(title),
              style: TextStyle(
                color: _text,
                fontSize: NautermFontSizes.labelLarge,
                fontWeight: NautermFontWeights.semibold,
                letterSpacing: 0,
              ),
            ),
            SizedBox(height: 12),
            ...children,
          ],
        ),
      ),
    );
  }
}

class _WorkspaceInput extends StatelessWidget {
  const _WorkspaceInput({
    required this.controller,
    required this.label,
    this.hintText,
    this.size,
    this.isRequired = false,
    this.floatingLabel = true,
    this.autofocus = false,
    this.obscureText = false,
    this.keyboardType,
    this.inputFormatters,
    this.textInputAction,
    this.onSubmitted,
    this.minLines = 1,
    this.maxLines = 1,
    this.growable = false,
    this.clearable = true,
    this.trailing,
    this.errorText,
  });

  final TextEditingController controller;
  final String label;
  final String? hintText;
  final _WorkspaceControlSize? size;
  final bool isRequired;
  final bool floatingLabel;
  final bool autofocus;
  final bool obscureText;
  final TextInputType? keyboardType;
  final List<TextInputFormatter>? inputFormatters;
  final TextInputAction? textInputAction;
  final ValueChanged<String>? onSubmitted;
  final int minLines;
  final int maxLines;
  final bool growable;
  final bool clearable;
  final Widget? trailing;
  final String? errorText;

  bool get _singleLine => minLines == 1 && maxLines == 1;

  @override
  Widget build(BuildContext context) {
    final effectiveSize =
        size ??
        _WorkspaceControlSizeScope.maybeOf(context) ??
        _WorkspaceControlSize.medium;
    return _WorkspaceInputField(
      controller: controller,
      label: tr(label),
      hintText: hintText == null ? null : tr(hintText!),
      size: effectiveSize,
      isRequired: isRequired,
      floatingLabel: floatingLabel,
      autofocus: autofocus,
      obscureText: obscureText,
      keyboardType: keyboardType,
      inputFormatters: inputFormatters,
      textInputAction: textInputAction,
      onSubmitted: onSubmitted,
      minLines: minLines,
      maxLines: maxLines,
      singleLine: _singleLine,
      growable: growable,
      clearable: clearable,
      trailing: trailing,
      errorText: errorText,
    );
  }
}

class _WorkspaceInputField extends StatefulWidget {
  const _WorkspaceInputField({
    required this.controller,
    required this.label,
    required this.hintText,
    required this.size,
    required this.isRequired,
    required this.floatingLabel,
    required this.autofocus,
    required this.obscureText,
    required this.keyboardType,
    required this.inputFormatters,
    required this.textInputAction,
    required this.onSubmitted,
    required this.minLines,
    required this.maxLines,
    required this.singleLine,
    required this.growable,
    required this.clearable,
    required this.errorText,
    this.trailing,
  });

  final TextEditingController controller;
  final String label;
  final String? hintText;
  final _WorkspaceControlSize size;
  final bool isRequired;
  final bool floatingLabel;
  final bool autofocus;
  final bool obscureText;
  final TextInputType? keyboardType;
  final List<TextInputFormatter>? inputFormatters;
  final TextInputAction? textInputAction;
  final ValueChanged<String>? onSubmitted;
  final int minLines;
  final int maxLines;
  final bool singleLine;
  final bool growable;
  final bool clearable;
  final String? errorText;
  final Widget? trailing;

  @override
  State<_WorkspaceInputField> createState() => _WorkspaceInputFieldState();
}

class _WorkspaceInputFieldState extends State<_WorkspaceInputField> {
  late final FocusNode _focusNode;
  late final ScrollController _scrollController;

  @override
  void initState() {
    super.initState();
    _focusNode = FocusNode();
    _scrollController = ScrollController();
    _focusNode.addListener(_handleFocusChanged);
    widget.controller.addListener(_handleTextChanged);
  }

  @override
  void dispose() {
    _focusNode.removeListener(_handleFocusChanged);
    widget.controller.removeListener(_handleTextChanged);
    _focusNode.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  void _handleFocusChanged() {
    setState(() {});
  }

  void _handleTextChanged() {
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        return _buildField(context, constraints.maxWidth);
      },
    );
  }

  Widget _buildField(BuildContext context, double availableWidth) {
    final fieldHeight = widget.singleLine
        ? widget.size.inputHeight
        : widget.growable
        ? _growableFieldHeight(context, availableWidth)
        : math.max(
            widget.size.inputHeight,
            widget.maxLines *
                    widget.size.inputFontSize *
                    widget.size.inputLineHeight +
                widget.size.inputVerticalPadding * 2 +
                2,
          );
    final showClear =
        widget.clearable &&
        widget.trailing == null &&
        _focusNode.hasFocus &&
        widget.controller.text.isNotEmpty;
    return _WorkspaceFieldFrame(
      label: widget.label,
      isRequired: widget.isRequired,
      size: widget.size,
      focused: _focusNode.hasFocus,
      floatingLabel: widget.floatingLabel,
      hasContent: widget.controller.text.isNotEmpty,
      height: fieldHeight,
      textAlignVertical: widget.singleLine ? null : TextAlignVertical.top,
      alignLabelWithHint: !widget.singleLine,
      contentPadding: EdgeInsets.fromLTRB(
        widget.size.inputHorizontalPadding,
        widget.size.inputVerticalPadding,
        widget.singleLine ? widget.size.inputHorizontalPadding : 0,
        widget.size.inputVerticalPadding,
      ),
      onTap: _focusNode.requestFocus,
      errorText: widget.errorText,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final showTrailing =
              widget.trailing != null && constraints.maxWidth >= 42;
          final showClearAffix =
              widget.clearable &&
              widget.trailing == null &&
              constraints.maxWidth >= _workspaceFieldAffixWidth;
          if (!widget.singleLine && widget.trailing == null) {
            final clearRight = widget.size.inputHorizontalPadding;
            final clearTop = math.max(
              0.0,
              (widget.size.inputFontSize * widget.size.inputLineHeight -
                      _workspaceFieldAffixWidth) /
                  2,
            );
            final trailingInset = showClearAffix
                ? _workspaceFieldAffixWidth +
                      clearRight +
                      _workspaceInputAffixGap
                : 0.0;
            return Stack(
              children: [
                Positioned.fill(
                  child: Scrollbar(
                    controller: _scrollController,
                    thickness: _workspaceTextareaScrollbarThickness,
                    radius: const Radius.circular(
                      _workspaceTextareaScrollbarThickness / 2,
                    ),
                    child: Padding(
                      padding: EdgeInsets.only(right: trailingInset),
                      child: ScrollConfiguration(
                        behavior: const _WorkspaceTextareaScrollBehavior(),
                        child: _buildTextField(context),
                      ),
                    ),
                  ),
                ),
                if (showClearAffix && showClear)
                  Positioned(
                    top: clearTop,
                    right: clearRight,
                    child: SizedBox(
                      key: ValueKey('workspace-input-clear:${widget.label}'),
                      width: _workspaceFieldAffixWidth,
                      height: _workspaceFieldAffixWidth,
                      child: _WorkspaceClearButton(
                        tooltip: tr('Clear ${widget.label}'),
                        onPressed: () {
                          _focusNode.requestFocus();
                          widget.controller.clear();
                        },
                      ),
                    ),
                  ),
              ],
            );
          }
          return Row(
            children: [
              Expanded(child: _buildTextField(context)),
              if (showTrailing) ...[SizedBox(width: 6), widget.trailing!],
              if (showClearAffix) ...[
                SizedBox(width: _workspaceInputAffixGap),
                SizedBox(
                  key: ValueKey('workspace-input-clear:${widget.label}'),
                  width: _workspaceFieldAffixWidth,
                  height: _workspaceFieldAffixWidth,
                  child: showClear
                      ? _WorkspaceClearButton(
                          tooltip: tr('Clear ${widget.label}'),
                          onPressed: () {
                            _focusNode.requestFocus();
                            widget.controller.clear();
                          },
                        )
                      : null,
                ),
              ],
            ],
          );
        },
      ),
    );
  }

  double _growableFieldHeight(BuildContext context, double availableWidth) {
    final clearInset = widget.clearable && widget.trailing == null
        ? _workspaceFieldAffixWidth +
              widget.size.inputHorizontalPadding +
              _workspaceInputAffixGap
        : 0.0;
    final textWidth = math.max(
      1.0,
      availableWidth - widget.size.inputHorizontalPadding - clearInset - 2,
    );
    final text = widget.controller.text.isEmpty ? ' ' : widget.controller.text;
    final painter = TextPainter(
      text: TextSpan(
        text: text,
        style: TextStyle(
          fontSize: widget.size.inputFontSize,
          fontWeight: NautermFontWeights.regular,
          height: widget.size.inputLineHeight,
          letterSpacing: 0,
        ),
      ),
      textDirection: Directionality.of(context),
      maxLines: widget.maxLines,
    )..layout(maxWidth: textWidth);
    final visualLines = painter.computeLineMetrics().length.clamp(
      widget.minLines,
      widget.maxLines,
    );
    return math.max(
      widget.size.inputHeight,
      visualLines * widget.size.inputFontSize * widget.size.inputLineHeight +
          widget.size.inputVerticalPadding * 2 +
          2,
    );
  }

  Widget _buildTextField(BuildContext context) {
    final terminalColors = _WorkspaceDialogThemeScope.maybeOf(context);
    return TextField(
      controller: widget.controller,
      scrollController: widget.singleLine ? null : _scrollController,
      focusNode: _focusNode,
      autofocus: widget.autofocus,
      obscureText: widget.obscureText,
      keyboardType: widget.keyboardType,
      inputFormatters: widget.inputFormatters,
      textInputAction: widget.textInputAction,
      onSubmitted: widget.onSubmitted,
      minLines: widget.minLines,
      maxLines: widget.obscureText ? 1 : widget.maxLines,
      textAlignVertical: widget.singleLine ? null : TextAlignVertical.top,
      style: TextStyle(
        color: terminalColors?.foreground ?? _text,
        fontSize: widget.size.inputFontSize,
        fontWeight: NautermFontWeights.regular,
        height: widget.size.inputLineHeight,
        letterSpacing: 0,
      ),
      decoration: InputDecoration.collapsed(
        hintText: widget.hintText,
        hintStyle: TextStyle(
          color: (terminalColors?.foreground ?? _mutedText).withValues(
            alpha: 0.55,
          ),
          fontSize: widget.size.inputFontSize,
          fontWeight: NautermFontWeights.regular,
          height: widget.size.inputLineHeight,
        ),
      ),
    );
  }
}

class _WorkspaceTextareaScrollBehavior extends ScrollBehavior {
  const _WorkspaceTextareaScrollBehavior();

  @override
  Widget buildScrollbar(
    BuildContext context,
    Widget child,
    ScrollableDetails details,
  ) {
    return child;
  }
}

class _WorkspaceSelect<T> extends StatefulWidget {
  const _WorkspaceSelect({
    required this.label,
    required this.value,
    required this.items,
    required this.onChanged,
    this.size,
    this.isRequired = false,
    this.floatingLabel = true,
    this.editable = false,
    this.searchable = false,
    this.clearable = false,
    this.obscureText = false,
    this.inputController,
    this.onTextChanged,
    this.onCleared,
    this.createLabel,
    this.onCreate,
    this.leading,
    this.createConflictLabels = const [],
    this.colors,
    this.errorText,
    this.activationToken = 0,
  }) : assert((!searchable && !clearable) || editable),
       assert((createLabel == null) == (onCreate == null)),
       assert(createLabel == null || searchable);

  final String label;
  final T value;
  final List<DropdownMenuItem<T>> items;
  final ValueChanged<T?> onChanged;
  final _WorkspaceControlSize? size;
  final bool isRequired;
  final bool floatingLabel;
  final bool editable;
  final bool searchable;
  final bool clearable;
  final bool obscureText;
  final TextEditingController? inputController;
  final ValueChanged<String>? onTextChanged;
  final VoidCallback? onCleared;
  final String? createLabel;
  final ValueChanged<String>? onCreate;
  final Widget? leading;
  final Iterable<String> createConflictLabels;
  final _AiAssistantColors? colors;
  final String? errorText;

  /// Increment this to focus and open an editable select after it appears.
  final int activationToken;

  @override
  State<_WorkspaceSelect<T>> createState() => _WorkspaceSelectState<T>();
}

class _WorkspaceSelectState<T> extends State<_WorkspaceSelect<T>> {
  final GlobalKey _fieldKey = GlobalKey();
  final LayerLink _fieldLayerLink = LayerLink();
  final Object _overlayToken = Object();
  late final TextEditingController _internalInputController;
  late final FocusNode _inputFocusNode;
  NautermTransientOverlayHandle? _overlayEntry;
  NautermOverlayController? _overlayController;
  ScrollPosition? _ancestorScrollPosition;
  _WorkspaceControlSize _effectiveSize = _WorkspaceControlSize.medium;
  bool _active = true;
  bool _hovered = false;
  bool _rebuildScheduled = false;
  bool _scrollRebuildScheduled = false;

  bool get _open => _overlayEntry != null;
  TextEditingController get _inputController =>
      widget.inputController ?? _internalInputController;

  @override
  void didUpdateWidget(covariant _WorkspaceSelect<T> oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.size != widget.size) {
      _effectiveSize =
          widget.size ??
          _WorkspaceControlSizeScope.get(context) ??
          _WorkspaceControlSize.medium;
    }
    final oldController = oldWidget.inputController ?? _internalInputController;
    final nextController = widget.inputController ?? _internalInputController;
    if (!identical(oldController, nextController)) {
      oldController.removeListener(_handleInputTextChanged);
      nextController.addListener(_handleInputTextChanged);
    }
    final oldSelectedItem = oldWidget.items
        .where((item) => item.value == oldWidget.value)
        .firstOrNull;
    final oldSelectedText = _plainTextFor(oldSelectedItem?.child) ?? '';
    if (widget.searchable &&
        widget.inputController == null &&
        (oldWidget.value != widget.value ||
            oldSelectedText != _selectedText ||
            !_inputFocusNode.hasFocus)) {
      _restoreSelectedText();
    }
    if (widget.editable &&
        widget.activationToken != oldWidget.activationToken) {
      _scheduleActivation();
    }
    _markMenuNeedsBuild();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _effectiveSize =
        widget.size ??
        _WorkspaceControlSizeScope.maybeOf(context) ??
        _WorkspaceControlSize.medium;
    final nextScrollPosition = Scrollable.maybeOf(context)?.position;
    if (!identical(nextScrollPosition, _ancestorScrollPosition)) {
      _ancestorScrollPosition?.removeListener(_handleAncestorScroll);
      _ancestorScrollPosition = nextScrollPosition;
      _ancestorScrollPosition?.addListener(_handleAncestorScroll);
    }
    final nextController = NautermOverlayScope.maybeOf(context);
    if (identical(nextController, _overlayController)) {
      return;
    }
    _closeMenu();
    _overlayController = nextController;
  }

  @override
  void initState() {
    super.initState();
    _effectiveSize = widget.size ?? _WorkspaceControlSize.medium;
    _internalInputController = TextEditingController(
      text: widget.editable && widget.inputController == null
          ? _selectedText
          : null,
    );
    _inputFocusNode = FocusNode(onKeyEvent: _handleInputKeyEvent);
    _inputFocusNode.addListener(_handleInputFocusChanged);
    _inputController.addListener(_handleInputTextChanged);
    if (widget.editable && widget.activationToken != 0) {
      _scheduleActivation();
    }
  }

  @override
  void dispose() {
    final overlay = _overlayEntry;
    _overlayEntry = null;
    overlay?.dismiss(notify: false);
    _ancestorScrollPosition?.removeListener(_handleAncestorScroll);
    _inputController.removeListener(_handleInputTextChanged);
    _inputFocusNode.removeListener(_handleInputFocusChanged);
    _inputFocusNode.dispose();
    _internalInputController.dispose();
    super.dispose();
  }

  @override
  void activate() {
    super.activate();
    _active = true;
  }

  @override
  void deactivate() {
    _active = false;
    final overlay = _overlayEntry;
    _overlayEntry = null;
    overlay?.dismiss(notify: false);
    super.deactivate();
  }

  void _scheduleRebuild() {
    if (!_active || !mounted || _rebuildScheduled) {
      return;
    }
    _rebuildScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _rebuildScheduled = false;
      if (_active && mounted) {
        setState(() {});
      }
    });
  }

  void _handleInputFocusChanged() {
    _scheduleRebuild();
    if (!_inputFocusNode.hasFocus) {
      if (widget.searchable) {
        _restoreSelectedText();
      }
      _closeMenu();
    }
  }

  void _handleInputTextChanged() {
    _scheduleRebuild();
    if (!widget.searchable) {
      return;
    }
    if (_open) {
      if (_shouldOpenPanel) {
        _markMenuNeedsBuild();
      } else {
        _closeMenu();
      }
    } else if (_shouldOpenPanel) {
      // The panel was suppressed on focus because the field was empty with no
      // candidates; once the user types a value, open it to show matches or
      // the create option.
      _openMenu();
    }
  }

  KeyEventResult _handleInputKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) {
      return KeyEventResult.ignored;
    }
    if (!widget.editable &&
        (event.logicalKey == LogicalKeyboardKey.enter ||
            event.logicalKey == LogicalKeyboardKey.space ||
            event.logicalKey == LogicalKeyboardKey.arrowDown)) {
      _openMenu();
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  void _handleAncestorScroll() {
    if (!_open || _scrollRebuildScheduled) {
      return;
    }
    _scrollRebuildScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _scrollRebuildScheduled = false;
      if (_active && mounted && _open) {
        _markMenuNeedsBuild();
      }
    });
  }

  void _handleHoverChanged(bool hovered) {
    if (_hovered == hovered) {
      return;
    }
    setState(() => _hovered = hovered);
  }

  void _focusEditableField() {
    _inputFocusNode.requestFocus();
    if (!_open && _shouldOpenPanel) {
      _openMenu();
    }
  }

  void _scheduleActivation() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && widget.editable) {
        _focusEditableField();
      }
    });
  }

  void _toggleEditableMenu() {
    if (_open) {
      _closeMenu();
      return;
    }
    _focusEditableField();
  }

  void _clearInput() {
    if (_inputController.text.isEmpty) {
      return;
    }
    _inputController.clear();
    widget.onTextChanged?.call('');
    widget.onChanged(null);
    widget.onCleared?.call();
  }

  String get _selectedText {
    final selectedItem = widget.items
        .where((item) => item.value == widget.value)
        .firstOrNull;
    return _plainTextFor(selectedItem?.child) ?? '';
  }

  void _restoreSelectedText() {
    // An editable select may also represent free-form text (for example a
    // direct username or password).  Only replace that text on blur when an
    // actual option is selected; otherwise leaving the field must not erase
    // the user's draft.
    if (widget.value == null && widget.inputController != null) {
      return;
    }
    final selectedText = _selectedText;
    if (_inputController.text == selectedText) {
      return;
    }
    _inputController.value = TextEditingValue(
      text: selectedText,
      selection: TextSelection.collapsed(offset: selectedText.length),
    );
  }

  List<DropdownMenuItem<T>> get _visibleItems {
    if (!widget.searchable) {
      return widget.items;
    }
    final query = _inputController.text.trim().toLowerCase();
    if (query.isEmpty || query == _selectedText.trim().toLowerCase()) {
      return widget.items;
    }
    return widget.items
        .where(
          (item) =>
              (_plainTextFor(item.child) ?? '').toLowerCase().contains(query),
        )
        .toList(growable: false);
  }

  bool get _showCreateOption {
    if (widget.onCreate == null) {
      return false;
    }
    final query = _inputController.text.trim().toLowerCase();
    if (query.isEmpty) {
      return false;
    }
    return !widget.items.any(
          (item) =>
              (_plainTextFor(item.child) ?? '').trim().toLowerCase() == query,
        ) &&
        !widget.createConflictLabels.any(
          (label) => label.trim().toLowerCase() == query,
        );
  }

  String get _createOptionLabel =>
      '${widget.createLabel!} ${_inputController.text.trim()}';

  // Whether the dropdown panel should be shown. For a searchable field the
  // panel is pointless while the input is empty and there are no candidate
  // items to present, so focusing such a field must not pop an empty panel.
  // It reappears as soon as the user types a value (which yields matches or a
  // create option) or when candidates already exist.
  bool get _shouldOpenPanel {
    if (!widget.searchable) {
      return true;
    }
    final hasQuery = _inputController.text.trim().isNotEmpty;
    return hasQuery || widget.items.isNotEmpty;
  }

  void _toggleMenu() {
    _inputFocusNode.requestFocus();
    if (_open) {
      _closeMenu();
    } else {
      _openMenu();
    }
  }

  void _openMenu() {
    _overlayEntry = showNautermTransientOverlay(
      context: context,
      token: _overlayToken,
      dismissExisting: true,
      builder: _buildOverlay,
      onDismissed: () {
        _overlayEntry = null;
        _scheduleRebuild();
      },
    );
    setState(() {});
  }

  void _closeMenu() {
    final overlay = _overlayEntry;
    if (overlay == null) {
      return;
    }
    overlay.dismiss();
  }

  void _markMenuNeedsBuild() {
    _overlayEntry?.markNeedsBuild();
  }

  List<Widget> _buildDismissBarriers(Size overlaySize, Rect fieldRect) {
    final visibleFieldRect = fieldRect.intersect(Offset.zero & overlaySize);
    if (visibleFieldRect.isEmpty) {
      return [Positioned.fill(child: _buildDismissBarrier())];
    }
    return [
      Positioned(
        left: 0,
        top: 0,
        right: 0,
        height: visibleFieldRect.top,
        child: _buildDismissBarrier(),
      ),
      Positioned(
        left: 0,
        top: visibleFieldRect.bottom,
        right: 0,
        bottom: 0,
        child: _buildDismissBarrier(),
      ),
      Positioned(
        left: 0,
        top: visibleFieldRect.top,
        width: visibleFieldRect.left,
        height: visibleFieldRect.height,
        child: _buildDismissBarrier(),
      ),
      Positioned(
        left: visibleFieldRect.right,
        top: visibleFieldRect.top,
        right: 0,
        height: visibleFieldRect.height,
        child: _buildDismissBarrier(),
      ),
    ];
  }

  Widget _buildDismissBarrier() {
    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onTap: _closeMenu,
    );
  }

  Widget _buildOverlay(BuildContext context) {
    final colors = widget.colors;
    final fieldContext = _fieldKey.currentContext;
    final renderBox = fieldContext == null
        ? null
        : InputDecorator.containerOf(fieldContext);
    final overlayBox =
        Overlay.of(context).context.findRenderObject() as RenderBox?;
    final canMeasureField =
        renderBox != null &&
        renderBox.attached &&
        renderBox.hasSize &&
        overlayBox != null &&
        overlayBox.attached;
    final fieldTopLeft = canMeasureField
        ? overlayBox.globalToLocal(renderBox.localToGlobal(Offset.zero))
        : Offset.zero;
    final fieldTopRight = canMeasureField
        ? overlayBox.globalToLocal(
            renderBox.localToGlobal(Offset(renderBox.size.width, 0)),
          )
        : const Offset(280, 0);
    final fieldBottomLeft = canMeasureField
        ? overlayBox.globalToLocal(
            renderBox.localToGlobal(Offset(0, renderBox.size.height)),
          )
        : Offset(0, _effectiveSize.inputHeight);
    final fieldWidth = fieldTopRight.dx - fieldTopLeft.dx;
    final fieldTop = fieldTopLeft.dy;
    final fieldBottom = fieldBottomLeft.dy;
    final visibleItems = _visibleItems;
    final showCreateOption = _showCreateOption;
    final rows = <Widget>[
      if (showCreateOption)
        _WorkspaceSelectCreateRow(
          label: _createOptionLabel,
          size: _effectiveSize,
          colors: colors,
          onCreate: () {
            final query = _inputController.text.trim();
            _inputFocusNode.unfocus();
            _closeMenu();
            widget.onCreate!(query);
          },
        ),
      for (final item in visibleItems)
        _WorkspaceSelectMenuRow<T>(
          item: item,
          selected: item.value == widget.value,
          size: _effectiveSize,
          colors: colors,
          onSelected: () {
            widget.onChanged(item.value);
            if (widget.editable && widget.inputController == null) {
              _internalInputController.text = _plainTextFor(item.child) ?? '';
            }
            _closeMenu();
          },
        ),
      if (visibleItems.isEmpty && !showCreateOption)
        _WorkspaceSelectEmptyRow(size: _effectiveSize, colors: colors),
    ];
    final overlaySize = overlayBox?.size ?? MediaQuery.sizeOf(context);
    final fieldRect = Rect.fromLTRB(
      fieldTopLeft.dx,
      fieldTop,
      fieldTopRight.dx,
      fieldBottom,
    );
    final safePadding = MediaQuery.paddingOf(context);
    const menuGap = 4.0;
    const overlayMargin = 8.0;
    const menuVerticalPadding = _workspaceMenuVerticalInset * 2;
    const rowVerticalMargin = 2.0;
    const maxMenuHeight = 238.0;
    final rowCount = visibleItems.length + (showCreateOption ? 1 : 0);
    final contentHeight =
        math.max(1, rowCount) *
            (_effectiveSize.optionHeight + rowVerticalMargin) +
        menuVerticalPadding;
    final preferredHeight = math.min(maxMenuHeight, contentHeight);
    final availableBelow = math.max(
      0.0,
      overlaySize.height -
          safePadding.bottom -
          overlayMargin -
          fieldBottom -
          menuGap,
    );
    final availableAbove = math.max(
      0.0,
      fieldTop - safePadding.top - overlayMargin - menuGap,
    );
    final showAbove =
        availableBelow < preferredHeight && availableAbove > availableBelow;
    final availableHeight = showAbove ? availableAbove : availableBelow;
    final menuHeight = math.min(
      preferredHeight,
      math.max(_effectiveSize.optionHeight, availableHeight),
    );
    final needsScroll = contentHeight > menuHeight;
    final targetHeight = canMeasureField
        ? renderBox.size.height
        : _effectiveSize.inputHeight;

    return Stack(
      children: [
        ..._buildDismissBarriers(overlaySize, fieldRect),
        Positioned(
          left: 0,
          top: 0,
          width: fieldWidth,
          height: menuHeight,
          child: CompositedTransformFollower(
            link: _fieldLayerLink,
            showWhenUnlinked: false,
            targetAnchor: Alignment.topLeft,
            followerAnchor: Alignment.topLeft,
            offset: Offset(
              0,
              showAbove ? -menuHeight - menuGap : targetHeight + menuGap,
            ),
            child: TextFieldTapRegion(
              child: Material(
                key: ValueKey('workspace-select-menu:${widget.label}'),
                color: Colors.transparent,
                child: NautermDropdownSurface(
                  style: colors == null ? null : _terminalSftpMenuStyle(colors),
                  child: needsScroll
                      ? ListView(
                          padding: const EdgeInsets.symmetric(
                            vertical: _workspaceMenuVerticalInset,
                          ),
                          shrinkWrap: true,
                          children: rows,
                        )
                      : Padding(
                          padding: const EdgeInsets.symmetric(
                            vertical: _workspaceMenuVerticalInset,
                          ),
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            children: rows,
                          ),
                        ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final colors = widget.colors ?? _WorkspaceDialogThemeScope.maybeOf(context);
    final foreground = colors?.foreground ?? _text;
    final muted = colors?.muted ?? _mutedText;
    final selectedItem = widget.items
        .where((item) => item.value == widget.value)
        .firstOrNull;
    final selected = selectedItem?.child;
    final hasContent = widget.editable
        ? _inputController.text.isNotEmpty || widget.leading != null
        : selected != null;
    final showClear =
        widget.clearable &&
        hasContent &&
        (_hovered || _inputFocusNode.hasFocus);
    final fieldContent = widget.editable
        ? TextField(
            controller: _inputController,
            focusNode: _inputFocusNode,
            obscureText: widget.obscureText,
            onChanged: widget.onTextChanged,
            onTap: _focusEditableField,
            style: TextStyle(
              color: foreground,
              fontSize: _effectiveSize.inputFontSize,
              fontWeight: NautermFontWeights.regular,
              height: _effectiveSize.inputLineHeight,
              letterSpacing: 0,
            ),
            decoration: const InputDecoration.collapsed(hintText: null),
          )
        : DefaultTextStyle.merge(
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: selected == null ? muted : foreground,
              fontSize: _effectiveSize.inputFontSize,
              fontWeight: NautermFontWeights.regular,
              height: _effectiveSize.inputLineHeight,
              letterSpacing: 0,
            ),
            child: selected ?? const SizedBox.shrink(),
          );
    final field = CompositedTransformTarget(
      link: _fieldLayerLink,
      child: MouseRegion(
        onEnter: widget.clearable ? (_) => _handleHoverChanged(true) : null,
        onExit: widget.clearable ? (_) => _handleHoverChanged(false) : null,
        child: _WorkspaceFieldFrame(
          frameKey: _fieldKey,
          label: widget.label,
          isRequired: widget.isRequired,
          size: _effectiveSize,
          focused: _open || _inputFocusNode.hasFocus,
          floatingLabel: widget.floatingLabel,
          hasContent: hasContent,
          height: _effectiveSize.inputHeight,
          errorText: widget.errorText,
          onTap: widget.editable ? _focusEditableField : _toggleMenu,
          child: LayoutBuilder(
            builder: (context, constraints) {
              final showSuffix =
                  constraints.maxWidth >=
                  _workspaceFieldAffixWidth + _workspaceSelectAffixGap;
              return Row(
                children: [
                  if (widget.leading != null) ...[
                    ConstrainedBox(
                      constraints: BoxConstraints(
                        maxWidth: constraints.maxWidth * 0.68,
                      ),
                      child: widget.leading!,
                    ),
                    SizedBox(width: _workspaceSelectAffixGap),
                  ],
                  Expanded(child: fieldContent),
                  if (showSuffix) ...[
                    SizedBox(width: _workspaceSelectAffixGap),
                    SizedBox(
                      key: ValueKey('workspace-select-suffix:${widget.label}'),
                      width: _workspaceFieldAffixWidth,
                      height: _workspaceFieldAffixWidth,
                      child: showClear
                          ? _WorkspaceClearButton(
                              tooltip: tr('Clear ${widget.label}'),
                              onPressed: _clearInput,
                            )
                          : _WorkspaceSelectSuffixButton(
                              icon: _open
                                  ? Icons.keyboard_arrow_up_rounded
                                  : Icons.keyboard_arrow_down_rounded,
                              width: _workspaceFieldAffixWidth,
                              onPressed: widget.editable
                                  ? _toggleEditableMenu
                                  : _toggleMenu,
                              colors: colors,
                            ),
                    ),
                  ],
                ],
              );
            },
          ),
        ),
      ),
    );
    final themedField = widget.colors == null
        ? field
        : _WorkspaceDialogThemeScope(colors: widget.colors!, child: field);
    if (widget.editable) {
      return themedField;
    }
    return Focus(
      key: ValueKey('workspace-select-focus:${widget.label}'),
      focusNode: _inputFocusNode,
      child: themedField,
    );
  }
}

class _WorkspaceSelectSuffixButton extends StatelessWidget {
  const _WorkspaceSelectSuffixButton({
    required this.icon,
    required this.width,
    required this.onPressed,
    this.tooltip,
    this.colors,
  });

  final IconData icon;
  final double width;
  final VoidCallback? onPressed;
  final String? tooltip;
  final _AiAssistantColors? colors;

  @override
  Widget build(BuildContext context) {
    Widget button = MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: onPressed,
        child: SizedBox(
          width: width,
          height: double.infinity,
          child: Icon(
            icon,
            size: _workspaceFieldAffixWidth,
            color: colors?.muted ?? _mutedText,
          ),
        ),
      ),
    );
    if (tooltip != null) {
      button = Tooltip(
        message: tr(tooltip!),
        waitDuration: const Duration(milliseconds: 250),
        child: button,
      );
    }
    return TextFieldTapRegion(child: button);
  }
}

class _WorkspaceClearButton extends StatelessWidget {
  const _WorkspaceClearButton({required this.tooltip, required this.onPressed});

  final String tooltip;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final terminalColors = _WorkspaceDialogThemeScope.maybeOf(context);
    final idleBackground =
        terminalColors?.border.withValues(alpha: 0.72) ?? _workspaceMenuHover;
    final hoverBackground =
        terminalColors?.inputBackground ?? _workspaceMenuPressed;
    final pressedBackground = terminalColors?.border ?? _sidebarPressed;
    final foreground = terminalColors?.muted ?? _mutedText;
    return TextFieldTapRegion(
      child: Tooltip(
        message: tr(tooltip),
        waitDuration: const Duration(milliseconds: 250),
        child: MouseRegion(
          cursor: SystemMouseCursors.click,
          child: SizedBox.square(
            dimension: _workspaceFieldAffixWidth,
            child: Material(
              color: idleBackground,
              shape: const CircleBorder(),
              clipBehavior: Clip.antiAlias,
              child: InkWell(
                customBorder: const CircleBorder(),
                canRequestFocus: false,
                hoverColor: hoverBackground,
                highlightColor: pressedBackground,
                splashColor: pressedBackground,
                onTap: onPressed,
                child: Icon(Icons.clear_rounded, size: 10, color: foreground),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _WorkspaceToggleRow extends StatefulWidget {
  const _WorkspaceToggleRow({
    required this.label,
    this.description,
    required this.value,
    required this.onChanged,
  });

  final String label;
  final String? description;
  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  State<_WorkspaceToggleRow> createState() => _WorkspaceToggleRowState();
}

class _WorkspaceToggleRowState extends State<_WorkspaceToggleRow> {
  bool _pressed = false;
  bool _focused = false;

  void _toggle() => widget.onChanged(!widget.value);

  @override
  Widget build(BuildContext context) {
    final trackColor = widget.value ? _blue : _sidebarDivider;
    final description = widget.description;
    return Semantics(
      container: true,
      button: true,
      toggled: widget.value,
      label: tr(widget.label),
      onTap: _toggle,
      child: FocusableActionDetector(
        mouseCursor: SystemMouseCursors.click,
        onShowFocusHighlight: (focused) => setState(() => _focused = focused),
        shortcuts: const {
          SingleActivator(LogicalKeyboardKey.space): ActivateIntent(),
          SingleActivator(LogicalKeyboardKey.enter): ActivateIntent(),
        },
        actions: {
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (_) {
              _toggle();
              return null;
            },
          ),
        },
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: _toggle,
          onTapDown: (_) => setState(() => _pressed = true),
          onTapUp: (_) => setState(() => _pressed = false),
          onTapCancel: () => setState(() => _pressed = false),
          child: AnimatedContainer(
            key: ValueKey('workspace-toggle:${widget.label}'),
            duration: const Duration(milliseconds: 120),
            curve: Curves.easeOutCubic,
            constraints: BoxConstraints(
              minHeight: description == null ? 36 : 52,
            ),
            padding: const EdgeInsets.symmetric(horizontal: 8),
            decoration: BoxDecoration(
              color: Colors.transparent,
              borderRadius: BorderRadius.circular(8),
              border: _focused
                  ? Border.all(color: _blue.withValues(alpha: 0.52))
                  : null,
            ),
            child: Row(
              children: [
                Expanded(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        tr(widget.label),
                        style: TextStyle(
                          color: _text,
                          fontSize: NautermFontSizes.labelLarge,
                          fontWeight: NautermFontWeights.semibold,
                          letterSpacing: 0,
                        ),
                      ),
                      if (description != null) ...[
                        const SizedBox(height: 2),
                        Text(
                          tr(description),
                          style: TextStyle(
                            color: _mutedText,
                            fontSize: NautermFontSizes.labelSmall,
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
                AnimatedScale(
                  scale: _pressed ? 0.94 : 1,
                  duration: const Duration(milliseconds: 80),
                  child: AnimatedContainer(
                    duration: const Duration(milliseconds: 150),
                    curve: Curves.easeOutCubic,
                    width: 32,
                    height: 18,
                    padding: const EdgeInsets.all(2),
                    decoration: BoxDecoration(
                      color: trackColor,
                      borderRadius: BorderRadius.circular(9),
                    ),
                    child: AnimatedAlign(
                      duration: const Duration(milliseconds: 150),
                      curve: Curves.easeOutCubic,
                      alignment: widget.value
                          ? Alignment.centerRight
                          : Alignment.centerLeft,
                      child: Container(
                        key: ValueKey(
                          'workspace-toggle-thumb:${widget.label}:${widget.value ? 'on' : 'off'}',
                        ),
                        width: 14,
                        height: 14,
                        decoration: BoxDecoration(
                          color: _card,
                          shape: BoxShape.circle,
                          boxShadow: const [
                            BoxShadow(
                              color: Color(0x29000000),
                              blurRadius: 2,
                              offset: Offset(0, 1),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _WorkspaceFieldFrame extends StatelessWidget {
  const _WorkspaceFieldFrame({
    super.key,
    required this.label,
    this.isRequired = false,
    required this.size,
    required this.focused,
    required this.floatingLabel,
    required this.hasContent,
    required this.child,
    this.frameKey,
    this.height,
    this.textAlignVertical,
    this.alignLabelWithHint = false,
    this.contentPadding,
    this.onTap,
    this.errorText,
  });

  final String label;
  final bool isRequired;
  final _WorkspaceControlSize size;
  final bool focused;
  final bool floatingLabel;
  final bool hasContent;
  final Widget child;
  final Key? frameKey;
  final double? height;
  final TextAlignVertical? textAlignVertical;
  final bool alignLabelWithHint;
  final EdgeInsetsGeometry? contentPadding;
  final VoidCallback? onTap;
  final String? errorText;

  @override
  Widget build(BuildContext context) {
    final terminalColors = _WorkspaceDialogThemeScope.maybeOf(context);
    final muted = terminalColors?.muted ?? _mutedText;
    final fill = terminalColors?.inputBackground ?? _card;
    final borderColor = terminalColors?.border ?? _sidebarDivider;
    final focusColor = terminalColors?.accent ?? _blue;
    final error = errorText?.trim();
    final hasError = error?.isNotEmpty == true;
    final translatedLabel = tr(label);
    final displayLabel = isRequired ? '$translatedLabel *' : translatedLabel;
    const errorColor = Color(0xffe5453d);
    final fieldHeight = height ?? size.inputHeight;
    final frame = SizedBox(
      height: fieldHeight,
      child: InputDecorator(
        isFocused: focused,
        isEmpty: !hasContent,
        expands: true,
        textAlignVertical: textAlignVertical ?? TextAlignVertical.center,
        decoration: InputDecoration(
          labelText: floatingLabel ? displayLabel : null,
          hintText: floatingLabel ? null : displayLabel,
          alignLabelWithHint: alignLabelWithHint,
          labelStyle: TextStyle(
            color: hasError ? errorColor : muted,
            fontSize: size.inputFontSize,
            fontWeight: NautermFontWeights.medium,
            height: size.inputLineHeight,
            letterSpacing: 0,
          ),
          floatingLabelStyle: TextStyle(
            color: hasError ? errorColor : muted,
            fontSize: NautermFontSizes.labelMedium,
            fontWeight: NautermFontWeights.medium,
            letterSpacing: 0,
          ),
          hintStyle: TextStyle(
            color: muted,
            fontSize: size.inputFontSize,
            fontWeight: NautermFontWeights.medium,
            height: size.inputLineHeight,
            letterSpacing: 0,
          ),
          floatingLabelBehavior: floatingLabel
              ? FloatingLabelBehavior.auto
              : FloatingLabelBehavior.never,
          filled: true,
          fillColor: fill,
          isDense: true,
          constraints: BoxConstraints.tightFor(height: fieldHeight),
          contentPadding:
              contentPadding ??
              EdgeInsets.symmetric(
                horizontal: size.inputHorizontalPadding,
                vertical: size.inputVerticalPadding,
              ),
          border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(8),
            borderSide: BorderSide(color: borderColor),
          ),
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(8),
            borderSide: BorderSide(
              color: hasError ? errorColor : borderColor,
              width: hasError ? 1.2 : 1,
            ),
          ),
          focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(8),
            borderSide: BorderSide(
              color: hasError ? errorColor : focusColor.withValues(alpha: 0.68),
              width: 1.6,
            ),
          ),
        ),
        child: frameKey == null
            ? child
            : KeyedSubtree(key: frameKey, child: child),
      ),
    );

    final constrainedFrame = ConstrainedBox(
      constraints: BoxConstraints.tightFor(width: double.infinity),
      child: frame,
    );

    final framedChild = onTap == null
        ? constrainedFrame
        : MouseRegion(
            cursor: SystemMouseCursors.click,
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: onTap,
              child: constrainedFrame,
            ),
          );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        framedChild,
        if (hasError) ...[
          SizedBox(height: 4),
          Text(
            error!,
            key: ValueKey('workspace-field-error:$label'),
            style: const TextStyle(
              color: errorColor,
              fontSize: NautermFontSizes.labelSmall,
              fontWeight: NautermFontWeights.medium,
              height: 1.2,
              letterSpacing: 0,
            ),
          ),
        ],
      ],
    );
  }
}

class _WorkspaceMenuRowFrame extends StatefulWidget {
  const _WorkspaceMenuRowFrame({
    required this.child,
    this.enabled = true,
    this.selected = false,
    this.onTap,
    this.height,
    this.horizontalPadding,
    this.colors,
  });

  final Widget child;
  final bool enabled;
  final bool selected;
  final VoidCallback? onTap;
  final double? height;
  final double? horizontalPadding;
  final _AiAssistantColors? colors;

  @override
  State<_WorkspaceMenuRowFrame> createState() => _WorkspaceMenuRowFrameState();
}

class _WorkspaceMenuRowFrameState extends State<_WorkspaceMenuRowFrame> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final active = widget.enabled && (_hovered || widget.selected);
    final hover = widget.colors?.inputBackground ?? _workspaceMenuHover;
    final pressed = widget.colors?.border ?? _workspaceMenuPressed;
    return MouseRegion(
      onEnter: widget.enabled ? (_) => setState(() => _hovered = true) : null,
      onExit: widget.enabled ? (_) => setState(() => _hovered = false) : null,
      cursor: widget.enabled && widget.onTap != null
          ? SystemMouseCursors.click
          : MouseCursor.defer,
      child: Material(
        color: active ? hover : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          hoverColor: Colors.transparent,
          splashColor: pressed,
          highlightColor: pressed,
          onTap: widget.enabled ? widget.onTap : null,
          child: SizedBox(
            height: widget.height ?? _workspacePopupMenuRowHeight,
            child: Padding(
              padding: EdgeInsets.symmetric(
                horizontal: widget.horizontalPadding ?? 10,
              ),
              child: Align(
                alignment: Alignment.centerLeft,
                child: widget.child,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

String? _plainTextFor(Widget? widget) {
  if (widget is Text) {
    return widget.data;
  }
  if (widget is DefaultTextStyle) {
    return _plainTextFor(widget.child);
  }
  if (widget is Builder) {
    return null;
  }
  return null;
}

class _WorkspaceSelectMenuRow<T> extends StatelessWidget {
  const _WorkspaceSelectMenuRow({
    required this.item,
    required this.selected,
    required this.size,
    required this.onSelected,
    this.colors,
  });

  final DropdownMenuItem<T> item;
  final bool selected;
  final _WorkspaceControlSize size;
  final VoidCallback onSelected;
  final _AiAssistantColors? colors;

  @override
  Widget build(BuildContext context) {
    final itemChild = item.child;
    final localizedChild = itemChild is Text && itemChild.data != null
        ? Text(
            tr(itemChild.data!),
            key: itemChild.key,
            style: itemChild.style,
            strutStyle: itemChild.strutStyle,
            textAlign: itemChild.textAlign,
            textDirection: itemChild.textDirection,
            locale: itemChild.locale,
            softWrap: itemChild.softWrap,
            overflow: itemChild.overflow,
            textScaler: itemChild.textScaler,
            maxLines: itemChild.maxLines,
            semanticsLabel: itemChild.semanticsLabel,
            textWidthBasis: itemChild.textWidthBasis,
            textHeightBehavior: itemChild.textHeightBehavior,
            selectionColor: itemChild.selectionColor,
          )
        : itemChild;
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: _workspaceMenuHorizontalInset,
        vertical: 1,
      ),
      child: _WorkspaceMenuRowFrame(
        selected: selected,
        onTap: onSelected,
        height: size.optionHeight,
        horizontalPadding: _workspaceSelectOptionHorizontalPadding,
        colors: colors,
        child: DefaultTextStyle.merge(
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: TextStyle(
            color: colors?.foreground ?? _text,
            fontSize: _workspaceBaseControlFontSize,
            fontWeight: NautermFontWeights.regular,
            letterSpacing: 0,
          ),
          child: localizedChild,
        ),
      ),
    );
  }
}

class _WorkspaceSelectCreateRow extends StatelessWidget {
  const _WorkspaceSelectCreateRow({
    required this.label,
    required this.size,
    required this.onCreate,
    this.colors,
  });

  final String label;
  final _WorkspaceControlSize size;
  final VoidCallback onCreate;
  final _AiAssistantColors? colors;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: _workspaceMenuHorizontalInset,
        vertical: 1,
      ),
      child: _WorkspaceMenuRowFrame(
        onTap: onCreate,
        height: size.optionHeight,
        horizontalPadding: _workspaceSelectOptionHorizontalPadding,
        colors: colors,
        child: Row(
          children: [
            Icon(
              Icons.add_rounded,
              size: 16,
              color: colors?.muted ?? _mutedText,
            ),
            const SizedBox(width: 6),
            Expanded(
              child: Text(
                tr(label),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: colors?.foreground ?? _text,
                  fontSize: _workspaceBaseControlFontSize,
                  fontWeight: NautermFontWeights.regular,
                  letterSpacing: 0,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _WorkspaceSelectEmptyRow extends StatelessWidget {
  const _WorkspaceSelectEmptyRow({required this.size, this.colors});

  final _WorkspaceControlSize size;
  final _AiAssistantColors? colors;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: size.optionHeight,
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: _workspaceSelectOptionHorizontalPadding,
        ),
        child: Align(
          alignment: Alignment.centerLeft,
          child: Text(
            tr('workspace.label.noMatches', fallback: 'No matches'),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: colors?.muted ?? _mutedText,
              fontSize: _workspaceBaseControlFontSize,
              fontWeight: NautermFontWeights.regular,
              letterSpacing: 0,
            ),
          ),
        ),
      ),
    );
  }
}
