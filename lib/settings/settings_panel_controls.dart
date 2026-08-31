part of 'settings_panel.dart';

class _SettingsSidebar extends StatelessWidget {
  const _SettingsSidebar({
    required this.selectedPage,
    required this.compact,
    required this.onPageSelected,
  });

  final _SettingsPage selectedPage;
  final bool compact;
  final ValueChanged<_SettingsPage> onPageSelected;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: compact ? 68 : 200,
      decoration: BoxDecoration(
        color: _surfaceContainer,
        border: Border(right: BorderSide(color: _softOutline)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: EdgeInsets.fromLTRB(
              compact ? 14 : 20,
              24,
              compact ? 14 : 20,
              19,
            ),
            child: Row(
              mainAxisAlignment: compact
                  ? MainAxisAlignment.center
                  : MainAxisAlignment.start,
              children: [
                Container(
                  width: 24,
                  height: 24,
                  decoration: BoxDecoration(
                    color: _primary,
                    borderRadius: BorderRadius.circular(6),
                  ),
                  child: Icon(
                    LucideIcons.squareTerminal,
                    color: Colors.white,
                    size: 15,
                  ),
                ),
                if (!compact) ...[
                  SizedBox(width: 9),
                  Text(
                    'Nauterm',
                    style: TextStyle(
                      color: _text,
                      fontSize: 14,
                      fontWeight: FontWeight.w700,
                      letterSpacing: 0,
                    ),
                  ),
                ],
              ],
            ),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8),
            child: Column(
              children: [
                _SettingsNavItem(
                  key: const ValueKey('settings-nav-general'),
                  icon: Icons.tune_rounded,
                  label: context.tr(
                    'settings.sidebar.general.label',
                    fallback: 'General',
                  ),
                  compact: compact,
                  selected: selectedPage == _SettingsPage.general,
                  onTap: () => onPageSelected(_SettingsPage.general),
                ),
                _SettingsNavItem(
                  key: const ValueKey('settings-nav-terminal'),
                  icon: LucideIcons.squareTerminal,
                  label: context.tr(
                    'settings.sidebar.terminal.label',
                    fallback: 'Terminal',
                  ),
                  compact: compact,
                  selected: selectedPage == _SettingsPage.terminal,
                  onTap: () => onPageSelected(_SettingsPage.terminal),
                ),
                _SettingsNavItem(
                  key: const ValueKey('settings-nav-sftp'),
                  icon: LucideIcons.folderSync,
                  label: context.tr(
                    'settings.sidebar.sftp.label',
                    fallback: 'SFTP',
                  ),
                  compact: compact,
                  selected: selectedPage == _SettingsPage.sftp,
                  onTap: () => onPageSelected(_SettingsPage.sftp),
                ),
                _SettingsNavItem(
                  key: const ValueKey('settings-nav-ai'),
                  icon: LucideIcons.sparkles,
                  label: context.tr(
                    'settings.sidebar.aiAssistant.label',
                    fallback: 'AI Assistant',
                  ),
                  compact: compact,
                  selected: selectedPage == _SettingsPage.ai,
                  onTap: () => onPageSelected(_SettingsPage.ai),
                ),
                _SettingsNavItem(
                  key: const ValueKey('settings-nav-sync'),
                  icon: LucideIcons.refreshCw,
                  label: context.tr(
                    'settings.sidebar.syncBackup.label',
                    fallback: 'Sync & Backup',
                  ),
                  compact: compact,
                  selected: selectedPage == _SettingsPage.sync,
                  onTap: () => onPageSelected(_SettingsPage.sync),
                ),
                _SettingsNavItem(
                  key: const ValueKey('settings-nav-shortcuts'),
                  icon: LucideIcons.keyboard,
                  label: context.tr(
                    'settings.sidebar.shortcuts.label',
                    fallback: 'Shortcuts',
                  ),
                  compact: compact,
                  selected: selectedPage == _SettingsPage.shortcuts,
                  onTap: () => onPageSelected(_SettingsPage.shortcuts),
                ),
                _SettingsNavItem(
                  key: const ValueKey('settings-nav-about'),
                  icon: LucideIcons.info,
                  label: context.tr(
                    'settings.sidebar.about.label',
                    fallback: 'About',
                  ),
                  compact: compact,
                  selected: selectedPage == _SettingsPage.about,
                  onTap: () => onPageSelected(_SettingsPage.about),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _SettingsNavItem extends StatelessWidget {
  const _SettingsNavItem({
    super.key,
    required this.icon,
    required this.label,
    required this.onTap,
    required this.compact,
    this.selected = false,
  });

  final IconData icon;
  final String label;
  final VoidCallback onTap;
  final bool compact;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final item = Padding(
      padding: const EdgeInsets.only(bottom: 3),
      child: Material(
        color: selected ? _surface : Colors.transparent,
        borderRadius: BorderRadius.circular(7),
        elevation: selected ? 1 : 0,
        shadowColor: Colors.black.withValues(alpha: 0.09),
        child: InkWell(
          borderRadius: BorderRadius.circular(7),
          hoverColor: _settingsDark
              ? _settingsFieldHover
              : const Color(0xffe5e7eb),
          onTap: onTap,
          child: Container(
            height: 31,
            padding: EdgeInsets.symmetric(horizontal: compact ? 0 : 12),
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(7),
              border: selected ? Border.all(color: _softOutline) : null,
            ),
            child: Row(
              mainAxisAlignment: compact
                  ? MainAxisAlignment.center
                  : MainAxisAlignment.start,
              children: [
                Icon(
                  icon,
                  size: 18,
                  color: selected ? _primary : const Color(0xff9ca3af),
                ),
                if (!compact) ...[
                  SizedBox(width: 11),
                  Expanded(
                    child: Text(
                      label,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: selected ? _primary : _mutedText,
                        fontSize: 13,
                        fontWeight: selected
                            ? NautermFontWeights.semibold
                            : NautermFontWeights.medium,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
    if (!compact) {
      return item;
    }
    return Tooltip(message: label, child: item);
  }
}

class _SettingsHeader extends StatelessWidget {
  const _SettingsHeader({
    required this.searchController,
    required this.onSearchChanged,
    required this.onResetPage,
    required this.onResetAll,
    required this.onDone,
  });

  final TextEditingController searchController;
  final ValueChanged<String> onSearchChanged;
  final VoidCallback? onResetPage;
  final VoidCallback onResetAll;
  final VoidCallback onDone;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final showSearch = constraints.maxWidth >= 420;
        final searchWidth = math.min(
          192.0,
          math.max(132.0, constraints.maxWidth * 0.34),
        );
        return Container(
          height: 56,
          padding: EdgeInsets.symmetric(
            horizontal: constraints.maxWidth < 520 ? 16 : 24,
          ),
          decoration: BoxDecoration(
            color: _surface,
            border: Border(bottom: BorderSide(color: _softOutline)),
          ),
          child: Row(
            children: [
              Text(
                tr('common.label.settings', fallback: 'Settings'),
                style: TextStyle(
                  color: _text,
                  fontSize: 14,
                  fontWeight: NautermFontWeights.semibold,
                  letterSpacing: 0,
                ),
              ),
              const Spacer(),
              if (showSearch) ...[
                _SearchField(
                  width: searchWidth,
                  controller: searchController,
                  onChanged: onSearchChanged,
                ),
                SizedBox(width: 12),
              ],
              _ResetSettingsButton(
                onResetPage: onResetPage,
                onResetAll: onResetAll,
              ),
              SizedBox(width: 8),
              _PrimaryButton(label: 'Done', onTap: onDone),
            ],
          ),
        );
      },
    );
  }
}

class _SearchField extends StatelessWidget {
  const _SearchField({
    required this.width,
    required this.controller,
    required this.onChanged,
  });

  final double width;
  final TextEditingController controller;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: width,
      height: 31,
      child: TextField(
        key: const ValueKey('settings-search-field'),
        controller: controller,
        onChanged: onChanged,
        style: TextStyle(
          color: _mutedText,
          fontSize: 12,
          fontWeight: NautermFontWeights.regular,
          letterSpacing: 0,
        ),
        decoration: InputDecoration(
          hintText: tr(
            'settings.search.settings.hint',
            fallback: 'Search settings...',
          ),
          hintStyle: TextStyle(color: _faintText),
          prefixIcon: Icon(Icons.search_rounded, size: 16),
          prefixIconConstraints: const BoxConstraints(
            minWidth: 31,
            minHeight: 31,
          ),
          isDense: true,
          filled: true,
          fillColor: _surfaceContainer,
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
      ),
    );
  }
}

class _ResetSettingsButton extends StatefulWidget {
  const _ResetSettingsButton({
    required this.onResetPage,
    required this.onResetAll,
  });

  final VoidCallback? onResetPage;
  final VoidCallback onResetAll;

  @override
  State<_ResetSettingsButton> createState() => _ResetSettingsButtonState();
}

class _ResetSettingsButtonState extends State<_ResetSettingsButton> {
  final GlobalKey _buttonKey = GlobalKey();
  OverlayEntry? _overlayEntry;
  ScrollController? _scrollController;

  bool get _open => _overlayEntry != null;

  List<String> get _values => [if (widget.onResetPage != null) 'page', 'all'];

  @override
  void dispose() {
    _removeMenu();
    super.dispose();
  }

  void _toggleMenu() => _open ? _closeMenu() : _openMenu();

  void _openMenu() {
    final buttonContext = _buttonKey.currentContext;
    final overlay = Overlay.of(context);
    final buttonBox = buttonContext?.findRenderObject();
    final overlayBox = overlay.context.findRenderObject();
    if (_open || buttonBox is! RenderBox || overlayBox is! RenderBox) return;

    const menuWidth = 210.0;
    final values = _values;
    final menuHeight = values.length * _settingsSelectHeight;
    final buttonOrigin = buttonBox.localToGlobal(
      Offset.zero,
      ancestor: overlayBox,
    );
    final menuLeft = (buttonOrigin.dx + buttonBox.size.width - menuWidth)
        .clamp(8.0, math.max(8.0, overlayBox.size.width - menuWidth - 8))
        .toDouble();
    final menuTop = buttonOrigin.dy + buttonBox.size.height + 4;
    _scrollController = ScrollController();
    _overlayEntry = OverlayEntry(
      builder: (overlayContext) => Stack(
        children: [
          Positioned.fill(
            child: GestureDetector(
              behavior: HitTestBehavior.translucent,
              onTap: _closeMenu,
              child: const SizedBox.expand(),
            ),
          ),
          Positioned(
            left: menuLeft,
            top: menuTop,
            width: menuWidth,
            height: menuHeight,
            child: _SettingsSelectMenu(
              values: values,
              selectedValue: '',
              highlightedIndex: -1,
              format: (value) => switch (value) {
                'page' => 'Reset Current Page',
                _ => 'Reset All Settings',
              },
              leadingBuilder: (value) => Icon(
                value == 'all' ? LucideIcons.refreshCcw : LucideIcons.rotateCcw,
                size: 14,
                color: _primary,
              ),
              scrollController: _scrollController!,
              onSelected: _selectValue,
            ),
          ),
        ],
      ),
    );
    overlay.insert(_overlayEntry!);
    setState(() {});
  }

  void _selectValue(String value) {
    _closeMenu();
    if (value == 'page') {
      widget.onResetPage?.call();
    } else {
      widget.onResetAll();
    }
  }

  void _closeMenu() {
    if (!_open) return;
    _removeMenu();
    if (mounted) setState(() {});
  }

  void _removeMenu() {
    final entry = _overlayEntry;
    _overlayEntry = null;
    if (entry?.mounted ?? false) entry!.remove();
    _scrollController?.dispose();
    _scrollController = null;
  }

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tr('settings.label.resetSettings', fallback: 'Reset settings'),
      child: Semantics(
        button: true,
        label: 'Reset settings',
        child: Material(
          color: Colors.transparent,
          borderRadius: BorderRadius.circular(7),
          child: InkWell(
            key: const ValueKey('settings-reset-menu'),
            onTap: _toggleMenu,
            borderRadius: BorderRadius.circular(7),
            child: Container(
              key: _buttonKey,
              height: 29,
              padding: const EdgeInsets.symmetric(horizontal: 10),
              decoration: BoxDecoration(
                color: _surfaceContainer,
                borderRadius: BorderRadius.circular(7),
                border: Border.all(color: _softOutline),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(LucideIcons.rotateCcw, size: 14, color: _mutedText),
                  SizedBox(width: 6),
                  Text(
                    tr('common.action.reset', fallback: 'Reset'),
                    style: TextStyle(
                      color: _mutedText,
                      fontSize: 12,
                      fontWeight: NautermFontWeights.medium,
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

Widget _buildSettingsSearchResults(_SettingsPanelState state) {
  final matches = _settingsSearchEntries
      .where((entry) => entry.matches(state._settingsSearchQuery))
      .toList(growable: false);
  return ListView(
    padding: state._contentPadding,
    children: [
      Text(
        tr('settings.label.searchSettings', fallback: 'Search Settings'),
        style: TextStyle(
          color: _text,
          fontSize: 20,
          height: 1.4,
          fontWeight: NautermFontWeights.semibold,
        ),
      ),
      SizedBox(height: 3),
      Text(
        matches.isEmpty
            ? 'No settings match “${state._settingsSearchQuery}”.'
            : '${matches.length} matching settings',
        style: TextStyle(color: _mutedText, fontSize: 12),
      ),
      SizedBox(height: 22),
      for (final entry in matches)
        Padding(
          padding: const EdgeInsets.only(bottom: 8),
          child: Material(
            color: _surfaceContainer,
            borderRadius: BorderRadius.circular(8),
            child: InkWell(
              key: ValueKey(
                'settings-search-result-${entry.page.name}-${entry.title}',
              ),
              borderRadius: BorderRadius.circular(8),
              onTap: () => state._openSearchResult(entry),
              child: Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: 14,
                  vertical: 11,
                ),
                decoration: BoxDecoration(
                  border: Border.all(color: _softOutline),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Row(
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            tr(entry.title),
                            style: TextStyle(
                              color: _text,
                              fontSize: 13,
                              fontWeight: NautermFontWeights.medium,
                            ),
                          ),
                          SizedBox(height: 2),
                          Text(
                            tr(entry.subtitle),
                            style: TextStyle(color: _faintText, fontSize: 12),
                          ),
                        ],
                      ),
                    ),
                    SizedBox(width: 12),
                    Text(
                      _settingsPageLabel(state.context, entry.page),
                      style: TextStyle(color: _primary, fontSize: 11),
                    ),
                    SizedBox(width: 5),
                    Icon(LucideIcons.chevronRight, size: 15, color: _primary),
                  ],
                ),
              ),
            ),
          ),
        ),
    ],
  );
}

String _settingsPageLabel(BuildContext context, _SettingsPage page) =>
    context.tr(switch (page) {
      _SettingsPage.general => 'General',
      _SettingsPage.terminal => 'Terminal',
      _SettingsPage.sftp => 'SFTP',
      _SettingsPage.ai => 'AI Assistant',
      _SettingsPage.sync => 'Sync & Backup',
      _SettingsPage.shortcuts => 'Shortcuts',
      _SettingsPage.about => 'About',
    });

class _PrimaryButton extends StatelessWidget {
  const _PrimaryButton({required this.label, required this.onTap});

  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 29,
      child: FilledButton(
        style: FilledButton.styleFrom(
          backgroundColor: _primary,
          foregroundColor: Colors.white,
          padding: const EdgeInsets.symmetric(horizontal: 13),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(7)),
          textStyle: TextStyle(
            fontSize: 12,
            fontWeight: NautermFontWeights.medium,
            letterSpacing: 0,
          ),
        ),
        onPressed: onTap,
        child: Text(tr(label)),
      ),
    );
  }
}

class _SettingsSection extends StatelessWidget {
  const _SettingsSection({
    super.key,
    required this.icon,
    required this.title,
    required this.child,
    this.trailing,
    this.localizationKey,
  });

  final IconData icon;
  final String title;
  final Widget child;
  final Widget? trailing;
  final String? localizationKey;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          width: double.infinity,
          padding: const EdgeInsets.only(bottom: 9),
          decoration: BoxDecoration(
            border: Border(bottom: BorderSide(color: _softOutline)),
          ),
          child: Align(
            alignment: Alignment.centerLeft,
            child: ConstrainedBox(
              constraints: const BoxConstraints(
                maxWidth: _settingsContentMaxWidth,
              ),
              child: Row(
                children: [
                  Icon(icon, color: _primary, size: 20),
                  SizedBox(width: 8),
                  Text(
                    (localizationKey == null
                            ? tr(title)
                            : tr(localizationKey!, fallback: title))
                        .toUpperCase(),
                    style: TextStyle(
                      color: const Color(0xff6b7280),
                      fontSize: 11,
                      height: 1.45,
                      fontWeight: FontWeight.w700,
                      letterSpacing: 0.8,
                    ),
                  ),
                  if (trailing != null) ...[Spacer(), trailing!],
                ],
              ),
            ),
          ),
        ),
        SizedBox(height: 16),
        child,
      ],
    );
  }
}

class _SettingsRow extends StatelessWidget {
  const _SettingsRow({
    required this.title,
    required this.subtitle,
    required this.trailing,
    this.showSubtitle = true,
    this.localizationKey,
  });

  final String title;
  final String subtitle;
  final Widget trailing;
  final bool showSubtitle;
  final String? localizationKey;

  @override
  Widget build(BuildContext context) {
    final labels = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          localizationKey == null
              ? tr(title)
              : tr('$localizationKey.label', fallback: title),
          style: TextStyle(
            color: _text,
            fontSize: 14,
            height: 1.4,
            fontWeight: NautermFontWeights.medium,
            letterSpacing: 0,
          ),
        ),
        if (showSubtitle) ...[
          SizedBox(height: 2),
          Text(
            localizationKey == null
                ? tr(subtitle)
                : tr('$localizationKey.description', fallback: subtitle),
            style: TextStyle(
              color: _faintText,
              fontSize: 12,
              height: 1.35,
              fontWeight: NautermFontWeights.regular,
              letterSpacing: 0,
            ),
          ),
        ],
      ],
    );
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < _settingsStackedRowBreakpoint) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              labels,
              SizedBox(height: 12),
              Align(
                alignment: Alignment.centerRight,
                child: ConstrainedBox(
                  constraints: BoxConstraints(maxWidth: constraints.maxWidth),
                  child: trailing,
                ),
              ),
            ],
          );
        }
        return Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Expanded(child: labels),
            SizedBox(width: 16),
            Expanded(child: trailing),
          ],
        );
      },
    );
  }
}

class _SettingsSwitch extends StatelessWidget {
  const _SettingsSwitch({
    super.key,
    required this.value,
    required this.onChanged,
  });

  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.centerRight,
      child: Transform.scale(
        scale: 0.82,
        alignment: Alignment.centerRight,
        child: CupertinoSwitch(
          value: value,
          onChanged: onChanged,
          activeTrackColor: _settingsDark
              ? const Color(0xff238c68)
              : _secondary,
          inactiveTrackColor: _settingsDark
              ? const Color(0xff28323d)
              : _softOutline,
          thumbColor: Colors.white,
          inactiveThumbColor: _settingsDark
              ? const Color(0xff9aa8b3)
              : Colors.white,
        ),
      ),
    );
  }
}

ButtonStyle _settingsOutlinedButtonStyle() {
  return OutlinedButton.styleFrom(
    foregroundColor: _primary,
    disabledForegroundColor: _faintText,
    backgroundColor: Colors.transparent,
    disabledBackgroundColor: Colors.transparent,
    side: BorderSide(color: _softOutline),
    overlayColor: _primary.withValues(alpha: 0.10),
    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
    textStyle: TextStyle(
      fontSize: 12,
      fontWeight: NautermFontWeights.medium,
      letterSpacing: 0,
    ),
  );
}

class _SettingsSelect extends StatefulWidget {
  const _SettingsSelect({
    super.key,
    required this.label,
    required this.value,
    required this.values,
    required this.onChanged,
    this.format,
    this.leadingBuilder,
    this.semanticsLabel,
    this.showLabel = true,
    this.localizeOptions = true,
    this.searchable = false,
    this.allowCustomValue = false,
  });

  final String label;
  final String value;
  final List<String> values;
  final ValueChanged<String> onChanged;
  final String Function(String value)? format;
  final Widget? Function(String value)? leadingBuilder;
  final String? semanticsLabel;
  final bool showLabel;
  final bool localizeOptions;
  final bool searchable;
  final bool allowCustomValue;

  @override
  State<_SettingsSelect> createState() => _SettingsSelectState();
}

class _SettingsSelectState extends State<_SettingsSelect> {
  final GlobalKey _fieldKey = GlobalKey();
  final LayerLink _fieldLayerLink = LayerLink();
  late final FocusNode _fieldFocusNode;
  final FocusNode _menuFocusNode = FocusNode();
  late final TextEditingController _inputController;
  OverlayEntry? _overlayEntry;
  ScrollController? _menuScrollController;
  int _highlightedIndex = 0;
  bool _hovered = false;

  bool get _open => _overlayEntry != null;

  List<String> get _visibleValues {
    if (!widget.searchable) {
      return widget.values;
    }
    final query = _inputController.text.trim();
    if (query.isEmpty || query.toLowerCase() == widget.value.toLowerCase()) {
      return widget.values;
    }
    final normalized = query.toLowerCase();
    final matches = widget.values
        .where((value) => value.toLowerCase().contains(normalized))
        .toList(growable: true);
    final exact = matches.any((value) => value.toLowerCase() == normalized);
    if (widget.allowCustomValue && !exact) {
      matches.insert(0, query);
    }
    return matches;
  }

  @override
  void initState() {
    super.initState();
    _inputController = TextEditingController(text: widget.value);
    _fieldFocusNode = FocusNode(onKeyEvent: _handleFieldKeyEvent);
    _fieldFocusNode.addListener(_handleFieldFocusChanged);
  }

  @override
  void didUpdateWidget(_SettingsSelect oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_open &&
        (oldWidget.value != widget.value ||
            !listEquals(oldWidget.values, widget.values))) {
      _removeMenuOverlay();
    }
    if (oldWidget.value != widget.value &&
        (!_fieldFocusNode.hasFocus || !_open)) {
      _inputController.text = widget.value;
    }
  }

  @override
  void dispose() {
    _fieldFocusNode.removeListener(_handleFieldFocusChanged);
    _removeMenuOverlay();
    _fieldFocusNode.dispose();
    _menuFocusNode.dispose();
    _inputController.dispose();
    super.dispose();
  }

  void _handleFieldFocusChanged() {
    if (mounted) {
      setState(() {});
    }
  }

  void _setHovered(bool value) {
    if (mounted && _hovered != value) {
      setState(() => _hovered = value);
    }
  }

  void _toggleMenu() {
    if (_open) {
      _closeMenu();
    } else {
      _openMenu();
    }
  }

  void _handleInputTap() {
    _fieldFocusNode.requestFocus();
    if (!_open) {
      _inputController.selection = TextSelection(
        baseOffset: 0,
        extentOffset: _inputController.text.length,
      );
      _openMenu();
    }
  }

  void _handleInputChanged(String _) {
    _highlightedIndex = 0;
    if (!_open) {
      _openMenu();
      return;
    }
    _overlayEntry?.markNeedsBuild();
    if (_menuScrollController?.hasClients ?? false) {
      _menuScrollController!.jumpTo(0);
    }
  }

  void _submitInput(String rawValue) {
    final query = rawValue.trim();
    if (query.isEmpty) {
      return;
    }
    final exact = widget.values
        .where((value) => value.toLowerCase() == query.toLowerCase())
        .firstOrNull;
    if (exact != null) {
      _selectValue(exact);
    } else if (widget.allowCustomValue) {
      _selectValue(query);
    }
  }

  void _openMenu() {
    final values = _visibleValues;
    if (_open || values.isEmpty) {
      return;
    }
    final fieldContext = _fieldKey.currentContext;
    if (fieldContext == null) {
      return;
    }
    final fieldBox = fieldContext.findRenderObject();
    final overlay = Overlay.of(context);
    final overlayBox = overlay.context.findRenderObject();
    if (fieldBox is! RenderBox || overlayBox is! RenderBox) {
      return;
    }

    const rowHeight = _settingsSelectHeight;
    const maximumMenuHeight =
        _settingsSelectHeight * _settingsSelectMaximumVisibleRows;
    final fieldOrigin = fieldBox.localToGlobal(
      Offset.zero,
      ancestor: overlayBox,
    );
    final overlaySize = overlayBox.size;
    final desiredHeight = math.min(
      values.length * rowHeight,
      maximumMenuHeight,
    );
    final spaceBelow =
        overlaySize.height - fieldOrigin.dy - fieldBox.size.height;
    final spaceAbove = fieldOrigin.dy;
    final openAbove = spaceBelow < desiredHeight + 8 && spaceAbove > spaceBelow;
    final availableSpace = openAbove ? spaceAbove : spaceBelow;
    final menuHeight = math.min(
      desiredHeight,
      math.max(rowHeight, availableSpace - 8),
    );
    final selectedIndex = values.indexOf(widget.value);
    _highlightedIndex = selectedIndex < 0 ? 0 : selectedIndex;
    final maximumScrollOffset = math.max(
      0.0,
      values.length * rowHeight - menuHeight,
    );
    final initialScrollOffset = selectedIndex < 0
        ? 0.0
        : ((_highlightedIndex - 2) * rowHeight).clamp(0.0, maximumScrollOffset);
    _menuScrollController = ScrollController(
      initialScrollOffset: initialScrollOffset,
      keepScrollOffset: false,
    );

    _overlayEntry = OverlayEntry(
      builder: (overlayContext) => Stack(
        children: [
          Positioned.fill(
            child: GestureDetector(
              behavior: HitTestBehavior.translucent,
              onTap: _closeMenu,
              child: const SizedBox.expand(),
            ),
          ),
          CompositedTransformFollower(
            link: _fieldLayerLink,
            showWhenUnlinked: false,
            targetAnchor: openAbove ? Alignment.topLeft : Alignment.bottomLeft,
            followerAnchor: openAbove
                ? Alignment.bottomLeft
                : Alignment.topLeft,
            offset: Offset(0, openAbove ? -4 : 4),
            child: SizedBox(
              width: fieldBox.size.width,
              height: math.min(_visibleValues.length * rowHeight, menuHeight),
              child: Focus(
                focusNode: _menuFocusNode,
                onKeyEvent: _handleMenuKeyEvent,
                child: _SettingsSelectMenu(
                  key: const ValueKey('settings-select-menu'),
                  values: _visibleValues,
                  selectedValue: widget.value,
                  highlightedIndex: _highlightedIndex,
                  format: widget.format,
                  localizeOptions: widget.localizeOptions,
                  leadingBuilder: widget.leadingBuilder,
                  scrollController: _menuScrollController!,
                  onSelected: _selectValue,
                ),
              ),
            ),
          ),
        ],
      ),
    );
    overlay.insert(_overlayEntry!);
    setState(() {});
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_open && !widget.searchable) {
        _menuFocusNode.requestFocus();
      }
    });
  }

  void _closeMenu({bool restoreInput = true}) {
    if (!_open) {
      return;
    }
    _removeMenuOverlay();
    if (mounted) {
      setState(() {});
      if (widget.searchable && restoreInput) {
        _inputController.text = widget.value;
      }
      _fieldFocusNode.requestFocus();
    }
  }

  void _removeMenuOverlay() {
    final entry = _overlayEntry;
    _overlayEntry = null;
    if (entry?.mounted ?? false) {
      entry!.remove();
    }
    _menuScrollController?.dispose();
    _menuScrollController = null;
  }

  void _selectValue(String value) {
    _inputController.text = value;
    _closeMenu(restoreInput: false);
    if (value != widget.value) {
      widget.onChanged(value);
    }
  }

  KeyEventResult _handleFieldKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) {
      return KeyEventResult.ignored;
    }
    if (widget.searchable) {
      if (event.logicalKey == LogicalKeyboardKey.escape && _open) {
        _closeMenu();
        return KeyEventResult.handled;
      }
      if (event.logicalKey == LogicalKeyboardKey.enter) {
        _submitInput(_inputController.text);
        return KeyEventResult.handled;
      }
      final delta = switch (event.logicalKey) {
        LogicalKeyboardKey.arrowDown => 1,
        LogicalKeyboardKey.arrowUp => -1,
        _ => 0,
      };
      if (delta != 0) {
        final values = _visibleValues;
        if (!_open) {
          _openMenu();
        } else if (values.isNotEmpty) {
          _highlightedIndex = (_highlightedIndex + delta).clamp(
            0,
            values.length - 1,
          );
          _overlayEntry?.markNeedsBuild();
        }
        return KeyEventResult.handled;
      }
      return KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.enter ||
        event.logicalKey == LogicalKeyboardKey.space ||
        event.logicalKey == LogicalKeyboardKey.arrowDown) {
      _openMenu();
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  KeyEventResult _handleMenuKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) {
      return KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.escape ||
        event.logicalKey == LogicalKeyboardKey.tab) {
      _closeMenu();
      return KeyEventResult.handled;
    }
    if (event.logicalKey == LogicalKeyboardKey.enter ||
        event.logicalKey == LogicalKeyboardKey.space) {
      final values = _visibleValues;
      if (values.isNotEmpty) {
        _selectValue(values[_highlightedIndex.clamp(0, values.length - 1)]);
      }
      return KeyEventResult.handled;
    }
    final delta = switch (event.logicalKey) {
      LogicalKeyboardKey.arrowDown => 1,
      LogicalKeyboardKey.arrowUp => -1,
      _ => 0,
    };
    if (delta == 0) {
      return KeyEventResult.ignored;
    }
    _highlightedIndex = (_highlightedIndex + delta).clamp(
      0,
      _visibleValues.length - 1,
    );
    _overlayEntry?.markNeedsBuild();
    _menuScrollController?.animateTo(
      (_highlightedIndex * _settingsSelectHeight - _settingsSelectHeight).clamp(
        0,
        _menuScrollController!.position.maxScrollExtent,
      ),
      duration: const Duration(milliseconds: 100),
      curve: Curves.easeOut,
    );
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final displayValue = widget.value;
    final focused = _open || _fieldFocusNode.hasFocus;
    final content = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (widget.showLabel) ...[
          Text(
            tr(widget.label),
            style: TextStyle(
              color: _text,
              fontSize: 12,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
          SizedBox(height: 6),
        ],
        MouseRegion(
          onEnter: (_) => _setHovered(true),
          onExit: (_) => _setHovered(false),
          child: Semantics(
            label: tr(widget.semanticsLabel ?? '${widget.label} select'),
            button: true,
            container: true,
            explicitChildNodes: false,
            child: Focus(
              focusNode: widget.searchable ? null : _fieldFocusNode,
              onKeyEvent: widget.searchable ? null : _handleFieldKeyEvent,
              child: CompositedTransformTarget(
                link: _fieldLayerLink,
                child: AnimatedContainer(
                  key: _fieldKey,
                  duration: const Duration(milliseconds: 120),
                  height: _settingsSelectHeight,
                  width: double.infinity,
                  decoration: _settingsFieldDecoration(
                    focused: focused,
                    hovered: _hovered,
                  ),
                  child: Material(
                    color: Colors.transparent,
                    borderRadius: BorderRadius.circular(7),
                    clipBehavior: Clip.antiAlias,
                    child: InkWell(
                      onTap: widget.searchable
                          ? null
                          : () {
                              _fieldFocusNode.requestFocus();
                              _toggleMenu();
                            },
                      hoverColor: Colors.transparent,
                      splashColor: _settingsDark
                          ? _primary.withValues(alpha: 0.10)
                          : const Color(0xffedf4ff),
                      highlightColor: _settingsDark
                          ? _primary.withValues(alpha: 0.10)
                          : const Color(0xffedf4ff),
                      child: Padding(
                        padding: _settingsSelectPadding,
                        child: Row(
                          children: [
                            Expanded(
                              child: widget.searchable
                                  ? TextField(
                                      key: const ValueKey(
                                        'settings-select-input',
                                      ),
                                      controller: _inputController,
                                      focusNode: _fieldFocusNode,
                                      onTap: _handleInputTap,
                                      onChanged: _handleInputChanged,
                                      onSubmitted: _submitInput,
                                      textInputAction: TextInputAction.done,
                                      style: TextStyle(
                                        color: _text,
                                        fontSize: _settingsFieldFontSize,
                                        fontWeight: NautermFontWeights.regular,
                                        height: 1.2,
                                        letterSpacing: 0,
                                      ),
                                      decoration:
                                          const InputDecoration.collapsed(
                                            hintText: null,
                                          ),
                                    )
                                  : Row(
                                      children: [
                                        if (widget.leadingBuilder?.call(
                                              displayValue,
                                            )
                                            case final leading?) ...[
                                          leading,
                                          SizedBox(width: 7),
                                        ],
                                        Expanded(
                                          child: Text(
                                            _formatOption(displayValue),
                                            maxLines: 1,
                                            overflow: TextOverflow.ellipsis,
                                            style: TextStyle(
                                              color: _text,
                                              fontSize: _settingsFieldFontSize,
                                              fontWeight:
                                                  NautermFontWeights.regular,
                                              height: 1.2,
                                              letterSpacing: 0,
                                            ),
                                          ),
                                        ),
                                      ],
                                    ),
                            ),
                            AnimatedRotation(
                              turns: _open ? 0.5 : 0,
                              duration: const Duration(milliseconds: 120),
                              child: Icon(
                                Icons.keyboard_arrow_down_rounded,
                                size: 18,
                                color: focused ? _primary : _mutedText,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
    if (!widget.searchable) {
      return content;
    }
    return Listener(
      behavior: HitTestBehavior.opaque,
      onPointerDown: (_) => _handleInputTap(),
      child: content,
    );
  }

  String _formatOption(String value) {
    final fallback = widget.format?.call(value) ?? value;
    if (!widget.localizeOptions) return fallback;
    return tr(fallback, fallback: fallback);
  }
}

class _SettingsSelectMenu extends StatelessWidget {
  const _SettingsSelectMenu({
    super.key,
    required this.values,
    required this.selectedValue,
    required this.highlightedIndex,
    required this.format,
    this.localizeOptions = true,
    required this.leadingBuilder,
    required this.scrollController,
    required this.onSelected,
  });

  final List<String> values;
  final String selectedValue;
  final int highlightedIndex;
  final String Function(String value)? format;
  final bool localizeOptions;
  final Widget? Function(String value)? leadingBuilder;
  final ScrollController scrollController;
  final ValueChanged<String> onSelected;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: _surface,
        borderRadius: BorderRadius.circular(7),
        border: Border.all(color: _softOutline),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: _settingsDark ? 0.30 : 0.12),
            blurRadius: 18,
            offset: const Offset(0, 8),
          ),
          BoxShadow(
            color: Colors.black.withValues(alpha: _settingsDark ? 0.14 : 0.05),
            blurRadius: 3,
            offset: const Offset(0, 1),
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(7),
        child: LayoutBuilder(
          builder: (context, constraints) {
            final scrollable =
                values.length * _settingsSelectHeight >
                constraints.maxHeight + precisionErrorTolerance;
            return Scrollbar(
              key: const ValueKey('settings-select-scrollbar'),
              controller: scrollController,
              thumbVisibility: scrollable,
              interactive: true,
              child: ListView.builder(
                controller: scrollController,
                padding: EdgeInsets.zero,
                itemExtent: _settingsSelectHeight,
                itemCount: values.length,
                itemBuilder: (context, index) {
                  final value = values[index];
                  final selected = value == selectedValue;
                  final highlighted = index == highlightedIndex;
                  return ColoredBox(
                    color: selected
                        ? (_settingsDark
                              ? _primary.withValues(alpha: 0.18)
                              : const Color(0xffeaf3ff))
                        : highlighted
                        ? _settingsFieldHover
                        : Colors.transparent,
                    child: Material(
                      type: MaterialType.transparency,
                      child: InkWell(
                        hoverColor: _settingsDark
                            ? _settingsFieldHover
                            : selected
                            ? const Color(0xffe3efff)
                            : const Color(0xfff3f6fa),
                        splashColor: _settingsDark
                            ? _primary.withValues(alpha: 0.14)
                            : const Color(0xffdceaff),
                        highlightColor: _settingsDark
                            ? _primary.withValues(alpha: 0.08)
                            : const Color(0xffdceaff),
                        onTap: () => onSelected(value),
                        child: Padding(
                          padding: _settingsSelectPadding.copyWith(right: 10),
                          child: Row(
                            children: [
                              if (leadingBuilder?.call(value)
                                  case final leading?) ...[
                                leading,
                                SizedBox(width: 7),
                              ],
                              Expanded(
                                child: _SettingsOverflowTooltipText(
                                  text: _formatOption(value),
                                  style: TextStyle(
                                    color: selected ? _primary : _text,
                                    fontSize: _settingsFieldFontSize,
                                    height: 1.2,
                                    fontWeight: selected
                                        ? NautermFontWeights.semibold
                                        : NautermFontWeights.regular,
                                    letterSpacing: 0,
                                  ),
                                ),
                              ),
                              SizedBox(
                                width: 20,
                                child: selected
                                    ? Icon(
                                        Icons.check_rounded,
                                        size: 16,
                                        color: _primary,
                                      )
                                    : null,
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  );
                },
              ),
            );
          },
        ),
      ),
    );
  }

  String _formatOption(String value) {
    final fallback = format?.call(value) ?? value;
    if (!localizeOptions) return fallback;
    return tr(fallback, fallback: fallback);
  }
}

class _SettingsOverflowTooltipText extends StatelessWidget {
  const _SettingsOverflowTooltipText({required this.text, required this.style});

  final String text;
  final TextStyle style;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final textWidget = Text(
          text,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: style,
        );
        if (!constraints.maxWidth.isFinite) return textWidget;

        final painter = TextPainter(
          text: TextSpan(text: text, style: style),
          maxLines: 1,
          ellipsis: '\u2026',
          textDirection: Directionality.of(context),
          textScaler: MediaQuery.textScalerOf(context),
        )..layout(maxWidth: constraints.maxWidth);
        final overflowed = painter.didExceedMaxLines;
        painter.dispose();

        if (!overflowed) return textWidget;
        return Tooltip(
          message: text,
          waitDuration: const Duration(milliseconds: 300),
          constraints: const BoxConstraints(maxWidth: 360),
          child: textWidget,
        );
      },
    );
  }
}

class _EditorEntry {
  const _EditorEntry({
    required this.name,
    required this.command,
    this.iconBytes,
  });

  final String name;
  final SftpExternalEditorCommand command;
  final Uint8List? iconBytes;
}

class _SettingsTextField extends StatefulWidget {
  const _SettingsTextField({
    super.key,
    required this.controller,
    required this.onChanged,
    this.obscureText = false,
    this.revealable = false,
    this.onSubmitted,
    this.onFocusLost,
    this.keyboardType,
    this.inputFormatters,
    this.hint,
    this.prefixText,
    this.suffixText,
  });

  final TextEditingController controller;
  final ValueChanged<String> onChanged;
  final bool obscureText;
  final bool revealable;
  final ValueChanged<String>? onSubmitted;
  final ValueChanged<String>? onFocusLost;
  final TextInputType? keyboardType;
  final List<TextInputFormatter>? inputFormatters;
  final String? hint;
  final String? prefixText;
  final String? suffixText;

  @override
  State<_SettingsTextField> createState() => _SettingsTextFieldState();
}

class _SettingsTextFieldState extends State<_SettingsTextField> {
  late final FocusNode _focusNode;
  late bool _obscured;
  bool _hovered = false;

  @override
  void initState() {
    super.initState();
    _focusNode = FocusNode()..addListener(_handleFocusChanged);
    _obscured = widget.obscureText;
    widget.controller.addListener(_handleTextChanged);
  }

  @override
  void didUpdateWidget(_SettingsTextField oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_handleTextChanged);
      widget.controller.addListener(_handleTextChanged);
    }
    if (oldWidget.obscureText != widget.obscureText) {
      _obscured = widget.obscureText;
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_handleTextChanged);
    _focusNode.removeListener(_handleFocusChanged);
    _focusNode.dispose();
    super.dispose();
  }

  void _handleFocusChanged() {
    if (!mounted) {
      return;
    }
    setState(() {});
    if (!_focusNode.hasFocus) {
      widget.onFocusLost?.call(widget.controller.text);
    }
  }

  void _handleTextChanged() {
    if (mounted) {
      setState(() {});
    }
  }

  void _setHovered(bool value) {
    if (mounted && _hovered != value) {
      setState(() => _hovered = value);
    }
  }

  @override
  Widget build(BuildContext context) {
    final focused = _focusNode.hasFocus;
    final textStyle = TextStyle(
      color: _text,
      fontSize: _settingsFieldFontSize,
      fontWeight: NautermFontWeights.regular,
      height: 1.2,
      letterSpacing: 0,
    );
    return Listener(
      behavior: HitTestBehavior.opaque,
      onPointerDown: (_) {
        if (!_focusNode.hasFocus) {
          _focusNode.requestFocus();
        }
      },
      child: MouseRegion(
        onEnter: (_) => _setHovered(true),
        onExit: (_) => _setHovered(false),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 120),
          width: double.infinity,
          height: _settingsSelectHeight,
          decoration: _settingsFieldDecoration(
            focused: focused,
            hovered: _hovered,
          ),
          child: Material(
            color: Colors.transparent,
            borderRadius: BorderRadius.circular(7),
            clipBehavior: Clip.antiAlias,
            child: Padding(
              padding: _settingsInputPadding,
              child: Row(
                children: [
                  if (widget.prefixText case final prefixText?) ...[
                    Text(
                      prefixText,
                      style: textStyle.copyWith(color: _mutedText),
                    ),
                    const SizedBox(width: 2),
                  ],
                  Expanded(
                    child: SizedBox(
                      height: _settingsFieldTextHeight,
                      child: TextField(
                        controller: widget.controller,
                        focusNode: _focusNode,
                        style: textStyle,
                        strutStyle: StrutStyle.fromTextStyle(
                          textStyle,
                          forceStrutHeight: true,
                        ),
                        cursorColor: _primary,
                        cursorHeight: 18,
                        cursorWidth: 1.4,
                        maxLines: 1,
                        obscureText: _obscured,
                        enableSuggestions: !_obscured,
                        autocorrect: false,
                        keyboardType: widget.keyboardType,
                        inputFormatters: widget.inputFormatters,
                        onChanged: widget.onChanged,
                        onSubmitted: widget.onSubmitted,
                        decoration: InputDecoration(
                          isCollapsed: true,
                          border: InputBorder.none,
                          enabledBorder: InputBorder.none,
                          focusedBorder: InputBorder.none,
                          hintText: widget.hint == null
                              ? null
                              : tr(widget.hint!),
                          hintStyle: textStyle.copyWith(color: _faintText),
                          contentPadding: EdgeInsets.zero,
                        ),
                        selectionControls: switch (defaultTargetPlatform) {
                          TargetPlatform.macOS =>
                            cupertinoTextSelectionControls,
                          TargetPlatform.iOS => cupertinoTextSelectionControls,
                          _ => materialTextSelectionControls,
                        },
                        selectionHeightStyle: BoxHeightStyle.tight,
                        selectionWidthStyle: BoxWidthStyle.tight,
                        contextMenuBuilder: (context, editableTextState) {
                          return AdaptiveTextSelectionToolbar.editableText(
                            editableTextState: editableTextState,
                          );
                        },
                      ),
                    ),
                  ),
                  if (widget.suffixText case final suffixText?) ...[
                    const SizedBox(width: 6),
                    Text(
                      suffixText,
                      style: textStyle.copyWith(color: _mutedText),
                    ),
                  ],
                  if (widget.obscureText && widget.revealable)
                    ExcludeFocusTraversal(
                      key: const ValueKey('settings-credential-reveal'),
                      child: Tooltip(
                        message: _obscured
                            ? 'Show credential'
                            : 'Hide credential',
                        child: IconButton(
                          onPressed: () =>
                              setState(() => _obscured = !_obscured),
                          padding: EdgeInsets.zero,
                          constraints: const BoxConstraints.tightFor(
                            width: 28,
                            height: 28,
                          ),
                          splashRadius: 14,
                          icon: Icon(
                            _obscured ? LucideIcons.eyeOff : LucideIcons.eye,
                            size: 14,
                            color: _mutedText,
                          ),
                        ),
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
