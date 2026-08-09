part of 'settings_panel.dart';

Widget _buildSettingsThemeSection(_SettingsPanelState state) {
  final theme = state._effectiveTheme;
  final allThemes = state._allThemes;
  final filtered = state._themeSearchQuery.isEmpty
      ? allThemes
      : allThemes?.where((t) {
          final q = state._themeSearchQuery.toLowerCase();
          return t.id.toLowerCase().contains(q) ||
              t.theme.name.toLowerCase().contains(q);
        }).toList();
  return SizedBox(
    height: 620,
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          flex: 1,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              TerminalThemePreviewCard(
                title: _settingsThemeDisplayName(state),
                theme: theme,
                compact: true,
              ),
              SizedBox(height: 14),
              Expanded(
                child: Container(
                  width: double.infinity,
                  decoration: BoxDecoration(
                    color: _surfaceContainer,
                    borderRadius: BorderRadius.circular(10),
                    border: Border.all(color: _softOutline),
                  ),
                  padding: const EdgeInsets.fromLTRB(14, 14, 14, 14),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      _colorSectionLabel('PRIMARY'),
                      SizedBox(height: 10),
                      Wrap(
                        spacing: 8,
                        runSpacing: 8,
                        children: [
                          _AnsiColorSwatch(
                            name: 'Accent',
                            color: theme.primary.accent,
                            width: 56,
                            onChanged: (c) =>
                                state._updateThemeColor('accent', c),
                          ),
                          _AnsiColorSwatch(
                            name: 'Foreground',
                            color: theme.primary.foreground,
                            width: 56,
                            onChanged: (c) =>
                                state._updateThemeColor('foreground', c),
                          ),
                          _AnsiColorSwatch(
                            name: 'Background',
                            color: theme.primary.background,
                            width: 56,
                            onChanged: (c) =>
                                state._updateThemeColor('background', c),
                          ),
                        ],
                      ),
                      SizedBox(height: 16),
                      Divider(height: 1, color: _softOutline),
                      SizedBox(height: 16),
                      _colorSectionLabel('CURSOR'),
                      SizedBox(height: 10),
                      Wrap(
                        spacing: 8,
                        runSpacing: 8,
                        children: [
                          _AnsiColorSwatch(
                            name: 'Color',
                            color: theme.cursor.cursor,
                            width: 56,
                            onChanged: (c) =>
                                state._updateThemeColor('cursor', c),
                          ),
                          _AnsiColorSwatch(
                            name: 'Text',
                            color: theme.cursor.text,
                            width: 56,
                            onChanged: (c) =>
                                state._updateThemeColor('cursorText', c),
                          ),
                        ],
                      ),
                      SizedBox(height: 16),
                      Divider(height: 1, color: _softOutline),
                      SizedBox(height: 16),
                      _colorSectionLabel('SELECTION'),
                      SizedBox(height: 10),
                      Wrap(
                        spacing: 8,
                        runSpacing: 8,
                        children: [
                          _AnsiColorSwatch(
                            name: 'Background',
                            color: theme.selection.background,
                            width: 56,
                            onChanged: (c) =>
                                state._updateThemeColor('selection', c),
                          ),
                          _AnsiColorSwatch(
                            name: 'Text',
                            color: theme.selection.text,
                            width: 56,
                            onChanged: (c) =>
                                state._updateThemeColor('selectionText', c),
                          ),
                        ],
                      ),
                      SizedBox(height: 16),
                      Divider(height: 1, color: _softOutline),
                      SizedBox(height: 16),
                      _colorSectionLabel('ANSI COLORS'),
                      SizedBox(height: 12),
                      _AnsiColorEditor(
                        label: 'Normal',
                        colors: theme.normal,
                        onChanged: (index, color) =>
                            state._updateAnsiColor(false, index, color),
                      ),
                      SizedBox(height: 14),
                      _AnsiColorEditor(
                        label: 'Bright',
                        colors: theme.bright,
                        onChanged: (index, color) =>
                            state._updateAnsiColor(true, index, color),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
        SizedBox(width: 24),
        Expanded(
          flex: 1,
          child: Container(
            decoration: BoxDecoration(
              color: _surfaceContainer,
              borderRadius: BorderRadius.circular(10),
              border: Border.all(color: _softOutline),
            ),
            padding: const EdgeInsets.all(10),
            child: Column(
              mainAxisSize: MainAxisSize.max,
              children: [
                TextField(
                  style: TextStyle(
                    color: _mutedText,
                    fontSize: 12,
                    fontWeight: NautermFontWeights.regular,
                  ),
                  decoration: InputDecoration(
                    hintText: tr(
                      'settings.search.themes.hint',
                      fallback: 'Search themes...',
                    ),
                    hintStyle: TextStyle(color: _faintText),
                    prefixIcon: Icon(Icons.search_rounded, size: 16),
                    prefixIconConstraints: const BoxConstraints(
                      minWidth: 31,
                      minHeight: 31,
                    ),
                    isDense: true,
                    filled: true,
                    fillColor: _surface,
                    contentPadding: const EdgeInsets.fromLTRB(0, 7, 8, 7),
                    enabledBorder: OutlineInputBorder(
                      borderSide: BorderSide(color: _softOutline),
                      borderRadius: BorderRadius.circular(7),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderSide: BorderSide(color: _primary),
                      borderRadius: BorderRadius.circular(7),
                    ),
                  ),
                  onChanged: (value) =>
                      state._mutate(() => state._themeSearchQuery = value),
                ),
                SizedBox(height: 10),
                if (filtered == null)
                  Expanded(
                    child: Center(
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  )
                else
                  Expanded(
                    child: _ThemeGrid(
                      themes: filtered,
                      selectedId: state._themeId,
                      onSelect: state._selectTheme,
                    ),
                  ),
              ],
            ),
          ),
        ),
      ],
    ),
  );
}

String _settingsThemeDisplayName(_SettingsPanelState state) {
  if (state._themeId == null) {
    return defaultTerminalTheme.name;
  }
  if (state._themeId == 'custom') {
    return 'Custom';
  }
  final match = state._allThemes
      ?.where((t) => t.id == state._themeId)
      .firstOrNull;
  return match?.theme.name ?? state._themeId ?? 'Default';
}

class _ApplicationThemeSelector extends StatelessWidget {
  const _ApplicationThemeSelector({
    required this.value,
    required this.onChanged,
  });

  final AppThemeMode value;
  final ValueChanged<AppThemeMode> onChanged;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          context.tr(
            'settings.general.applicationTheme.title',
            fallback: 'Application Theme',
          ),
          style: TextStyle(
            color: _text,
            fontSize: 14,
            height: 1.4,
            fontWeight: NautermFontWeights.medium,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 2),
        Text(
          context.tr(
            'settings.general.applicationTheme.description',
            fallback: 'Choose light, dark, or match your system.',
          ),
          style: TextStyle(
            color: _faintText,
            fontSize: 12,
            height: 1.35,
            fontWeight: NautermFontWeights.regular,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 14),
        Row(
          children: [
            for (
              var index = 0;
              index < AppThemeMode.values.length;
              index++
            ) ...[
              if (index > 0) SizedBox(width: 12),
              Expanded(
                child: _ApplicationThemeOption(
                  mode: AppThemeMode.values[index],
                  selected: value == AppThemeMode.values[index],
                  onTap: () => onChanged(AppThemeMode.values[index]),
                ),
              ),
            ],
          ],
        ),
      ],
    );
  }
}

class _ApplicationThemeOption extends StatelessWidget {
  const _ApplicationThemeOption({
    required this.mode,
    required this.selected,
    required this.onTap,
  });

  final AppThemeMode mode;
  final bool selected;
  final VoidCallback onTap;

  String _label(BuildContext context) {
    final (key, fallback) = switch (mode) {
      AppThemeMode.light => (
        'settings.general.applicationTheme.light',
        'Light',
      ),
      AppThemeMode.system => (
        'settings.general.applicationTheme.system',
        'System',
      ),
      AppThemeMode.dark => ('settings.general.applicationTheme.dark', 'Dark'),
    };
    return context.tr(key, fallback: fallback);
  }

  @override
  Widget build(BuildContext context) {
    return Semantics(
      key: ValueKey('application-theme-${mode.name}'),
      label: '${_label(context)} application theme',
      button: true,
      selected: selected,
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Column(
            children: [
              AspectRatio(
                aspectRatio: 1.72,
                child: AnimatedContainer(
                  duration: const Duration(milliseconds: 140),
                  decoration: BoxDecoration(
                    color: _settingsDark ? _surfaceContainer : Colors.white,
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(
                      color: selected ? _primary : _softOutline,
                      width: selected ? 2 : 1,
                    ),
                    boxShadow: const [
                      BoxShadow(
                        color: Color(0x0f111827),
                        blurRadius: 3,
                        offset: Offset(0, 1),
                      ),
                    ],
                  ),
                  child: Stack(
                    children: [
                      Positioned.fill(
                        child: Padding(
                          padding: EdgeInsets.all(selected ? 5 : 6),
                          child: ClipRRect(
                            borderRadius: BorderRadius.circular(4),
                            child: _ApplicationThemePreview(mode: mode),
                          ),
                        ),
                      ),
                      if (selected)
                        Positioned(
                          key: ValueKey(
                            'application-theme-selected-${mode.name}',
                          ),
                          top: 7,
                          right: 7,
                          child: Container(
                            width: 18,
                            height: 18,
                            decoration: BoxDecoration(
                              color: _settingsDark ? _surface : Colors.white,
                              shape: BoxShape.circle,
                              border: Border.all(color: _primary, width: 1.5),
                            ),
                            child: Icon(
                              Icons.check_rounded,
                              size: 13,
                              color: _primary,
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
              ),
              SizedBox(height: 7),
              Text(
                _label(context),
                style: TextStyle(
                  color: selected ? _primary : _text,
                  fontSize: 13,
                  fontWeight: selected
                      ? NautermFontWeights.semibold
                      : NautermFontWeights.medium,
                  letterSpacing: 0,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ApplicationThemePreview extends StatelessWidget {
  const _ApplicationThemePreview({required this.mode});

  final AppThemeMode mode;

  @override
  Widget build(BuildContext context) {
    final preview = switch (mode) {
      AppThemeMode.light => const _ApplicationThemePreviewSurface(),
      AppThemeMode.system => Row(
        children: [
          Expanded(child: _ApplicationThemePreviewSurface()),
          Expanded(child: _ApplicationThemePreviewSurface(dark: true)),
        ],
      ),
      AppThemeMode.dark => const _ApplicationThemePreviewSurface(dark: true),
    };
    final isSystem = mode == AppThemeMode.system;
    return LayoutBuilder(
      builder: (context, constraints) {
        final center = constraints.maxWidth / 2;
        return Stack(
          children: [
            Positioned.fill(child: preview),
            if (defaultTargetPlatform == TargetPlatform.macOS) ...[
              const Positioned(
                key: ValueKey('application-theme-traffic-lights'),
                top: 7,
                left: 7,
                child: _MacosTrafficLights(),
              ),
              if (isSystem)
                Positioned(
                  key: const ValueKey('application-theme-traffic-lights-right'),
                  top: 7,
                  left: center + 7,
                  child: const _MacosTrafficLights(),
                ),
            ],
          ],
        );
      },
    );
  }
}

class _MacosTrafficLights extends StatelessWidget {
  const _MacosTrafficLights();

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: const [
        _MacosTrafficLight(color: Color(0xffff5f57)),
        SizedBox(width: 3),
        _MacosTrafficLight(color: Color(0xfffebc2e)),
        SizedBox(width: 3),
        _MacosTrafficLight(color: Color(0xff28c840)),
      ],
    );
  }
}

class _MacosTrafficLight extends StatelessWidget {
  const _MacosTrafficLight({required this.color});

  final Color color;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: color,
        shape: BoxShape.circle,
        border: Border.all(color: Colors.black.withValues(alpha: 0.08)),
      ),
      child: const SizedBox.square(dimension: 6),
    );
  }
}

class _ApplicationThemePreviewSurface extends StatelessWidget {
  const _ApplicationThemePreviewSurface({this.dark = false});

  final bool dark;

  @override
  Widget build(BuildContext context) {
    final background = dark
        ? NautermPalette.dark.background
        : const Color(0xfffbfcfd);
    final sidebar = dark
        ? NautermPalette.dark.surface
        : const Color(0xffedf1f4);
    final line = dark ? NautermPalette.dark.outline : const Color(0xffe8edf1);
    final field = dark ? NautermPalette.dark.surfaceContainer : Colors.white;
    return ColoredBox(
      color: background,
      child: Padding(
        padding: EdgeInsets.fromLTRB(
          7,
          defaultTargetPlatform == TargetPlatform.macOS ? 18 : 7,
          7,
          7,
        ),
        child: Row(
          children: [
            Container(
              width: 14,
              decoration: BoxDecoration(
                color: sidebar,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
            SizedBox(width: 7),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  FractionallySizedBox(
                    widthFactor: 0.58,
                    child: Container(
                      height: 5,
                      decoration: BoxDecoration(
                        color: line,
                        borderRadius: BorderRadius.circular(2),
                      ),
                    ),
                  ),
                  SizedBox(height: 5),
                  FractionallySizedBox(
                    widthFactor: 0.82,
                    child: Container(
                      height: 4,
                      decoration: BoxDecoration(
                        color: line,
                        borderRadius: BorderRadius.circular(2),
                      ),
                    ),
                  ),
                  SizedBox(height: 7),
                  Row(
                    children: [
                      Expanded(
                        child: _ThemePreviewField(color: field, line: line),
                      ),
                      SizedBox(width: 5),
                      Expanded(
                        child: _ThemePreviewField(color: field, line: line),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ThemePreviewField extends StatelessWidget {
  const _ThemePreviewField({required this.color, required this.line});

  final Color color;
  final Color line;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 18,
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(2),
        border: Border.all(color: line),
      ),
    );
  }
}

class _ThemeGrid extends StatelessWidget {
  const _ThemeGrid({
    required this.themes,
    required this.selectedId,
    required this.onSelect,
  });

  final List<StoredTerminalTheme> themes;
  final String? selectedId;
  final ValueChanged<String?> onSelect;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final columns = constraints.maxWidth < 500 ? 2 : 3;
        return GridView.builder(
          gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
            crossAxisCount: columns,
            mainAxisSpacing: 10,
            crossAxisSpacing: 10,
            mainAxisExtent: 132,
          ),
          itemCount: themes.length,
          itemBuilder: (context, index) {
            final entry = themes[index];
            final isDefault = entry.id == nysaLightTerminalThemeId;
            return TerminalThemePreviewCard(
              title: entry.theme.name,
              theme: entry.theme,
              selected:
                  selectedId == entry.id || (selectedId == null && isDefault),
              onTap: () => onSelect(isDefault ? null : entry.id),
            );
          },
        );
      },
    );
  }
}

Widget _colorSectionLabel(String text) {
  return Text(
    text,
    style: TextStyle(
      color: const Color(0xff6b7280),
      fontSize: 11,
      height: 1.45,
      fontWeight: FontWeight.w700,
      letterSpacing: 0.8,
    ),
  );
}

const List<String> _ansiColorNames = [
  'Black',
  'Red',
  'Green',
  'Yellow',
  'Blue',
  'Magenta',
  'Cyan',
  'White',
];

class _AnsiColorEditor extends StatelessWidget {
  const _AnsiColorEditor({
    required this.label,
    required this.colors,
    required this.onChanged,
  });

  final String label;
  final TerminalAnsiColors colors;
  final void Function(int index, Color color) onChanged;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: TextStyle(
            color: _text,
            fontSize: 12,
            fontWeight: NautermFontWeights.medium,
          ),
        ),
        SizedBox(height: 8),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            for (var i = 0; i < 8; i++)
              Expanded(
                child: _AnsiColorSwatch(
                  name: _ansiColorNames[i],
                  color: colors.byIndex(i),
                  onChanged: (c) => onChanged(i, c),
                ),
              ),
          ],
        ),
      ],
    );
  }
}

class _AnsiColorSwatch extends StatefulWidget {
  const _AnsiColorSwatch({
    required this.name,
    required this.color,
    required this.onChanged,
    this.width,
  });

  final String name;
  final Color color;
  final ValueChanged<Color> onChanged;
  final double? width;

  @override
  State<_AnsiColorSwatch> createState() => _AnsiColorSwatchState();
}

class _AnsiColorSwatchState extends State<_AnsiColorSwatch> {
  void _openPicker(TapDownDetails details) {
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    final position = overlay.globalToLocal(details.globalPosition);

    late NautermTransientOverlayHandle handle;
    handle = showNautermTransientOverlay(
      context: context,
      token: Object(),
      dismissExisting: true,
      builder: (ctx) {
        return _ColorPickerOverlay(
          initialColor: widget.color,
          position: position,
          onSelected: (color) {
            widget.onChanged(color);
          },
          onDismiss: () => handle.dismiss(),
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: widget.width,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        mainAxisSize: MainAxisSize.min,
        children: [
          FittedBox(
            fit: BoxFit.scaleDown,
            child: Text(
              widget.name,
              textAlign: TextAlign.center,
              maxLines: 1,
              style: TextStyle(
                color: _mutedText,
                fontSize: 9,
                fontWeight: NautermFontWeights.medium,
              ),
            ),
          ),
          SizedBox(height: 3),
          InkWell(
            borderRadius: BorderRadius.circular(11),
            onTapDown: _openPicker,
            child: Container(
              width: 22,
              height: 22,
              decoration: BoxDecoration(
                color: widget.color,
                shape: BoxShape.circle,
                border: Border.all(color: _softOutline, width: 1.5),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _ColorPickerOverlay extends StatefulWidget {
  const _ColorPickerOverlay({
    required this.initialColor,
    required this.position,
    required this.onSelected,
    required this.onDismiss,
  });

  final Color initialColor;
  final Offset position;
  final ValueChanged<Color> onSelected;
  final VoidCallback onDismiss;

  @override
  State<_ColorPickerOverlay> createState() => _ColorPickerOverlayState();
}

class _ColorPickerOverlayState extends State<_ColorPickerOverlay> {
  late HSVColor _hsv;
  late final TextEditingController _hexController;
  final _pickerKey = GlobalKey();

  @override
  void initState() {
    super.initState();
    _hsv = HSVColor.fromColor(widget.initialColor);
    _hexController = TextEditingController(text: _hexStr());
    GestureBinding.instance.pointerRouter.addGlobalRoute(_handleGlobalPointer);
  }

  @override
  void dispose() {
    GestureBinding.instance.pointerRouter.removeGlobalRoute(
      _handleGlobalPointer,
    );
    _hexController.dispose();
    super.dispose();
  }

  void _handleGlobalPointer(PointerEvent event) {
    if (event is! PointerDownEvent) {
      return;
    }
    final box = _pickerKey.currentContext?.findRenderObject() as RenderBox?;
    if (box == null || !box.attached) {
      return;
    }
    final rect = box.localToGlobal(Offset.zero) & box.size;
    if (rect.contains(event.position)) {
      return;
    }
    _dismiss();
  }

  Color get _color => _hsv.toColor();

  String _hexStr() => _color.toHex().replaceFirst('#', '').toUpperCase();

  void _syncHex() => _hexController.text = _hexStr();

  void _applyColor() {
    widget.onSelected(_color);
  }

  void _dismiss() {
    if (_color != widget.initialColor) {
      _applyColor();
    }
    widget.onDismiss();
  }

  @override
  Widget build(BuildContext context) {
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    final left = widget.position.dx.clamp(8.0, overlay.size.width - 220.0);
    final top = widget.position.dy.clamp(8.0, overlay.size.height - 220.0);
    return Stack(
      children: [
        Positioned(
          left: left,
          top: top,
          child: KeyboardListener(
            focusNode: FocusNode()..requestFocus(),
            onKeyEvent: (e) {
              if (e is KeyDownEvent &&
                  e.logicalKey == LogicalKeyboardKey.escape) {
                _dismiss();
              }
            },
            child: GestureDetector(
              onTap: () {},
              child: Material(
                key: _pickerKey,
                elevation: 8,
                shadowColor: const Color(0x26000000),
                borderRadius: BorderRadius.circular(8),
                color: _surfaceContainer,
                child: Container(
                  width: 200,
                  padding: const EdgeInsets.all(10),
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(color: _softOutline),
                  ),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      ClipRRect(
                        borderRadius: BorderRadius.circular(5),
                        child: SizedBox(
                          height: 120,
                          child: _SBPicker(
                            hue: _hsv.hue,
                            sat: _hsv.saturation,
                            val: _hsv.value,
                            onChanged: (s, v) {
                              _hsv = _hsv.withSaturation(s).withValue(v);
                              _syncHex();
                              _applyColor();
                              setState(() {});
                            },
                          ),
                        ),
                      ),
                      SizedBox(height: 8),
                      ClipRRect(
                        borderRadius: BorderRadius.circular(5),
                        child: SizedBox(
                          height: 14,
                          child: _HueBar(
                            hue: _hsv.hue,
                            onChanged: (h) {
                              _hsv = _hsv.withHue(h);
                              _syncHex();
                              _applyColor();
                              setState(() {});
                            },
                          ),
                        ),
                      ),
                      SizedBox(height: 8),
                      Row(
                        children: [
                          Container(
                            width: 20,
                            height: 20,
                            decoration: BoxDecoration(
                              color: _color,
                              borderRadius: BorderRadius.circular(4),
                              border: Border.all(color: _softOutline),
                            ),
                          ),
                          SizedBox(width: 6),
                          Expanded(
                            child: Container(
                              height: 20,
                              padding: const EdgeInsets.symmetric(
                                horizontal: 5,
                              ),
                              decoration: BoxDecoration(
                                color: Colors.white,
                                borderRadius: BorderRadius.circular(4),
                                border: Border.all(color: _softOutline),
                              ),
                              child: Row(
                                children: [
                                  Text(
                                    tr('#'),
                                    style: TextStyle(
                                      color: _mutedText,
                                      fontSize: 10,
                                      fontFamily: 'monospace',
                                    ),
                                  ),
                                  SizedBox(width: 2),
                                  Expanded(
                                    child: TextField(
                                      controller: _hexController,
                                      style: TextStyle(
                                        color: _text,
                                        fontSize: 10,
                                        fontFamily: 'monospace',
                                      ),
                                      decoration: InputDecoration(
                                        isCollapsed: true,
                                        contentPadding: EdgeInsets.zero,
                                        border: InputBorder.none,
                                      ),
                                      onChanged: (text) {
                                        final hex = text.trim();
                                        if (hex.length == 6) {
                                          _hsv = HSVColor.fromColor(
                                            Color(
                                              int.parse('ff$hex', radix: 16),
                                            ),
                                          );
                                          _applyColor();
                                          setState(() {});
                                        }
                                      },
                                    ),
                                  ),
                                ],
                              ),
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _SBPicker extends StatelessWidget {
  const _SBPicker({
    required this.hue,
    required this.sat,
    required this.val,
    required this.onChanged,
  });

  final double hue;
  final double sat;
  final double val;
  final void Function(double s, double v) onChanged;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, c) {
        return GestureDetector(
          onPanStart: (d) => _update(d.localPosition, c),
          onPanUpdate: (d) => _update(d.localPosition, c),
          onTapDown: (d) => _update(d.localPosition, c),
          child: CustomPaint(
            size: Size(c.maxWidth, c.maxHeight),
            painter: _SBPainter(hue: hue, sat: sat, val: val),
          ),
        );
      },
    );
  }

  void _update(Offset p, BoxConstraints c) {
    onChanged(
      (p.dx / c.maxWidth).clamp(0.0, 1.0),
      (1.0 - p.dy / c.maxHeight).clamp(0.0, 1.0),
    );
  }
}

class _SBPainter extends CustomPainter {
  _SBPainter({required this.hue, required this.sat, required this.val});

  final double hue;
  final double sat;
  final double val;

  @override
  void paint(Canvas canvas, Size size) {
    for (var x = 0.0; x < size.width; x += 2) {
      for (var y = 0.0; y < size.height; y += 2) {
        canvas.drawRect(
          Rect.fromLTWH(x, y, 2, 2),
          Paint()
            ..color = HSVColor.fromAHSV(
              1,
              hue,
              (x / size.width).clamp(0, 1),
              (1 - y / size.height).clamp(0, 1),
            ).toColor(),
        );
      }
    }
    final cx = sat * size.width;
    final cy = (1 - val) * size.height;
    canvas.drawCircle(
      Offset(cx, cy),
      8,
      Paint()
        ..color = Colors.white
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2,
    );
    canvas.drawCircle(
      Offset(cx, cy),
      7,
      Paint()
        ..color = Colors.black
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1,
    );
  }

  @override
  bool shouldRepaint(covariant _SBPainter o) =>
      o.hue != hue || o.sat != sat || o.val != val;
}

class _HueBar extends StatelessWidget {
  const _HueBar({required this.hue, required this.onChanged});

  final double hue;
  final ValueChanged<double> onChanged;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, c) {
        return GestureDetector(
          onPanStart: (d) => _update(d.localPosition.dx, c.maxWidth),
          onPanUpdate: (d) => _update(d.localPosition.dx, c.maxWidth),
          onTapDown: (d) => _update(d.localPosition.dx, c.maxWidth),
          child: CustomPaint(
            size: Size(c.maxWidth, c.maxHeight),
            painter: _HuePainter(hue: hue),
          ),
        );
      },
    );
  }

  void _update(double x, double w) => onChanged((x / w * 360).clamp(0, 360));
}

class _HuePainter extends CustomPainter {
  _HuePainter({required this.hue});

  final double hue;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    canvas.drawRect(
      rect,
      Paint()
        ..shader = LinearGradient(
          colors: [
            for (var h = 0; h <= 360; h += 30)
              HSVColor.fromAHSV(1, h.toDouble(), 1, 1).toColor(),
          ],
        ).createShader(rect),
    );
    final cx = hue / 360 * size.width;
    canvas.drawRect(
      Rect.fromCenter(
        center: Offset(cx, size.height / 2),
        width: 4,
        height: size.height,
      ),
      Paint()
        ..color = Colors.white
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2,
    );
  }

  @override
  bool shouldRepaint(covariant _HuePainter o) => o.hue != hue;
}

class _TerminalPaddingControl extends StatefulWidget {
  const _TerminalPaddingControl({
    required this.padding,
    required this.onChanged,
  });

  final EdgeInsets padding;
  final ValueChanged<EdgeInsets> onChanged;

  @override
  State<_TerminalPaddingControl> createState() =>
      _TerminalPaddingControlState();
}

class _TerminalPaddingControlState extends State<_TerminalPaddingControl> {
  late final TextEditingController _controller;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(text: _formatPadding(widget.padding));
  }

  @override
  void didUpdateWidget(covariant _TerminalPaddingControl oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.padding != widget.padding) {
      final formatted = _formatPadding(widget.padding);
      if (_controller.text != formatted) {
        _controller.text = formatted;
      }
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  static String _formatPadding(EdgeInsets p) {
    if (p.left == p.top && p.top == p.right && p.right == p.bottom) {
      return '${p.left.round()}';
    }
    if (p.top == p.bottom && p.left == p.right) {
      return '${p.top.round()}, ${p.left.round()}';
    }
    if (p.left == p.right) {
      return '${p.top.round()}, ${p.right.round()}, ${p.bottom.round()}';
    }
    return '${p.top.round()}, ${p.right.round()}, ${p.bottom.round()}, ${p.left.round()}';
  }

  static EdgeInsets? _parsePadding(String text) {
    final parts = text
        .split(',')
        .map((s) => double.tryParse(s.trim()))
        .toList();
    if (parts.any((p) => p == null || p < 0)) {
      return null;
    }
    return switch (parts) {
      [final a] when a != null => EdgeInsets.all(a),
      [final a, final b] when a != null && b != null => EdgeInsets.symmetric(
        vertical: a,
        horizontal: b,
      ),
      [final t, final h, final b] when t != null && h != null && b != null =>
        EdgeInsets.fromLTRB(h, t, h, b),
      [final t, final r, final b, final l]
          when t != null && r != null && b != null && l != null =>
        EdgeInsets.fromLTRB(l, t, r, b),
      _ => null,
    };
  }

  void _handleSubmitted(String text) {
    final parsed = _parsePadding(text);
    if (parsed != null) {
      widget.onChanged(parsed);
    } else {
      _controller.text = _formatPadding(widget.padding);
    }
  }

  @override
  Widget build(BuildContext context) {
    return _SettingsTextField(
      controller: _controller,
      hint: 'all · top/bottom left/right · top right bottom left',
      onChanged: (_) {},
      onSubmitted: _handleSubmitted,
      onFocusLost: _handleSubmitted,
    );
  }
}
