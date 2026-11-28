part of 'settings_panel.dart';

String _formatShortcut(String value) {
  return formatShortcutForPlatform(value, compact: false) ??
      tr('common.label.disabled', fallback: 'Disabled');
}

class _ShortcutBindingsTable extends StatelessWidget {
  const _ShortcutBindingsTable({required this.config, required this.onChanged});

  static const _defaults = TerminalShortcutConfig();

  final TerminalShortcutConfig config;
  final ValueChanged<TerminalShortcutConfig> onChanged;

  @override
  Widget build(BuildContext context) {
    final rows = <_ShortcutBindingRow>[
      _ShortcutBindingRow(
        tr('common.label.workspace', fallback: 'Workspace'),
        tr('settings.shortcuts.action.quickConnect', fallback: 'Quick Connect'),
        config.quickConnect,
        _defaults.quickConnect,
        (v) => onChanged(config.copyWith(quickConnect: v)),
      ),
      _ShortcutBindingRow(
        tr('common.label.workspace', fallback: 'Workspace'),
        tr(
          'settings.shortcuts.action.commandPalette',
          fallback: 'Command Palette',
        ),
        config.commandPalette,
        _defaults.commandPalette,
        (v) => onChanged(config.copyWith(commandPalette: v)),
      ),
      _ShortcutBindingRow(
        tr('common.label.workspace', fallback: 'Workspace'),
        tr('settings.shortcuts.action.openSettings', fallback: 'Open Settings'),
        config.openSettings,
        _defaults.openSettings,
        (v) => onChanged(config.copyWith(openSettings: v)),
      ),
      _ShortcutBindingRow(
        tr('common.label.workspace', fallback: 'Workspace'),
        tr(
          'settings.shortcuts.action.editWorkspaceItem',
          fallback: 'Edit Selected Item',
        ),
        config.editWorkspaceItem,
        _defaults.editWorkspaceItem,
        (v) => onChanged(config.copyWith(editWorkspaceItem: v)),
      ),
      _ShortcutBindingRow(
        tr('common.label.workspace', fallback: 'Workspace'),
        tr(
          'settings.shortcuts.action.duplicateWorkspaceItem',
          fallback: 'Duplicate Selected Item',
        ),
        config.duplicateWorkspaceItem,
        _defaults.duplicateWorkspaceItem,
        (v) => onChanged(config.copyWith(duplicateWorkspaceItem: v)),
      ),
      _ShortcutBindingRow(
        tr('common.label.workspace', fallback: 'Workspace'),
        tr('settings.shortcuts.action.switchToSsh', fallback: 'Switch to SSH'),
        config.switchToSsh,
        _defaults.switchToSsh,
        (v) => onChanged(config.copyWith(switchToSsh: v)),
      ),
      _ShortcutBindingRow(
        tr('common.label.workspace', fallback: 'Workspace'),
        tr(
          'settings.shortcuts.action.switchToSftp',
          fallback: 'Switch to SFTP',
        ),
        config.switchToSftp,
        _defaults.switchToSftp,
        (v) => onChanged(config.copyWith(switchToSftp: v)),
      ),
      _ShortcutBindingRow(
        tr('settings.shortcuts.scope.tabs', fallback: 'Tabs'),
        tr('settings.shortcuts.action.previousTab', fallback: 'Previous Tab'),
        config.previousTab,
        _defaults.previousTab,
        (v) => onChanged(config.copyWith(previousTab: v)),
      ),
      _ShortcutBindingRow(
        tr('settings.shortcuts.scope.tabs', fallback: 'Tabs'),
        tr('settings.shortcuts.action.nextTab', fallback: 'Next Tab'),
        config.nextTab,
        _defaults.nextTab,
        (v) => onChanged(config.copyWith(nextTab: v)),
      ),
      _ShortcutBindingRow(
        tr('settings.shortcuts.scope.tabs', fallback: 'Tabs'),
        tr('settings.shortcuts.action.closeTab', fallback: 'Close Tab'),
        config.closeTab,
        _defaults.closeTab,
        (v) => onChanged(config.copyWith(closeTab: v)),
      ),
      for (var i = 0; i < config.tabSwitches.length; i++)
        _ShortcutBindingRow(
          tr('settings.shortcuts.scope.tabs', fallback: 'Tabs'),
          tr(
            'settings.shortcuts.action.selectTab',
            fallback: 'Select Tab {number}',
            args: {'number': i + 1},
          ),
          config.tabSwitches[i],
          i < _defaults.tabSwitches.length ? _defaults.tabSwitches[i] : '',
          (v) {
            final newSwitches = List<String>.from(config.tabSwitches);
            newSwitches[i] = v;
            onChanged(config.copyWith(tabSwitches: newSwitches));
          },
        ),
      _ShortcutBindingRow(
        tr('common.label.terminal', fallback: 'Terminal'),
        tr('settings.shortcuts.action.copy', fallback: 'Copy'),
        config.copy,
        _defaults.copy,
        (v) => onChanged(config.copyWith(copy: v)),
        terminalEditing: true,
      ),
      _ShortcutBindingRow(
        tr('common.label.terminal', fallback: 'Terminal'),
        tr('settings.shortcuts.action.paste', fallback: 'Paste'),
        config.paste,
        _defaults.paste,
        (v) => onChanged(config.copyWith(paste: v)),
        terminalEditing: true,
      ),
      _ShortcutBindingRow(
        tr('common.label.terminal', fallback: 'Terminal'),
        tr('settings.shortcuts.action.selectAll', fallback: 'Select All'),
        config.selectAll,
        _defaults.selectAll,
        (v) => onChanged(config.copyWith(selectAll: v)),
        terminalEditing: true,
      ),
      _ShortcutBindingRow(
        tr('common.label.terminal', fallback: 'Terminal'),
        tr('settings.shortcuts.action.search', fallback: 'Search'),
        config.search,
        _defaults.search,
        (v) => onChanged(config.copyWith(search: v)),
        terminalEditing: true,
      ),
      _ShortcutBindingRow(
        tr('common.label.terminal', fallback: 'Terminal'),
        tr('settings.shortcuts.action.splitRight', fallback: 'Split Right'),
        config.splitRight,
        _defaults.splitRight,
        (v) => onChanged(config.copyWith(splitRight: v)),
      ),
      _ShortcutBindingRow(
        tr('common.label.terminal', fallback: 'Terminal'),
        tr('settings.shortcuts.action.splitDown', fallback: 'Split Down'),
        config.splitDown,
        _defaults.splitDown,
        (v) => onChanged(config.copyWith(splitDown: v)),
      ),
      _ShortcutBindingRow(
        tr('common.label.terminal', fallback: 'Terminal'),
        tr(
          'settings.shortcuts.action.newLocalTerminal',
          fallback: 'New Local Terminal',
        ),
        config.newLocalTerminal,
        _defaults.newLocalTerminal,
        (v) => onChanged(config.copyWith(newLocalTerminal: v)),
      ),
    ];

    return Container(
      clipBehavior: Clip.antiAlias,
      decoration: BoxDecoration(
        color: _surface,
        border: Border.all(color: _softOutline),
        borderRadius: BorderRadius.circular(9),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.04),
            blurRadius: 8,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      child: Column(
        children: [
          const _ShortcutBindingsHeader(),
          for (final row in rows) _ShortcutBindingRowWidget(row: row),
        ],
      ),
    );
  }
}

class _ShortcutBindingRow {
  const _ShortcutBindingRow(
    this.group,
    this.action,
    this.shortcut,
    this.defaultShortcut,
    this.onChanged, {
    this.terminalEditing = false,
  });

  final String group;
  final String action;
  final String shortcut;
  final String defaultShortcut;
  final ValueChanged<String> onChanged;
  final bool terminalEditing;
}

class _ShortcutBindingsHeader extends StatelessWidget {
  const _ShortcutBindingsHeader();

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 44,
      color: _surfaceContainer,
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: Row(
        children: [
          Expanded(
            flex: 2,
            child: _TableHeaderText(
              tr('common.label.scope', fallback: 'Scope'),
            ),
          ),
          Expanded(
            flex: 4,
            child: _TableHeaderText(
              tr('common.label.action', fallback: 'Action'),
            ),
          ),
          Expanded(
            flex: 3,
            child: _TableHeaderText(
              tr('common.label.shortcut', fallback: 'Shortcut'),
            ),
          ),
          SizedBox(width: 72),
        ],
      ),
    );
  }
}

class _ShortcutResetAllButton extends StatelessWidget {
  const _ShortcutResetAllButton({required this.onPressed});

  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tr(
        'settings.label.restoreAllDefaultShortcuts',
        fallback: 'Restore all default shortcuts',
      ),
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(6),
        child: InkWell(
          borderRadius: BorderRadius.circular(6),
          hoverColor: _settingsFieldHover,
          splashColor: _primary.withValues(alpha: 0.12),
          onTap: onPressed,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(LucideIcons.rotateCcw, size: 13, color: _mutedText),
                SizedBox(width: 5),
                Text(
                  tr('settings.label.resetAll', fallback: 'Reset all'),
                  style: TextStyle(
                    color: _mutedText,
                    fontSize: 11,
                    fontWeight: NautermFontWeights.medium,
                    letterSpacing: 0,
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

class _TableHeaderText extends StatelessWidget {
  const _TableHeaderText(this.label);

  final String label;

  @override
  Widget build(BuildContext context) {
    return Text(
      label.toUpperCase(),
      style: TextStyle(
        color: const Color(0xff6b7280),
        fontSize: 10,
        fontWeight: FontWeight.w700,
        letterSpacing: 0.7,
      ),
    );
  }
}

class _ShortcutBindingRowWidget extends StatefulWidget {
  const _ShortcutBindingRowWidget({required this.row});

  final _ShortcutBindingRow row;

  @override
  State<_ShortcutBindingRowWidget> createState() =>
      _ShortcutBindingRowWidgetState();
}

class _ShortcutBindingRowWidgetState extends State<_ShortcutBindingRowWidget> {
  bool _recording = false;
  final _focusNode = FocusNode();

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  void _startRecording() {
    setState(() => _recording = true);
    _focusNode.requestFocus();
  }

  void _stopRecording() {
    if (!_recording) {
      return;
    }
    setState(() => _recording = false);
  }

  KeyEventResult _handleKeyEvent(FocusNode node, KeyEvent event) {
    if (!_recording) {
      return KeyEventResult.ignored;
    }
    if (event is! KeyDownEvent) {
      return KeyEventResult.handled;
    }

    final key = event.logicalKey;
    if (key == LogicalKeyboardKey.escape) {
      _stopRecording();
      return KeyEventResult.handled;
    }

    final isMacOS = defaultTargetPlatform == TargetPlatform.macOS;
    final hasMeta = HardwareKeyboard.instance.isMetaPressed;
    final hasCtrl = HardwareKeyboard.instance.isControlPressed;
    final hasShift = HardwareKeyboard.instance.isShiftPressed;
    final hasAlt = HardwareKeyboard.instance.isAltPressed;

    if (isMacOS ? !hasMeta : !hasCtrl) {
      return KeyEventResult.handled;
    }

    final parts = <String>['cmd'];
    if (hasAlt) {
      parts.add('alt');
    }
    if (hasShift) {
      parts.add('shift');
    }

    final keyName = _keyNameFromLogical(key);
    if (keyName == null) {
      return KeyEventResult.handled;
    }

    parts.add(keyName);
    widget.row.onChanged(parts.join('+'));
    _stopRecording();
    return KeyEventResult.handled;
  }

  static String? _keyNameFromLogical(LogicalKeyboardKey key) {
    if (key == LogicalKeyboardKey.keyA) return 'a';
    if (key == LogicalKeyboardKey.keyB) return 'b';
    if (key == LogicalKeyboardKey.keyC) return 'c';
    if (key == LogicalKeyboardKey.keyD) return 'd';
    if (key == LogicalKeyboardKey.keyE) return 'e';
    if (key == LogicalKeyboardKey.keyF) return 'f';
    if (key == LogicalKeyboardKey.keyG) return 'g';
    if (key == LogicalKeyboardKey.keyH) return 'h';
    if (key == LogicalKeyboardKey.keyI) return 'i';
    if (key == LogicalKeyboardKey.keyJ) return 'j';
    if (key == LogicalKeyboardKey.keyK) return 'k';
    if (key == LogicalKeyboardKey.keyL) return 'l';
    if (key == LogicalKeyboardKey.keyM) return 'm';
    if (key == LogicalKeyboardKey.keyN) return 'n';
    if (key == LogicalKeyboardKey.keyO) return 'o';
    if (key == LogicalKeyboardKey.keyP) return 'p';
    if (key == LogicalKeyboardKey.keyQ) return 'q';
    if (key == LogicalKeyboardKey.keyR) return 'r';
    if (key == LogicalKeyboardKey.keyS) return 's';
    if (key == LogicalKeyboardKey.keyT) return 't';
    if (key == LogicalKeyboardKey.keyU) return 'u';
    if (key == LogicalKeyboardKey.keyV) return 'v';
    if (key == LogicalKeyboardKey.keyW) return 'w';
    if (key == LogicalKeyboardKey.keyX) return 'x';
    if (key == LogicalKeyboardKey.keyY) return 'y';
    if (key == LogicalKeyboardKey.keyZ) return 'z';
    if (key == LogicalKeyboardKey.digit0) return '0';
    if (key == LogicalKeyboardKey.digit1) return '1';
    if (key == LogicalKeyboardKey.digit2) return '2';
    if (key == LogicalKeyboardKey.digit3) return '3';
    if (key == LogicalKeyboardKey.digit4) return '4';
    if (key == LogicalKeyboardKey.digit5) return '5';
    if (key == LogicalKeyboardKey.digit6) return '6';
    if (key == LogicalKeyboardKey.digit7) return '7';
    if (key == LogicalKeyboardKey.digit8) return '8';
    if (key == LogicalKeyboardKey.digit9) return '9';
    if (key == LogicalKeyboardKey.bracketLeft) return '[';
    if (key == LogicalKeyboardKey.bracketRight) return ']';
    if (key == LogicalKeyboardKey.slash) return '/';
    if (key == LogicalKeyboardKey.backslash) return '\\';
    if (key == LogicalKeyboardKey.semicolon) return ';';
    if (key == LogicalKeyboardKey.quote) return "'";
    if (key == LogicalKeyboardKey.comma) return ',';
    if (key == LogicalKeyboardKey.period) return '.';
    if (key == LogicalKeyboardKey.minus) return '-';
    if (key == LogicalKeyboardKey.equal) return '=';
    if (key == LogicalKeyboardKey.backquote) return '`';
    if (key == LogicalKeyboardKey.arrowRight) return 'right';
    if (key == LogicalKeyboardKey.arrowDown) return 'down';
    if (key == LogicalKeyboardKey.arrowLeft) return 'left';
    if (key == LogicalKeyboardKey.arrowUp) return 'up';
    if (key == LogicalKeyboardKey.home) return 'home';
    if (key == LogicalKeyboardKey.end) return 'end';
    if (key == LogicalKeyboardKey.pageUp) return 'pageup';
    if (key == LogicalKeyboardKey.pageDown) return 'pagedown';
    if (key == LogicalKeyboardKey.space) return 'space';
    if (key == LogicalKeyboardKey.enter) return 'enter';
    if (key == LogicalKeyboardKey.tab) return 'tab';
    if (key == LogicalKeyboardKey.backspace) return 'backspace';
    if (key == LogicalKeyboardKey.delete) return 'delete';
    if (key == LogicalKeyboardKey.f1) return 'f1';
    if (key == LogicalKeyboardKey.f2) return 'f2';
    if (key == LogicalKeyboardKey.f3) return 'f3';
    if (key == LogicalKeyboardKey.f4) return 'f4';
    if (key == LogicalKeyboardKey.f5) return 'f5';
    if (key == LogicalKeyboardKey.f6) return 'f6';
    if (key == LogicalKeyboardKey.f7) return 'f7';
    if (key == LogicalKeyboardKey.f8) return 'f8';
    if (key == LogicalKeyboardKey.f9) return 'f9';
    if (key == LogicalKeyboardKey.f10) return 'f10';
    if (key == LogicalKeyboardKey.f11) return 'f11';
    if (key == LogicalKeyboardKey.f12) return 'f12';
    return null;
  }

  @override
  Widget build(BuildContext context) {
    return Focus(
      focusNode: _focusNode,
      onKeyEvent: _handleKeyEvent,
      onFocusChange: (hasFocus) {
        if (!hasFocus && _recording) {
          _stopRecording();
        }
      },
      child: Container(
        height: 48,
        padding: const EdgeInsets.symmetric(horizontal: 16),
        decoration: BoxDecoration(
          color: _surface,
          border: Border(top: BorderSide(color: _softOutline)),
        ),
        child: Row(
          children: [
            Expanded(
              flex: 2,
              child: Text(
                widget.row.group,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: _faintText,
                  fontSize: 11,
                  fontWeight: NautermFontWeights.medium,
                  letterSpacing: 0,
                ),
              ),
            ),
            Expanded(
              flex: 4,
              child: Text(
                widget.row.action,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: _text,
                  fontSize: 13,
                  fontWeight: NautermFontWeights.medium,
                  letterSpacing: 0,
                ),
              ),
            ),
            Expanded(
              flex: 3,
              child: TapRegion(
                onTapOutside: (_) => _stopRecording(),
                child: Tooltip(
                  message: context.tr(
                    'settings.shortcuts.recording.hint',
                    fallback:
                        'Click to record a shortcut. Escape cancels recording.',
                  ),
                  child: GestureDetector(
                    key: ValueKey('shortcut-recorder:${widget.row.action}'),
                    onTap: _recording ? _stopRecording : _startRecording,
                    child: Center(
                      child: SizedBox(
                        width: double.infinity,
                        height: 28,
                        child: _recording
                            ? Container(
                                key: ValueKey(
                                  'shortcut-recorder-surface:${widget.row.action}',
                                ),
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 9,
                                  vertical: 4,
                                ),
                                decoration: BoxDecoration(
                                  color: _primary.withValues(alpha: 0.14),
                                  border: Border.all(
                                    color: _primary.withValues(alpha: 0.40),
                                  ),
                                  borderRadius: BorderRadius.circular(6),
                                ),
                                child: Text(
                                  tr(
                                    'settings.shortcuts.recording.prompt',
                                    fallback: 'Press shortcut…',
                                  ),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                    color: _primary,
                                    fontSize: 11,
                                    fontFamily: 'monospace',
                                    fontWeight: NautermFontWeights.medium,
                                    letterSpacing: 0,
                                  ),
                                ),
                              )
                            : _ShortcutBadge(
                                key: ValueKey(
                                  'shortcut-recorder-surface:${widget.row.action}',
                                ),
                                label: widget.row.terminalEditing
                                    ? formatTerminalEditingShortcutForPlatform(
                                            widget.row.shortcut,
                                            compact: false,
                                          ) ??
                                          tr(
                                            'common.label.disabled',
                                            fallback: 'Disabled',
                                          )
                                    : _formatShortcut(widget.row.shortcut),
                              ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
            SizedBox(
              width: 72,
              child: Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  _ShortcutRowAction(
                    icon: LucideIcons.ban,
                    tooltip: tr(
                      'settings.label.disableShortcut',
                      fallback: 'Disable shortcut',
                    ),
                    enabled: widget.row.shortcut.isNotEmpty,
                    onPressed: () {
                      _stopRecording();
                      widget.row.onChanged('');
                    },
                  ),
                  SizedBox(width: 4),
                  _ShortcutRowAction(
                    icon: LucideIcons.rotateCcw,
                    tooltip: tr(
                      'settings.label.restoreDefault',
                      fallback: 'Restore default',
                    ),
                    enabled: widget.row.shortcut != widget.row.defaultShortcut,
                    onPressed: () {
                      _stopRecording();
                      widget.row.onChanged(widget.row.defaultShortcut);
                    },
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

class _ShortcutBadge extends StatelessWidget {
  const _ShortcutBadge({super.key, required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Container(
      constraints: const BoxConstraints(minHeight: 28),
      alignment: Alignment.centerLeft,
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 4),
      decoration: BoxDecoration(
        color: _settingsFieldHover,
        border: Border.all(color: _softOutline),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Text(
        tr(label),
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: TextStyle(
          color: _mutedText,
          fontSize: 11,
          fontFamily: 'monospace',
          fontWeight: NautermFontWeights.medium,
          letterSpacing: 0,
        ),
      ),
    );
  }
}

class _ShortcutRowAction extends StatelessWidget {
  const _ShortcutRowAction({
    required this.icon,
    required this.tooltip,
    required this.enabled,
    required this.onPressed,
  });

  final IconData icon;
  final String tooltip;
  final bool enabled;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tr(tooltip),
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(6),
        child: InkWell(
          borderRadius: BorderRadius.circular(6),
          hoverColor: enabled ? _settingsFieldHover : Colors.transparent,
          splashColor: enabled
              ? _primary.withValues(alpha: 0.12)
              : Colors.transparent,
          onTap: enabled ? onPressed : null,
          child: SizedBox(
            width: 28,
            height: 28,
            child: Icon(
              icon,
              size: 15,
              color: enabled ? _mutedText : _faintText.withValues(alpha: 0.45),
            ),
          ),
        ),
      ),
    );
  }
}
