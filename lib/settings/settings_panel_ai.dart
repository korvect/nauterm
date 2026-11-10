part of 'settings_panel.dart';

const _aiProviderButtonShape = RoundedRectangleBorder(
  borderRadius: BorderRadius.all(Radius.circular(6)),
);

class _AiProvidersEditor extends StatelessWidget {
  const _AiProvidersEditor({
    required this.providers,
    required this.onProviderSelected,
    required this.onProviderAdded,
    required this.onProviderDeleted,
    required this.onProviderSetActive,
    required this.onPresetsRefresh,
    required this.presetsRefreshing,
    this.presetsStatus,
    this.error,
  });

  final List<AiProviderEntry> providers;
  final ValueChanged<int> onProviderSelected;
  final VoidCallback onProviderAdded;
  final ValueChanged<int> onProviderDeleted;
  final ValueChanged<int> onProviderSetActive;
  final Future<void> Function() onPresetsRefresh;
  final bool presetsRefreshing;
  final String? presetsStatus;
  final String? error;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Container(
          clipBehavior: Clip.antiAlias,
          decoration: BoxDecoration(
            color: _surface,
            border: Border.all(color: _softOutline),
            borderRadius: BorderRadius.circular(9),
          ),
          child: Column(
            children: [
              if (providers.isEmpty)
                Padding(
                  padding: const EdgeInsets.all(22),
                  child: Text(
                    tr(
                      'settings.ai.providers.empty',
                      fallback:
                          'No AI providers yet. Add one to use the assistant.',
                    ),
                    style: TextStyle(color: _mutedText, fontSize: 12),
                  ),
                ),
              for (var i = 0; i < providers.length; i++)
                _AiProviderRow(
                  provider: providers[i],
                  onTap: () => onProviderSelected(i),
                  onSetActive: () => onProviderSetActive(i),
                  onDelete: () => onProviderDeleted(i),
                ),
              Container(
                padding: const EdgeInsets.fromLTRB(8, 4, 12, 4),
                decoration: BoxDecoration(
                  color: _surfaceContainer,
                  border: Border(top: BorderSide(color: _softOutline)),
                ),
                child: Row(
                  children: [
                    TextButton.icon(
                      key: const ValueKey('settings-ai-add-provider'),
                      onPressed: onProviderAdded,
                      icon: const Icon(LucideIcons.plus, size: 14),
                      label: Text(
                        tr(
                          'settings.label.addProvider',
                          fallback: 'Add Provider',
                        ),
                      ),
                      style: TextButton.styleFrom(
                        shape: _aiProviderButtonShape,
                        foregroundColor: _primary,
                        textStyle: const TextStyle(fontSize: 12),
                      ),
                    ),
                    const Spacer(),
                    if (presetsRefreshing)
                      Padding(
                        padding: const EdgeInsets.all(6),
                        child: SizedBox.square(
                          dimension: 14,
                          child: CircularProgressIndicator(
                            strokeWidth: 2,
                            color: _primary,
                          ),
                        ),
                      )
                    else
                      _aiProviderAction(
                        tooltip: tr(
                          'settings.label.updatePresets',
                          fallback: 'Update Presets',
                        ),
                        icon: LucideIcons.download,
                        onPressed: () => unawaited(onPresetsRefresh()),
                      ),
                  ],
                ),
              ),
            ],
          ),
        ),
        if (error ?? presetsStatus case final status?)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text(
              tr(status),
              style: TextStyle(
                color: error == null ? _mutedText : Colors.red,
                fontSize: 11,
              ),
            ),
          ),
      ],
    );
  }
}

Widget _aiProviderAction({
  required String tooltip,
  required IconData icon,
  required VoidCallback onPressed,
  Color? color,
}) => IconButton(
  tooltip: tooltip,
  onPressed: onPressed,
  icon: Icon(icon, size: 14),
  color: color ?? _primary,
  padding: EdgeInsets.zero,
  constraints: const BoxConstraints.tightFor(width: 28, height: 28),
  visualDensity: VisualDensity.compact,
  style: IconButton.styleFrom(shape: _aiProviderButtonShape),
);

class _AiProviderRow extends StatelessWidget {
  const _AiProviderRow({
    required this.provider,
    required this.onTap,
    required this.onSetActive,
    required this.onDelete,
  });

  final AiProviderEntry provider;
  final VoidCallback onTap;
  final VoidCallback onSetActive;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final protocol = AiApiProtocol.fromString(provider.protocol);
    final host = Uri.tryParse(provider.baseUrl)?.host;
    final detail = [
      protocol.label,
      if (provider.model.isNotEmpty) provider.model,
      if (host != null && host.isNotEmpty) host,
    ].join(' · ');
    final tint = provider.active ? _primary : _mutedText;
    return InkWell(
      key: ValueKey('settings-ai-provider-${provider.uuid}'),
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        decoration: BoxDecoration(
          border: Border(bottom: BorderSide(color: _softOutline)),
        ),
        child: Row(
          children: [
            Container(
              width: 28,
              height: 28,
              decoration: BoxDecoration(
                color: tint.withValues(alpha: 0.1),
                borderRadius: BorderRadius.circular(7),
              ),
              child: Icon(LucideIcons.sparkles, size: 14, color: tint),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    provider.name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: _text,
                      fontSize: 13,
                      fontWeight: NautermFontWeights.medium,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    detail,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(color: _mutedText, fontSize: 11),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            if (provider.active)
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  border: Border.all(color: _primary),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  tr('settings.ai.provider.active', fallback: 'Active'),
                  style: TextStyle(color: _primary, fontSize: 10),
                ),
              )
            else
              TextButton(
                onPressed: onSetActive,
                style: TextButton.styleFrom(
                  shape: _aiProviderButtonShape,
                  foregroundColor: _primary,
                  minimumSize: const Size(36, 28),
                  padding: const EdgeInsets.symmetric(horizontal: 6),
                  textStyle: const TextStyle(fontSize: 12),
                ),
                child: Text(tr('settings.ai.provider.use', fallback: 'Use')),
              ),
            const SizedBox(width: 14),
            _aiProviderAction(
              tooltip: tr(
                'settings.ai.provider.edit',
                fallback: 'Edit provider',
              ),
              icon: LucideIcons.pencil,
              onPressed: onTap,
            ),
            const SizedBox(width: 4),
            _aiProviderAction(
              tooltip: tr(
                'settings.ai.provider.delete',
                fallback: 'Delete provider',
              ),
              icon: LucideIcons.trash2,
              color: const Color(0xffef4444),
              onPressed: onDelete,
            ),
          ],
        ),
      ),
    );
  }
}

class _AiProviderEditor extends StatefulWidget {
  const _AiProviderEditor({required this.provider, required this.onSave});

  final AiProviderEntry provider;
  final void Function(String name, AiAssistantConfig config) onSave;

  @override
  State<_AiProviderEditor> createState() => _AiProviderEditorState();
}

class _AiProviderEditorState extends State<_AiProviderEditor> {
  late final TextEditingController _name = TextEditingController(
    text: widget.provider.name,
  );
  late final TextEditingController _baseUrl = TextEditingController(
    text: widget.provider.baseUrl,
  );
  late final TextEditingController _models = TextEditingController(
    text: widget.provider.model,
  );
  late final TextEditingController _apiKey = TextEditingController(
    text: widget.provider.apiKey,
  );
  late final TextEditingController _maxTokens = TextEditingController(
    text: widget.provider.maxTokens?.toString() ?? '',
  );
  late final TextEditingController _temperature = TextEditingController(
    text: widget.provider.temperature?.toString() ?? '',
  );
  late AiApiProtocol _protocol = AiApiProtocol.fromString(
    widget.provider.protocol,
  );
  bool _advanced = false;
  bool _dirty = false;
  String? _error;

  @override
  void dispose() {
    for (final controller in [
      _name,
      _baseUrl,
      _models,
      _apiKey,
      _maxTokens,
      _temperature,
    ]) {
      controller.dispose();
    }
    super.dispose();
  }

  void _changed(String _) => setState(() {
    _dirty = true;
    _error = null;
  });

  void _save() {
    final uri = Uri.tryParse(_baseUrl.text.trim());
    final tokensText = _maxTokens.text.trim();
    final tokens = int.tryParse(tokensText);
    final temperatureText = _temperature.text.trim();
    final temperature = double.tryParse(temperatureText);
    final error = _name.text.trim().isEmpty
        ? 'settings.ai.provider.error.name'
        : uri == null ||
              !['http', 'https'].contains(uri.scheme) ||
              uri.host.isEmpty
        ? 'settings.ai.provider.error.baseUrl'
        : tokensText.isNotEmpty && (tokens == null || tokens <= 0)
        ? 'settings.ai.provider.error.maxTokens'
        : temperatureText.isNotEmpty &&
              (temperature == null ||
                  !temperature.isFinite ||
                  temperature < 0 ||
                  temperature > 1)
        ? 'settings.ai.provider.error.temperature'
        : null;
    if (error != null) {
      setState(() => _error = tr(error));
      return;
    }
    try {
      widget.onSave(
        _name.text.trim(),
        AiAssistantConfig(
          protocol: _protocol,
          baseUrl: _baseUrl.text.trim(),
          model: _models.text.trim(),
          apiKey: _apiKey.text,
          maxTokens: tokens,
          temperature: temperature,
          includeTerminalSelection: aiAssistantConfig.includeTerminalSelection,
          includeRecentTerminalOutput:
              aiAssistantConfig.includeRecentTerminalOutput,
        ),
      );
      Navigator.of(context).pop();
    } on Object {
      setState(
        () => _error = tr(
          'settings.ai.provider.error.save',
          fallback: 'Unable to save the AI provider.',
        ),
      );
    }
  }

  Widget _field(
    String id,
    String label,
    TextEditingController controller, {
    bool secret = false,
    String? hint,
  }) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          tr('settings.ai.provider.$id.label', fallback: label),
          style: TextStyle(
            color: _text,
            fontSize: 12,
            fontWeight: NautermFontWeights.medium,
          ),
        ),
        const SizedBox(height: 5),
        _SettingsTextField(
          key: ValueKey('settings-ai-$id-field'),
          controller: controller,
          onChanged: _changed,
          obscureText: secret,
          revealable: secret,
        ),
        if (hint != null) ...[
          const SizedBox(height: 4),
          Text(tr(hint), style: TextStyle(color: _mutedText, fontSize: 11)),
        ],
      ],
    );
  }

  Widget _pair(Widget first, Widget second) => LayoutBuilder(
    builder: (context, constraints) {
      if (constraints.maxWidth < 400) {
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [first, const SizedBox(height: 12), second],
        );
      }
      return Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(child: first),
          const SizedBox(width: 10),
          Expanded(child: second),
        ],
      );
    },
  );

  @override
  Widget build(BuildContext context) {
    return _AiProviderDialogFrame(
      title: widget.provider.id == null
          ? tr('settings.label.addAiProvider', fallback: 'Add AI Provider')
          : tr('settings.ai.provider.editTitle', fallback: 'Edit Provider'),
      actions: [
        if (_dirty)
          Expanded(
            child: Text(
              tr('settings.ai.provider.unsaved', fallback: 'Unsaved changes'),
              style: TextStyle(color: _mutedText, fontSize: 11),
            ),
          )
        else
          const Spacer(),
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          style: TextButton.styleFrom(shape: _aiProviderButtonShape),
          child: Text(tr('common.action.cancel', fallback: 'Cancel')),
        ),
        const SizedBox(width: 8),
        FilledButton(
          key: const ValueKey('settings-ai-save-provider'),
          onPressed: _save,
          style: FilledButton.styleFrom(
            backgroundColor: _primary,
            shape: _aiProviderButtonShape,
          ),
          child: Text(tr('common.action.save', fallback: 'Save')),
        ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _pair(
            _field('name', 'Name', _name),
            Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  tr(
                    'settings.ai.provider.protocol.label',
                    fallback: 'Protocol',
                  ),
                  style: TextStyle(
                    color: _text,
                    fontSize: 12,
                    fontWeight: NautermFontWeights.medium,
                  ),
                ),
                const SizedBox(height: 5),
                _SettingsSelect(
                  key: const ValueKey('settings-ai-protocol-select'),
                  label: 'Protocol',
                  showLabel: false,
                  value: _protocol.storageValue,
                  values: AiApiProtocol.values
                      .map((p) => p.storageValue)
                      .toList(),
                  format: (value) => AiApiProtocol.fromString(value).label,
                  onChanged: (value) {
                    final next = AiApiProtocol.fromString(value);
                    if (_usesDefaultAiBaseUrl(_baseUrl.text, _protocol)) {
                      _baseUrl.text = next.defaultBaseUrl;
                    }
                    setState(() {
                      _protocol = next;
                      _dirty = true;
                      _error = null;
                    });
                  },
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
          _field('baseUrl', 'Base URL', _baseUrl),
          const SizedBox(height: 12),
          _field('apiKey', 'API Key', _apiKey, secret: true),
          const SizedBox(height: 12),
          _field(
            'models',
            'Models',
            _models,
            hint: 'settings.ai.provider.models.hint',
          ),
          const SizedBox(height: 8),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              key: const ValueKey('settings-ai-advanced'),
              onPressed: () => setState(() => _advanced = !_advanced),
              icon: AnimatedRotation(
                turns: _advanced ? .25 : 0,
                duration: const Duration(milliseconds: 150),
                child: Icon(
                  LucideIcons.chevronRight,
                  size: 14,
                  color: _mutedText,
                ),
              ),
              label: Text(tr('workspace.label.advanced', fallback: 'Advanced')),
              style: TextButton.styleFrom(
                shape: _aiProviderButtonShape,
                foregroundColor: _mutedText,
                padding: EdgeInsets.zero,
                textStyle: const TextStyle(fontSize: 12),
              ),
            ),
          ),
          AnimatedSize(
            duration: const Duration(milliseconds: 180),
            alignment: Alignment.topCenter,
            child: _advanced
                ? Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: _pair(
                      _field(
                        'maxTokens',
                        'Max Tokens',
                        _maxTokens,
                        hint: 'settings.ai.provider.maxTokens.hint',
                      ),
                      _field(
                        'temperature',
                        'Temperature',
                        _temperature,
                        hint: 'settings.ai.provider.temperature.hint',
                      ),
                    ),
                  )
                : const SizedBox.shrink(),
          ),
          if (_error case final error?)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: Text(
                error,
                key: const ValueKey('settings-ai-provider-error'),
                style: const TextStyle(color: Color(0xffef4444), fontSize: 12),
              ),
            ),
        ],
      ),
    );
  }
}

class _AiProviderDialogFrame extends StatelessWidget {
  const _AiProviderDialogFrame({
    required this.title,
    required this.child,
    required this.actions,
    this.width = 520,
  });
  final String title;
  final Widget child;
  final List<Widget> actions;
  final double width;

  @override
  Widget build(BuildContext context) => Dialog(
    backgroundColor: _surface,
    surfaceTintColor: Colors.transparent,
    clipBehavior: Clip.antiAlias,
    insetPadding: const EdgeInsets.all(24),
    shape: RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(12),
      side: BorderSide(color: _softOutline),
    ),
    child: ConstrainedBox(
      constraints: BoxConstraints(
        maxWidth: width,
        maxHeight: math.max(0, MediaQuery.sizeOf(context).height - 48),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            padding: const EdgeInsets.fromLTRB(18, 10, 8, 10),
            decoration: BoxDecoration(
              color: _surfaceContainer,
              border: Border(bottom: BorderSide(color: _softOutline)),
            ),
            child: Row(
              children: [
                Icon(LucideIcons.sparkles, size: 16, color: _primary),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    title,
                    style: TextStyle(
                      color: _text,
                      fontSize: 14,
                      fontWeight: NautermFontWeights.semibold,
                    ),
                  ),
                ),
                _aiProviderAction(
                  tooltip: tr('common.action.close', fallback: 'Close'),
                  icon: LucideIcons.x,
                  color: _mutedText,
                  onPressed: () => Navigator.of(context).pop(),
                ),
              ],
            ),
          ),
          Flexible(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(18),
              child: child,
            ),
          ),
          Container(
            padding: const EdgeInsets.fromLTRB(18, 10, 18, 12),
            decoration: BoxDecoration(
              border: Border(top: BorderSide(color: _softOutline)),
            ),
            child: Row(children: actions),
          ),
        ],
      ),
    ),
  );
}

class _AiPresetDialog extends StatefulWidget {
  const _AiPresetDialog({required this.presets});

  final Map<String, AiProviderPreset> presets;

  @override
  State<_AiPresetDialog> createState() => _AiPresetDialogState();
}

class _AiPresetDialogResult {
  const _AiPresetDialogResult(this.preset);

  final AiProviderPreset? preset;
}

class _AiPresetDialogState extends State<_AiPresetDialog> {
  String _searchQuery = '';
  late TextEditingController _searchController;

  @override
  void initState() {
    super.initState();
    _searchController = TextEditingController();
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  List<MapEntry<String, AiProviderPreset>> get _filteredPresets {
    final entries = widget.presets.entries.toList();
    if (_searchQuery.isEmpty) {
      return entries;
    }
    final query = _searchQuery.toLowerCase();
    return entries.where((entry) {
      return entry.value.name.toLowerCase().contains(query) ||
          entry.key.toLowerCase().contains(query);
    }).toList();
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: Colors.transparent,
      surfaceTintColor: Colors.transparent,
      insetPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 24),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 520, maxHeight: 580),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: _surface,
            borderRadius: BorderRadius.circular(12),
            border: Border.all(color: _softOutline),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(
                  alpha: _settingsDark ? 0.38 : 0.14,
                ),
                blurRadius: 24,
                offset: const Offset(0, 14),
              ),
            ],
          ),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(12),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Container(
                  padding: const EdgeInsets.fromLTRB(15, 10, 8, 9),
                  decoration: BoxDecoration(
                    color: _surfaceContainer,
                    border: Border(bottom: BorderSide(color: _softOutline)),
                  ),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Container(
                        width: 28,
                        height: 28,
                        decoration: BoxDecoration(
                          color: _primary.withValues(alpha: 0.09),
                          borderRadius: BorderRadius.circular(7),
                        ),
                        alignment: Alignment.center,
                        child: Icon(LucideIcons.bot, size: 14, color: _primary),
                      ),
                      SizedBox(width: 9),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              tr(
                                'settings.label.addAiProvider',
                                fallback: 'Add AI Provider',
                              ),
                              style: TextStyle(
                                fontSize: 14,
                                fontWeight: NautermFontWeights.semibold,
                                color: _text,
                                letterSpacing: 0,
                              ),
                            ),
                            SizedBox(height: 1),
                            Text(
                              tr(
                                'settings.ai.providerPreset.description',
                                fallback: 'Select a provider preset or create a blank one.',
                              ),
                              style: TextStyle(
                                fontSize: 10,
                                color: _mutedText,
                                fontWeight: NautermFontWeights.regular,
                              ),
                            ),
                          ],
                        ),
                      ),
                      IconButton(
                        tooltip: tr('common.action.close', fallback: 'Close'),
                        onPressed: () => Navigator.of(context).pop(),
                        icon: Icon(LucideIcons.x, size: 15),
                        color: _mutedText,
                        visualDensity: VisualDensity.compact,
                        style: IconButton.styleFrom(
                          hoverColor: _softOutline,
                          highlightColor: _softOutline,
                        ),
                      ),
                    ],
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(18, 16, 18, 12),
                  child: TextField(
                    controller: _searchController,
                    style: TextStyle(color: _text, fontSize: 13),
                    cursorColor: _primary,
                    decoration: InputDecoration(
                      hintText: tr(
                        'settings.search.providers.hint',
                        fallback: 'Search providers...',
                      ),
                      hintStyle: TextStyle(color: _faintText, fontSize: 13),
                      prefixIcon: Icon(
                        Icons.search,
                        size: 18,
                        color: _faintText,
                      ),
                      isDense: true,
                      filled: true,
                      fillColor: _surface,
                      contentPadding: const EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: 10,
                      ),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide: BorderSide(color: _softOutline),
                      ),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide: BorderSide(color: _primary),
                      ),
                    ),
                    onChanged: (value) => setState(() => _searchQuery = value),
                  ),
                ),
                Flexible(
                  child: _filteredPresets.isEmpty
                      ? Center(
                          child: Padding(
                            padding: const EdgeInsets.all(24),
                            child: Text(
                              tr(
                                'settings.ai.providerPreset.noMatches',
                                fallback: 'No matching presets.',
                              ),
                              style: TextStyle(color: _mutedText, fontSize: 12),
                            ),
                          ),
                        )
                      : ListView.separated(
                          padding: const EdgeInsets.fromLTRB(10, 0, 10, 10),
                          shrinkWrap: true,
                          itemCount: _filteredPresets.length,
                          separatorBuilder: (_, _) => Divider(
                            height: 1,
                            color: _softOutline.withValues(alpha: 0.7),
                          ),
                          itemBuilder: (context, index) {
                            final entry = _filteredPresets[index];
                            return _AiPresetRow(
                              preset: entry.value,
                              onTap: () =>
                                  Navigator.of(context)
                                      .pop(_AiPresetDialogResult(entry.value)),
                            );
                          },
                        ),
                ),
                Container(
                  padding: const EdgeInsets.fromLTRB(18, 10, 18, 12),
                  decoration: BoxDecoration(
                    border: Border(top: BorderSide(color: _softOutline)),
                  ),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      OutlinedButton(
                        onPressed: () =>
                            Navigator.of(context)
                                .pop(const _AiPresetDialogResult(null)),
                        style: _settingsOutlinedButtonStyle().copyWith(
                          foregroundColor: WidgetStatePropertyAll(_text),
                        ),
                        child: Text(
                          tr(
                            'settings.label.createBlankProvider',
                            fallback: 'Create Blank Provider',
                          ),
                        ),
                      ),
                    ],
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

class _AiPresetRow extends StatelessWidget {
  const _AiPresetRow({required this.preset, required this.onTap});

  final AiProviderPreset preset;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    preset.name,
                    style: TextStyle(
                      fontSize: 15,
                      fontWeight: NautermFontWeights.medium,
                      color: _text,
                    ),
                  ),
                  if (preset.defaultModels.isNotEmpty)
                    Text(
                      preset.defaultModels.join(', '),
                      style: TextStyle(fontSize: 12, color: _mutedText),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                ],
              ),
            ),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
              decoration: BoxDecoration(
                color: _surfaceContainer,
                borderRadius: BorderRadius.circular(4),
                border: Border.all(color: _softOutline),
              ),
              child: Text(
                preset.protocol.toUpperCase(),
                style: TextStyle(
                  fontSize: 10,
                  fontWeight: NautermFontWeights.medium,
                  color: _mutedText,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
