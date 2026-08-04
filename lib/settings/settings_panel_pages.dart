part of 'settings_panel.dart';

Widget _buildSettingsGeneralContent(_SettingsPanelState state) {
  return Scrollbar(
    controller: state._contentScrollController,
    thumbVisibility: true,
    child: ListView(
      scrollCacheExtent: const ScrollCacheExtent.viewport(100),
      controller: state._contentScrollController,
      padding: state._contentPadding,
      children: [
        Text(
          state.context.tr(
            'settings.pages.general.title',
            fallback: 'General Settings',
          ),
          style: TextStyle(
            color: _text,
            fontSize: 20,
            height: 1.4,
            fontWeight: NautermFontWeights.semibold,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 3),
        Text(
          state.context.tr(
            'settings.pages.general.description',
            fallback:
                'Manage appearance, window size, and application behavior.',
          ),
          style: TextStyle(
            color: _mutedText,
            fontSize: 12,
            fontWeight: NautermFontWeights.regular,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('general-appearance'),
          icon: Icons.palette_outlined,
          title: state.context.tr(
            'common.label.appearance',
            fallback: 'Appearance',
          ),
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _ApplicationThemeSelector(
                  value: state._applicationTheme,
                  onChanged: (value) {
                    state._mutate(() => state._applicationTheme = value);
                    setAppThemeMode(value);
                    state._persistRuntimeSettings();
                  },
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.general.language',
                  title: 'Language',
                  subtitle: 'Choose the language used by Nauterm.',
                  trailing: _SettingsSelect(
                    key: const ValueKey('settings-language-select'),
                    label: state.context.tr(
                      'common.label.language',
                      fallback: 'Language',
                    ),
                    showLabel: false,
                    value: state._applicationLanguage.configValue,
                    values: AppLanguage.values
                        .map((language) => language.configValue)
                        .toList(growable: false),
                    format: (value) => AppLanguage.fromString(
                      value,
                    ).displayName(state.context.l10n),
                    onChanged: (value) {
                      final language = AppLanguage.fromString(value);
                      state._mutate(
                        () => state._applicationLanguage = language,
                      );
                      setAppLanguage(language);
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.general.hostIcon',
                  title: 'Host Icon',
                  subtitle: 'How to display host icons in the workspace.',
                  trailing: _SettingsSelect(
                    label: 'Host Icon',
                    showLabel: false,
                    value: hostIconMode.name,
                    values: HostIconMode.values
                        .map((m) => m.name)
                        .toList(growable: false),
                    format: _hostIconModeLabel,
                    onChanged: (value) {
                      final mode = HostIconMode.values.byName(value);
                      state._mutate(() => hostIconMode = mode);
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
              ],
            ),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('general-window'),
          icon: Icons.crop_rounded,
          title: state.context.tr('common.label.window', fallback: 'Window'),
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: LayoutBuilder(
              builder: (context, constraints) {
                final widthField = _SettingsRow(
                  localizationKey: 'settings.general.window.width',
                  title: 'Width',
                  subtitle: 'Default window width in pixels.',
                  trailing: _SettingsTextField(
                    controller: state._windowWidthController,
                    onChanged: (value) {
                      state._windowWidth = value;
                      state._windowSizeDirty = true;
                      state._scheduleWindowSizeUpdate();
                    },
                    onSubmitted: (_) => state._applyWindowSizeSettings(),
                    keyboardType: TextInputType.number,
                    inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                  ),
                );
                final heightField = _SettingsRow(
                  localizationKey: 'settings.general.window.height',
                  title: 'Height',
                  subtitle: 'Default window height in pixels.',
                  trailing: _SettingsTextField(
                    controller: state._windowHeightController,
                    onChanged: (value) {
                      state._windowHeight = value;
                      state._windowSizeDirty = true;
                      state._scheduleWindowSizeUpdate();
                    },
                    onSubmitted: (_) => state._applyWindowSizeSettings(),
                    keyboardType: TextInputType.number,
                    inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                  ),
                );
                if (constraints.maxWidth < _settingsStackedFieldsBreakpoint) {
                  return Column(
                    children: [widthField, SizedBox(height: 18), heightField],
                  );
                }
                return Column(
                  children: [
                    Row(
                      children: [
                        Expanded(child: widthField),
                        SizedBox(width: 16),
                        Expanded(child: heightField),
                      ],
                    ),
                  ],
                );
              },
            ),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('general-behavior'),
          icon: Icons.tune_rounded,
          title: state.context.tr(
            'common.label.behavior',
            fallback: 'Behavior',
          ),
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  localizationKey: 'settings.general.workspacePage',
                  title: 'Workspace Page',
                  subtitle:
                      'Show the workspace overview page in the sessions tab.',
                  trailing: _SettingsSwitch(
                    value: state._workspacePageEnabled,
                    onChanged: (value) {
                      state._mutate(() => state._workspacePageEnabled = value);
                      setWorkspacePageEnabled(value);
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.general.confirmOnClose',
                  title: 'Confirm on Close',
                  subtitle:
                      'Ask for confirmation before closing a terminal tab.',
                  trailing: _SettingsSwitch(
                    value: terminalConfirmOnClose,
                    onChanged: (value) {
                      state._mutate(() => terminalConfirmOnClose = value);
                      state._persistRuntimeSettings();
                    },
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

Widget _buildSettingsExternalEditorSelect(_SettingsPanelState state) {
  if (state._externalEditors.isEmpty) {
    return _SettingsRow(
      localizationKey: 'settings.sftp.externalEditor',
      title: 'External Editor',
      subtitle: 'Editor used for configured text file extensions.',
      trailing: _SettingsSelect(
        label: 'External Editor',
        showLabel: false,
        value: 'none',
        values: const ['none'],
        format: (_) => 'Detecting editors...',
        onChanged: (_) {},
      ),
    );
  }

  return _SettingsRow(
    localizationKey: 'settings.sftp.externalEditor',
    title: 'External Editor',
    subtitle: 'Editor used for configured text file extensions.',
    trailing: _SettingsSelect(
      label: 'External Editor',
      showLabel: false,
      value:
          state._externalEditors
              .where((entry) => entry.command.id == state._externalEditor?.id)
              .firstOrNull
              ?.command
              .id ??
          state._externalEditors.first.command.id,
      values: state._externalEditors.map((entry) => entry.command.id).toList(),
      leadingBuilder: (id) {
        final iconBytes = state._externalEditors
            .where((entry) => entry.command.id == id)
            .firstOrNull
            ?.iconBytes;
        if (iconBytes == null) {
          return Icon(LucideIcons.appWindow, size: 17, color: _mutedText);
        }
        return Image.memory(
          iconBytes,
          width: 18,
          height: 18,
          filterQuality: FilterQuality.medium,
          gaplessPlayback: true,
          errorBuilder: (_, _, _) =>
              Icon(LucideIcons.appWindow, size: 17, color: _mutedText),
        );
      },
      format: (id) {
        final match = state._externalEditors
            .where((entry) => entry.command.id == id)
            .firstOrNull;
        return match?.name ?? id;
      },
      onChanged: (value) {
        final command = state._externalEditors
            .where((entry) => entry.command.id == value)
            .firstOrNull
            ?.command;
        if (command == null) return;
        state._mutate(() => state._externalEditor = command);
        sftpExternalEditor = command;
        state._persistRuntimeSettings();
      },
    ),
  );
}

Widget _buildSettingsTerminalContent(_SettingsPanelState state) {
  final fontOptions = [
    ...state._monospaceFontFamilies.where(
      (family) => family.trim().toLowerCase() != 'monospace',
    ),
  ];
  final fontSizeOptions = [
    for (final size in terminalFontSizeOptions)
      size.toStringAsFixed(size == size.roundToDouble() ? 0 : 1),
    if (!terminalFontSizeOptions.contains(double.tryParse(state._fontSize)))
      state._fontSize,
  ];
  final cjkFontOptions = [
    '',
    ...state._systemFontFamilies,
    if (state._cjkFontFamily.isNotEmpty &&
        !state._systemFontFamilies.contains(state._cjkFontFamily))
      state._cjkFontFamily,
  ];

  return Scrollbar(
    controller: state._contentScrollController,
    thumbVisibility: true,
    child: ListView(
      key: const ValueKey('settings-terminal-scroll-view'),
      scrollCacheExtent: const ScrollCacheExtent.viewport(100),
      controller: state._contentScrollController,
      padding: state._contentPadding,
      children: [
        Text(
          tr('settings.pages.terminal.title', fallback: 'Terminal'),
          style: TextStyle(
            color: _text,
            fontSize: 20,
            height: 1.4,
            fontWeight: NautermFontWeights.semibold,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 3),
        Text(
          tr(
            'settings.pages.terminal.description',
            fallback:
                'Configure terminal display and text interaction defaults.',
          ),
          style: TextStyle(
            color: _mutedText,
            fontSize: 12,
            fontWeight: NautermFontWeights.regular,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('terminal-theme'),
          icon: Icons.palette_outlined,
          title: 'Theme',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: _buildSettingsThemeSection(state),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('terminal-font'),
          icon: Icons.text_format,
          title: 'Font',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                LayoutBuilder(
                  builder: (context, constraints) {
                    final fontField = _SettingsSelect(
                      key: const ValueKey('settings-terminal-font-select'),
                      label: 'Font',
                      semanticsLabel: 'Terminal Font select',
                      value: state._fontFamily,
                      values: fontOptions,
                      searchable: true,
                      allowCustomValue: true,
                      format: (value) =>
                          value.trim().toLowerCase() == 'monospace'
                          ? tr(
                              'settings.terminal.systemMonospace',
                              fallback: 'System Monospace',
                            )
                          : value,
                      localizeOptions: false,
                      onChanged: (value) =>
                          state._updateTerminalFont(family: value),
                    );
                    final fontSizeField = _SettingsSelect(
                      key: const ValueKey('settings-terminal-font-size-select'),
                      label: 'Font Size',
                      value: state._fontSize,
                      values: fontSizeOptions,
                      onChanged: (value) {
                        final size = double.tryParse(value);
                        if (size != null) {
                          state._updateTerminalFont(size: size);
                        }
                      },
                    );
                    if (constraints.maxWidth <
                        _settingsStackedFieldsBreakpoint) {
                      return Column(
                        children: [
                          fontField,
                          SizedBox(height: 14),
                          fontSizeField,
                        ],
                      );
                    }
                    return Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(child: fontField),
                        SizedBox(width: 16),
                        Expanded(child: fontSizeField),
                      ],
                    );
                  },
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.cjkFont',
                  title: 'CJK Font',
                  subtitle:
                      'Leave blank to follow the application language, then the system fallback.',
                  trailing: _SettingsSelect(
                    key: const ValueKey('settings-terminal-cjk-font-select'),
                    label: 'CJK Font',
                    showLabel: false,
                    value: state._cjkFontFamily,
                    values: cjkFontOptions,
                    format: (value) => value.isEmpty
                        ? tr('settings.terminal.cjkFont.auto', fallback: 'Auto')
                        : value,
                    localizeOptions: false,
                    onChanged: state._updateTerminalCjkFont,
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.lineHeight',
                  title: 'Line Height',
                  subtitle: 'Multiplier for line spacing.',
                  trailing: _SettingsSelect(
                    label: 'Line Height',
                    showLabel: false,
                    value: terminalFontConfig.lineHeight.toStringAsFixed(2),
                    values: const [
                      '1.00',
                      '1.10',
                      '1.18',
                      '1.20',
                      '1.30',
                      '1.40',
                      '1.50',
                      '1.60',
                    ],
                    onChanged: (value) {
                      final parsed = double.tryParse(value);
                      if (parsed != null) {
                        state._updateTerminalFont(lineHeight: parsed);
                      }
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.letterSpacing',
                  title: 'Letter Spacing',
                  subtitle: 'Extra space between characters (px).',
                  trailing: _SettingsSelect(
                    label: 'Letter Spacing',
                    showLabel: false,
                    value: terminalFontConfig.letterSpacing.toStringAsFixed(1),
                    values: const ['0.0', '0.5', '1.0', '1.5', '2.0'],
                    onChanged: (value) {
                      final parsed = double.tryParse(value);
                      if (parsed != null) {
                        state._updateTerminalFont(letterSpacing: parsed);
                      }
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.weight',
                  title: 'Weight',
                  subtitle: 'Font weight for normal text.',
                  trailing: _SettingsSelect(
                    key: const ValueKey('settings-terminal-weight-select'),
                    label: 'Weight',
                    showLabel: false,
                    value: terminalFontConfig.weight.toString(),
                    values: const ['300', '400', '500'],
                    format: _weightLabel,
                    localizeOptions: false,
                    onChanged: (value) {
                      final parsed = int.tryParse(value);
                      if (parsed != null) {
                        state._updateTerminalFont(weight: parsed);
                      }
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.boldWeight',
                  title: 'Bold Weight',
                  subtitle: 'Font weight for bold text.',
                  trailing: _SettingsSelect(
                    key: const ValueKey('settings-terminal-bold-weight-select'),
                    label: 'Bold Weight',
                    showLabel: false,
                    value: terminalFontConfig.boldWeight.toString(),
                    values: const ['600', '700', '800', '900'],
                    format: _weightLabel,
                    localizeOptions: false,
                    onChanged: (value) {
                      final parsed = int.tryParse(value);
                      if (parsed != null) {
                        state._updateTerminalFont(boldWeight: parsed);
                      }
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.ligatures',
                  title: 'Ligatures',
                  subtitle: 'Enable font ligatures (e.g. -> => !=).',
                  trailing: _SettingsSwitch(
                    value: terminalFontConfig.enableLigatures,
                    onChanged: (value) {
                      state._updateTerminalFont(enableLigatures: value);
                    },
                  ),
                ),
              ],
            ),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('terminal-shell'),
          icon: Icons.tune_rounded,
          title: 'General',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  localizationKey: 'settings.terminal.shell',
                  title: 'Shell',
                  subtitle: 'Shell used for new local terminal sessions.',
                  trailing: _SettingsSelect(
                    key: const ValueKey('settings-terminal-shell-select'),
                    label: 'Shell',
                    showLabel: false,
                    value: state._shellPath,
                    values: ['', ...state._selectableShellPaths],
                    format: _shellPathLabel,
                    onChanged: (value) {
                      state._mutate(() => state._shellPath = value);
                      terminalShellPath = value.isEmpty ? null : value;
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.emulation',
                  title: 'Emulation Type',
                  subtitle:
                      'Controls color and feature support for the session.',
                  trailing: _SettingsSelect(
                    label: 'Emulation Type',
                    showLabel: false,
                    value: state._emulationType.term,
                    values: TerminalType.values
                        .map((t) => t.term)
                        .toList(growable: false),
                    onChanged: (value) {
                      final type = TerminalType.values.firstWhere(
                        (t) => t.term == value,
                        orElse: () => TerminalType.xterm256Color,
                      );
                      state._mutate(() => state._emulationType = type);
                      terminalEmulationType = type;
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.scrollback',
                  title: 'Scrollback Lines',
                  subtitle:
                      'Maximum number of lines kept in the scroll buffer.',
                  trailing: _SettingsTextField(
                    controller: state._scrollbackLinesController,
                    onChanged: (value) {
                      final parsed = int.tryParse(value);
                      if (parsed != null && parsed > 0) {
                        state._scrollbackLines = parsed.toString();
                        terminalScrollbackLines = parsed;
                        state._persistRuntimeSettings();
                      }
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.sshKeepalive',
                  title: 'SSH Keepalive Interval',
                  subtitle:
                      'Seconds between SSH protocol keepalive packets. Set to 0 to disable.',
                  trailing: _SettingsTextField(
                    controller: state._sshKeepaliveIntervalSecondsController,
                    keyboardType: TextInputType.number,
                    inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                    onChanged: (value) {
                      final parsed = int.tryParse(value);
                      if (parsed != null &&
                          parsed >= 0 &&
                          parsed <= maxSshKeepaliveIntervalSeconds) {
                        state._sshKeepaliveIntervalSeconds = parsed.toString();
                        terminalSshKeepaliveIntervalSeconds = parsed;
                        state._persistRuntimeSettings();
                      }
                    },
                  ),
                ),
              ],
            ),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('terminal-layout'),
          icon: Icons.aspect_ratio_rounded,
          title: 'Layout',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  localizationKey: 'settings.terminal.padding',
                  title: 'Padding',
                  subtitle:
                      'Inner spacing around terminal content. Supports 1 to 4 values.',
                  trailing: _TerminalPaddingControl(
                    padding: state._terminalPadding,
                    onChanged: (value) {
                      state._mutate(() => state._terminalPadding = value);
                      terminalPadding = value;
                      terminalPaddingNotifier.value++;
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.scrollbar',
                  title: 'Scrollbar',
                  subtitle:
                      'Show a scrollbar when scrollback history is available.',
                  trailing: _SettingsSwitch(
                    value: state._scrollbarEnabled,
                    onChanged: (value) {
                      state._mutate(() => state._scrollbarEnabled = value);
                      terminalScrollbarEnabled = value;
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
              ],
            ),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('terminal-cursor'),
          icon: Icons.text_fields_rounded,
          title: 'Cursor',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  localizationKey: 'settings.terminal.cursorShape',
                  title: 'Shape',
                  subtitle: 'Cursor appearance in the terminal.',
                  trailing: _SettingsSelect(
                    label: 'Shape',
                    showLabel: false,
                    value: terminalCursorShape.name,
                    values: TerminalCursorShape.values
                        .map((s) => s.name)
                        .toList(growable: false),
                    format: _cursorShapeLabel,
                    onChanged: (value) {
                      final shape = TerminalCursorShape.values.firstWhere(
                        (s) => s.name == value,
                        orElse: () => TerminalCursorShape.block,
                      );
                      state._mutate(() => terminalCursorShape = shape);
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.cursorBlink',
                  title: 'Blink',
                  subtitle: 'Cursor blinks at regular intervals.',
                  trailing: _SettingsSwitch(
                    value: terminalCursorBlink,
                    onChanged: (value) {
                      state._mutate(() => terminalCursorBlink = value);
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
              ],
            ),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('terminal-interaction'),
          icon: Icons.content_copy_rounded,
          title: 'Interaction',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  localizationKey: 'settings.terminal.copyOnSelect',
                  title: 'Copy on Select',
                  subtitle:
                      'Automatically copy selected text to the clipboard.',
                  trailing: _SettingsSwitch(
                    value: state._copyOnSelect,
                    onChanged: (value) {
                      state._mutate(() => state._copyOnSelect = value);
                      terminalCopyOnSelect = value;
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.composer',
                  title: 'Composer',
                  subtitle: 'Show the command input bar below the terminal.',
                  trailing: _SettingsSwitch(
                    value: state._composerEnabled,
                    onChanged: (value) {
                      state._mutate(() => state._composerEnabled = value);
                      terminalComposerEnabled = value;
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.multipleTabs',
                  title: 'Multiple Tabs',
                  subtitle:
                      'Allow multiple terminal sessions in the same pane.',
                  trailing: _SettingsSwitch(
                    value: state._multiTabEnabled,
                    onChanged: (value) {
                      state._mutate(() => state._multiTabEnabled = value);
                      terminalMultiTabEnabled = value;
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
              ],
            ),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('terminal-recording'),
          icon: Icons.history_rounded,
          localizationKey: 'settings.terminal.recording.title',
          title: 'History & Storage',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  localizationKey: 'settings.terminal.recording.history',
                  title: 'Terminal History',
                  subtitle: 'Save connection history on this device.',
                  trailing: _SettingsSwitch(
                    value: terminalRecordingConfig.enabled,
                    onChanged: (value) {
                      state._mutate(() {
                        terminalRecordingConfig = terminalRecordingConfig
                            .copyWith(enabled: value);
                      });
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.recording.rawOutput',
                  title: 'Raw Terminal Output',
                  subtitle: 'Save encrypted terminal output for replay.',
                  trailing: _SettingsSwitch(
                    key: const ValueKey('settings-terminal-raw-output-switch'),
                    value: terminalRecordingConfig.captureEnabled,
                    onChanged: state._setRawTerminalCaptureEnabled,
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.recording.retention',
                  title: 'Retention Period',
                  subtitle: 'Delete completed sessions after this many days.',
                  trailing: _SettingsSelect(
                    key: const ValueKey(
                      'settings-terminal-retention-days-select',
                    ),
                    label: 'Retention Period',
                    showLabel: false,
                    value: terminalRecordingConfig.retentionDays.toString(),
                    values: const ['7', '30', '90', '180', '365'],
                    format: (value) => '$value days',
                    onChanged: (value) {
                      state._mutate(() {
                        terminalRecordingConfig = terminalRecordingConfig
                            .copyWith(retentionDays: int.parse(value));
                      });
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.recording.sessionLimit',
                  title: 'Per-session Limit',
                  subtitle:
                      'Keep the latest output when a session exceeds this size.',
                  trailing: _SettingsSelect(
                    key: const ValueKey(
                      'settings-terminal-session-limit-select',
                    ),
                    label: 'Per-session Limit',
                    showLabel: false,
                    value:
                        '${terminalRecordingConfig.maxSessionBytes ~/ (1024 * 1024)}',
                    values: const ['25', '50', '100', '250', '500'],
                    format: (value) => '$value MB',
                    onChanged: (value) {
                      state._mutate(() {
                        terminalRecordingConfig = terminalRecordingConfig
                            .copyWith(
                              maxSessionBytes: int.parse(value) * 1024 * 1024,
                            );
                      });
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.terminal.recording.storageLimit',
                  title: 'Total Storage Limit',
                  subtitle:
                      'Delete the oldest completed sessions when storage is full.',
                  trailing: _SettingsSelect(
                    key: const ValueKey('settings-terminal-total-limit-select'),
                    label: 'Total Storage Limit',
                    showLabel: false,
                    value:
                        '${terminalRecordingConfig.maxTotalBytes ~/ (1024 * 1024 * 1024)}',
                    values: const ['1', '2', '5', '10', '20'],
                    format: (value) => '$value GB',
                    onChanged: (value) {
                      state._mutate(() {
                        terminalRecordingConfig = terminalRecordingConfig
                            .copyWith(
                              maxTotalBytes:
                                  int.parse(value) * 1024 * 1024 * 1024,
                            );
                      });
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 14),
                Align(
                  alignment: Alignment.centerLeft,
                  child: _TerminalRecordingLocation(
                    directory: NautermPaths.resolve().terminalLogsDirectory,
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

class _TerminalRecordingLocation extends StatelessWidget {
  const _TerminalRecordingLocation({required this.directory});

  final Directory directory;

  Future<void> _open(BuildContext context) async {
    try {
      final override = settingsDirectoryOpenerOverride;
      if (override != null) {
        await override(directory);
        return;
      }
      await directory.create(recursive: true);
      if (Platform.isMacOS) {
        await Process.start('open', [
          directory.path,
        ], mode: ProcessStartMode.detached);
      } else if (Platform.isWindows) {
        await Process.start('explorer.exe', [
          directory.path,
        ], mode: ProcessStartMode.detached);
      } else {
        await Process.start('xdg-open', [
          directory.path,
        ], mode: ProcessStartMode.detached);
      }
    } on Object {
      if (!context.mounted) return;
      ScaffoldMessenger.maybeOf(context)?.showSnackBar(
        SnackBar(
          content: Text(
            tr(
              'settings.terminal.recording.location.openError',
              fallback: 'Unable to open location.',
            ),
          ),
        ),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(
          tr(
            'settings.terminal.recording.location.label',
            fallback: 'Location',
          ),
          style: TextStyle(color: _mutedText, fontSize: 12),
        ),
        const SizedBox(width: 6),
        Flexible(
          child: Tooltip(
            message: tr(
              'settings.terminal.recording.location.open',
              fallback: 'Open location',
            ),
            child: TextButton.icon(
              key: const ValueKey('settings-terminal-recording-location'),
              onPressed: () => unawaited(_open(context)),
              icon: const Icon(Icons.folder_open_rounded, size: 15),
              label: Text(
                directory.path,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              style: TextButton.styleFrom(
                foregroundColor: _primary,
                padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 4),
                minimumSize: const Size(0, 28),
                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                textStyle: const TextStyle(fontSize: 12),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

Widget _buildSettingsSftpContent(_SettingsPanelState state) {
  return Scrollbar(
    controller: state._contentScrollController,
    thumbVisibility: true,
    child: ListView(
      scrollCacheExtent: const ScrollCacheExtent.viewport(100),
      controller: state._contentScrollController,
      padding: state._contentPadding,
      children: [
        Text(
          tr('settings.pages.sftp.title', fallback: 'SFTP'),
          style: TextStyle(
            color: _text,
            fontSize: 20,
            height: 1.4,
            fontWeight: NautermFontWeights.semibold,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 3),
        Text(
          tr(
            'settings.pages.sftp.description',
            fallback: 'Configure file transfer and editor behavior.',
          ),
          style: TextStyle(
            color: _mutedText,
            fontSize: 12,
            fontWeight: NautermFontWeights.regular,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('sftp-workspace'),
          icon: Icons.tab_rounded,
          title: 'Workspace',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: _SettingsRow(
              localizationKey: 'settings.sftp.workspaceTab',
              title: 'SFTP Tab',
              subtitle: 'Show the dedicated SFTP tab in the workspace bar.',
              trailing: _SettingsSwitch(
                value: state._sftpTabEnabled,
                onChanged: (value) {
                  state._mutate(() => state._sftpTabEnabled = value);
                  setSftpTabEnabled(value);
                  state._persistRuntimeSettings();
                },
              ),
            ),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('sftp-editors'),
          icon: Icons.cloud_sync_rounded,
          title: 'Editors',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  localizationKey: 'settings.sftp.sshEditor',
                  title: 'SSH Editor',
                  subtitle:
                      'Terminal editor for remote files (e.g. vim, nano).',
                  trailing: _SettingsTextField(
                    key: const ValueKey('settings-sftp-ssh-editor-input'),
                    controller: state._sshEditorController,
                    onChanged: (value) {
                      sftpSshEditor = value.trim();
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _buildSettingsExternalEditorSelect(state),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.sftp.textExtensions',
                  title: 'Text File Extensions',
                  subtitle:
                      'Extensions that show the configured external editor.',
                  trailing: _SettingsTextField(
                    controller: state._sftpTextFileExtensionsController,
                    hint: 'txt, md, json, yaml, sh, py',
                    onChanged: (_) {},
                    onFocusLost: (value) {
                      final extensions = parseSftpTextFileExtensions(value);
                      state._sftpTextFileExtensions = extensions;
                      sftpTextFileExtensions = extensions;
                      state._sftpTextFileExtensionsController.text = extensions
                          .join(', ');
                      state._persistRuntimeSettings();
                      if (state.widget.detectExternalEditors) {
                        unawaited(state._detectEditors());
                      }
                    },
                  ),
                ),
              ],
            ),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('sftp-files'),
          icon: Icons.folder_outlined,
          title: 'Files',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  localizationKey: 'settings.sftp.defaultDirectory',
                  title: 'Default Directory',
                  subtitle: 'Initial local directory when opening SFTP.',
                  trailing: _SettingsTextField(
                    controller: state._sftpDownloadDirController,
                    hint: defaultDownloadDirectory(),
                    onChanged: (value) {
                      state._sftpDefaultDownloadDir = value.trim();
                      sftpDefaultDownloadDir =
                          state._sftpDefaultDownloadDir.isNotEmpty
                          ? state._sftpDefaultDownloadDir
                          : null;
                      state._persistRuntimeSettings();
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.sftp.showHiddenFiles',
                  title: 'Show Hidden Files',
                  subtitle: 'Display dotfiles by default in file listings.',
                  trailing: _SettingsSwitch(
                    value: state._sftpShowHiddenFiles,
                    onChanged: (value) {
                      state._mutate(() => state._sftpShowHiddenFiles = value);
                      sftpShowHiddenFiles = value;
                      state._persistRuntimeSettings();
                    },
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

Widget _buildSettingsShortcutsContent(_SettingsPanelState state) {
  return Scrollbar(
    controller: state._contentScrollController,
    thumbVisibility: true,
    child: ListView(
      scrollCacheExtent: const ScrollCacheExtent.viewport(100),
      controller: state._contentScrollController,
      padding: state._contentPadding,
      children: [
        Text(
          tr('settings.pages.shortcuts.title', fallback: 'Shortcuts'),
          style: TextStyle(
            color: _text,
            fontSize: 20,
            height: 1.4,
            fontWeight: NautermFontWeights.semibold,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 3),
        Text(
          tr(
            'settings.pages.shortcuts.description',
            fallback:
                'Configure terminal keyboard behavior and review active bindings.',
          ),
          style: TextStyle(
            color: _mutedText,
            fontSize: 12,
            fontWeight: NautermFontWeights.regular,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('shortcuts-keyboard'),
          icon: Icons.keyboard_command_key_rounded,
          title: 'Keyboard Behavior',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  localizationKey: 'settings.shortcuts.optionAsMeta',
                  title: 'Use Option as Meta Key',
                  subtitle:
                      'Send Option key combinations with an Escape prefix.',
                  trailing: _SettingsSwitch(
                    value: state._useOptionAsMetaKey,
                    onChanged: (value) {
                      state._updateKeyboardSettings(
                        terminalKeyboardConfig.copyWith(
                          useOptionAsMetaKey: value,
                        ),
                      );
                    },
                  ),
                ),
              ],
            ),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('shortcuts-bindings'),
          icon: Icons.keyboard_rounded,
          title: 'Key Bindings',
          trailing: _ShortcutResetAllButton(
            onPressed: () =>
                state._updateShortcutSettings(const TerminalShortcutConfig()),
          ),
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: _ShortcutBindingsTable(
              config: state._shortcutConfig,
              onChanged: state._updateShortcutSettings,
            ),
          ),
        ),
      ],
    ),
  );
}

Widget _buildSettingsAiContent(_SettingsPanelState state) {
  return Scrollbar(
    controller: state._contentScrollController,
    thumbVisibility: true,
    child: ListView(
      scrollCacheExtent: const ScrollCacheExtent.viewport(100),
      controller: state._contentScrollController,
      padding: state._contentPadding,
      children: [
        Text(
          tr(
            'settings.pages.ai.title',
            fallback: 'AI Assistant (Alpha Preview)',
          ),
          style: TextStyle(
            color: _text,
            fontSize: 20,
            height: 1.4,
            fontWeight: NautermFontWeights.semibold,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 3),
        Text(
          tr(
            'settings.pages.ai.description',
            fallback: 'Configure AI providers and terminal context.',
          ),
          style: TextStyle(
            color: _mutedText,
            fontSize: 12,
            fontWeight: NautermFontWeights.regular,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('ai-providers'),
          icon: LucideIcons.sparkles,
          title: 'Providers',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: _AiProvidersEditor(
              providers: state._aiProviders,
              selectedIndex: state._selectedAiProviderIndex,
              onProviderSelected: state._selectAiProvider,
              onProviderAdded: state._addAiProvider,
              onProviderDeleted: state._deleteAiProvider,
              onProviderSetActive: state._setActiveAiProvider,
              onFieldChanged: state._updateSelectedAiProviderField,
              onPresetsRefresh: state._refreshAiPresets,
              presetsRefreshing: state._aiPresetsRefreshing,
              presetsStatus: state._aiPresetsStatus,
            ),
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          key: state._settingsSectionKey('ai-context'),
          icon: LucideIcons.squareTerminal,
          title: 'Terminal Context',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  localizationKey: 'settings.ai.terminalSelection',
                  title: 'Terminal Selection',
                  subtitle: 'Include selected terminal text in AI Assistant.',
                  trailing: _SettingsSwitch(
                    value: state._includeTerminalSelection,
                    onChanged: (value) {
                      state._mutate(
                        () => state._includeTerminalSelection = value,
                      );
                      state._updateAiSettings(
                        aiAssistantConfig.copyWith(
                          includeTerminalSelection: value,
                        ),
                        providerChanged: false,
                      );
                    },
                  ),
                ),
                SizedBox(height: 18),
                _SettingsRow(
                  localizationKey: 'settings.ai.recentOutput',
                  title: 'Recent Output',
                  subtitle: 'Include recent output from the active terminal.',
                  trailing: _SettingsSwitch(
                    value: state._includeRecentTerminalOutput,
                    onChanged: (value) {
                      state._mutate(
                        () => state._includeRecentTerminalOutput = value,
                      );
                      state._updateAiSettings(
                        aiAssistantConfig.copyWith(
                          includeRecentTerminalOutput: value,
                        ),
                        providerChanged: false,
                      );
                    },
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

String _weightLabel(String value) {
  return switch (value) {
    '100' => 'Thin',
    '200' => 'Extra Light',
    '300' => 'Light',
    '400' => 'Regular',
    '500' => 'Medium',
    '600' => 'Semi Bold',
    '700' => 'Bold',
    '800' => 'Extra Bold',
    '900' => 'Black',
    _ => value,
  };
}

String _cursorShapeLabel(String value) {
  return switch (value) {
    'block' => 'Block',
    'underline' => 'Underline',
    'beam' => 'Beam',
    'hollowBlock' => 'Hollow Block',
    _ => value,
  };
}

String _shellPathLabel(String value) {
  if (value.isEmpty) {
    final path = systemDefaultShellPath();
    if (path == null) return 'System Default';
    return 'System Default — ${shellDisplayName(path)} ($path)';
  }
  return '${shellDisplayName(value)} — $value';
}

String _normalizedSettingsShellPath(String? value) {
  final path = value?.trim() ?? '';
  return path == systemDefaultShellPath() ? '' : path;
}

String _hostIconModeLabel(String value) {
  return switch (value) {
    'defaultIcon' => 'Default',
    'osBadge' => 'With OS Badge',
    'osIcon' => 'OS Icon',
    _ => value,
  };
}
