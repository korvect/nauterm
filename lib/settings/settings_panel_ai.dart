part of 'settings_panel.dart';

class _AiProvidersEditor extends StatelessWidget {
  const _AiProvidersEditor({
    required this.providers,
    required this.selectedIndex,
    required this.onProviderSelected,
    required this.onProviderAdded,
    required this.onProviderDeleted,
    required this.onProviderSetActive,
    required this.onFieldChanged,
    required this.onPresetsRefresh,
    required this.presetsRefreshing,
    this.presetsStatus,
  });

  final List<AiProviderEntry> providers;
  final int selectedIndex;
  final ValueChanged<int> onProviderSelected;
  final VoidCallback onProviderAdded;
  final ValueChanged<int> onProviderDeleted;
  final ValueChanged<int> onProviderSetActive;
  final Future<void> Function() onPresetsRefresh;
  final bool presetsRefreshing;
  final String? presetsStatus;
  final void Function({
    String? name,
    String? protocol,
    String? baseUrl,
    String? model,
    String? apiKey,
    int? maxTokens,
    bool clearMaxTokens,
    double? temperature,
    bool clearTemperature,
  })
  onFieldChanged;

  @override
  Widget build(BuildContext context) {
    final hasSelection = selectedIndex >= 0 && selectedIndex < providers.length;
    final selected = hasSelection ? providers[selectedIndex] : null;

    return Column(
      children: [
        Container(
          decoration: BoxDecoration(
            color: _surface,
            border: Border.all(color: _softOutline),
            borderRadius: BorderRadius.circular(9),
          ),
          child: Column(
            children: [
              for (var i = 0; i < providers.length; i++)
                _AiProviderRow(
                  provider: providers[i],
                  selected: i == selectedIndex,
                  onTap: () => onProviderSelected(i),
                  onSetActive: () => onProviderSetActive(i),
                  onDelete: providers.length > 1
                      ? () => onProviderDeleted(i)
                      : null,
                ),
            ],
          ),
        ),
        SizedBox(height: 12),
        Row(
          children: [
            Expanded(
              child: OutlinedButton.icon(
                onPressed: onProviderAdded,
                icon: Icon(LucideIcons.plus, size: 16),
                label: Text(
                  tr('settings.label.addProvider', fallback: 'Add Provider'),
                ),
                style: _settingsOutlinedButtonStyle(),
              ),
            ),
            SizedBox(width: 10),
            OutlinedButton.icon(
              onPressed: presetsRefreshing
                  ? null
                  : () => unawaited(onPresetsRefresh()),
              icon: presetsRefreshing
                  ? SizedBox.square(
                      dimension: 14,
                      child: CircularProgressIndicator(
                        strokeWidth: 2,
                        color: _primary,
                      ),
                    )
                  : Icon(LucideIcons.refreshCw, size: 15),
              label: Text(
                tr('settings.label.updatePresets', fallback: 'Update Presets'),
              ),
              style: _settingsOutlinedButtonStyle(),
            ),
          ],
        ),
        if (presetsStatus case final status?) ...[
          SizedBox(height: 8),
          Align(
            alignment: Alignment.centerLeft,
            child: Text(
              tr(status),
              style: TextStyle(color: _mutedText, fontSize: 11),
            ),
          ),
        ],
        if (selected != null) ...[
          SizedBox(height: 24),
          _AiProviderEditor(provider: selected, onFieldChanged: onFieldChanged),
        ],
      ],
    );
  }
}

class _AiProviderRow extends StatelessWidget {
  const _AiProviderRow({
    required this.provider,
    required this.selected,
    required this.onTap,
    required this.onSetActive,
    this.onDelete,
  });

  final AiProviderEntry provider;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onSetActive;
  final VoidCallback? onDelete;

  @override
  Widget build(BuildContext context) {
    final protocol = AiApiProtocol.fromString(provider.protocol);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        decoration: BoxDecoration(
          color: selected ? _settingsFieldHover : Colors.transparent,
          border: Border(
            bottom: BorderSide(color: _softOutline.withValues(alpha: 0.5)),
          ),
        ),
        child: Row(
          children: [
            GestureDetector(
              onTap: onSetActive,
              child: Container(
                width: 18,
                height: 18,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: provider.active ? _primary : Colors.transparent,
                  border: Border.all(
                    color: provider.active ? _primary : _softOutline,
                    width: 2,
                  ),
                ),
                child: provider.active
                    ? Icon(Icons.check, size: 12, color: Colors.white)
                    : null,
              ),
            ),
            SizedBox(width: 10),
            Expanded(
              child: Text(
                provider.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: selected ? _text : _mutedText,
                  fontSize: 13,
                  fontWeight: selected
                      ? NautermFontWeights.semibold
                      : NautermFontWeights.medium,
                ),
              ),
            ),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
              decoration: BoxDecoration(
                color: protocol == AiApiProtocol.openAi
                    ? const Color(0xffe8f5e9)
                    : const Color(0xfff3e5f5),
                borderRadius: BorderRadius.circular(4),
              ),
              child: Text(
                protocol.label,
                style: TextStyle(
                  fontSize: 10,
                  fontWeight: NautermFontWeights.medium,
                  color: protocol == AiApiProtocol.openAi
                      ? const Color(0xff2e7d32)
                      : const Color(0xff7b1fa2),
                ),
              ),
            ),
            if (onDelete != null) ...[
              SizedBox(width: 8),
              GestureDetector(
                onTap: onDelete,
                child: Icon(
                  LucideIcons.trash2,
                  size: 14,
                  color: Color(0xff9ca3af),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _AiProviderEditor extends StatefulWidget {
  const _AiProviderEditor({
    required this.provider,
    required this.onFieldChanged,
  });

  final AiProviderEntry provider;
  final void Function({
    String? name,
    String? protocol,
    String? baseUrl,
    String? model,
    String? apiKey,
    int? maxTokens,
    bool clearMaxTokens,
    double? temperature,
    bool clearTemperature,
  })
  onFieldChanged;

  @override
  State<_AiProviderEditor> createState() => _AiProviderEditorState();
}

class _AiProviderEditorState extends State<_AiProviderEditor> {
  late TextEditingController _nameController;
  late TextEditingController _baseUrlController;
  late TextEditingController _modelController;
  late TextEditingController _apiKeyController;
  late TextEditingController _maxTokensController;
  late TextEditingController _temperatureController;
  bool _advancedExpanded = false;
  bool _advancedHovered = false;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.provider.name);
    _baseUrlController = TextEditingController(text: widget.provider.baseUrl);
    _modelController = TextEditingController(text: widget.provider.model);
    _apiKeyController = TextEditingController(text: widget.provider.apiKey);
    _maxTokensController = TextEditingController(
      text: widget.provider.maxTokens?.toString() ?? '',
    );
    _temperatureController = TextEditingController(
      text: widget.provider.temperature?.toString() ?? '',
    );
  }

  @override
  void didUpdateWidget(_AiProviderEditor oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.provider.id != widget.provider.id ||
        oldWidget.provider.name != widget.provider.name) {
      _nameController.text = widget.provider.name;
    }
    if (oldWidget.provider.id != widget.provider.id ||
        oldWidget.provider.baseUrl != widget.provider.baseUrl) {
      _baseUrlController.text = widget.provider.baseUrl;
    }
    if (oldWidget.provider.id != widget.provider.id ||
        oldWidget.provider.model != widget.provider.model) {
      _modelController.text = widget.provider.model;
    }
    if (oldWidget.provider.id != widget.provider.id ||
        oldWidget.provider.apiKey != widget.provider.apiKey) {
      _apiKeyController.text = widget.provider.apiKey;
    }
    if (oldWidget.provider.id != widget.provider.id ||
        oldWidget.provider.maxTokens != widget.provider.maxTokens) {
      _maxTokensController.text = widget.provider.maxTokens?.toString() ?? '';
    }
    _syncGenerationControllers(oldWidget.provider);
  }

  @override
  void dispose() {
    _nameController.dispose();
    _baseUrlController.dispose();
    _modelController.dispose();
    _apiKeyController.dispose();
    _maxTokensController.dispose();
    _temperatureController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        _SettingsRow(
          localizationKey: 'settings.ai.provider.name',
          title: 'Name',
          subtitle: 'Display name for this provider.',
          trailing: _SettingsTextField(
            controller: _nameController,
            onChanged: (value) => widget.onFieldChanged(name: value),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          localizationKey: 'settings.ai.provider.protocol',
          title: 'Protocol',
          subtitle: 'API protocol used by this provider.',
          trailing: _SettingsSelect(
            key: const ValueKey('settings-ai-protocol-select'),
            label: 'Protocol',
            showLabel: false,
            value: widget.provider.protocol,
            values: AiApiProtocol.values
                .map((p) => p.storageValue)
                .toList(growable: false),
            format: (value) => AiApiProtocol.fromString(value).label,
            onChanged: (value) => widget.onFieldChanged(protocol: value),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          localizationKey: 'settings.ai.provider.baseUrl',
          title: 'Base URL',
          subtitle: 'API endpoint for this provider.',
          trailing: _SettingsTextField(
            controller: _baseUrlController,
            onChanged: (value) => widget.onFieldChanged(baseUrl: value),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          localizationKey: 'settings.ai.provider.models',
          title: 'Models',
          subtitle:
              'Comma-separated model identifiers (e.g. gpt-4o,gpt-4o-mini).',
          trailing: _SettingsTextField(
            controller: _modelController,
            onChanged: (value) => widget.onFieldChanged(model: value),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          localizationKey: 'settings.ai.provider.apiKey',
          title: 'API Key',
          subtitle: 'Credential for this provider.',
          trailing: _SettingsTextField(
            controller: _apiKeyController,
            obscureText: true,
            onChanged: (value) => widget.onFieldChanged(apiKey: value),
          ),
        ),
        SizedBox(height: 16),
        MouseRegion(
          cursor: SystemMouseCursors.click,
          onEnter: (_) => setState(() => _advancedHovered = true),
          onExit: (_) => setState(() => _advancedHovered = false),
          child: Semantics(
            button: true,
            expanded: _advancedExpanded,
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: () =>
                  setState(() => _advancedExpanded = !_advancedExpanded),
              child: SizedBox(
                height: 28,
                child: Row(
                  children: [
                    AnimatedRotation(
                      turns: _advancedExpanded ? 0.25 : 0,
                      duration: const Duration(milliseconds: 150),
                      curve: Curves.easeOutCubic,
                      child: Icon(
                        LucideIcons.chevronRight,
                        size: 14,
                        color: _advancedHovered ? _mutedText : _faintText,
                      ),
                    ),
                    SizedBox(width: 5),
                    Text(
                      tr('workspace.label.advanced', fallback: 'Advanced'),
                      style: TextStyle(
                        color: _advancedHovered ? _text : _mutedText,
                        fontSize: 12,
                        height: 1.35,
                        fontWeight: NautermFontWeights.medium,
                        letterSpacing: 0,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
        AnimatedSize(
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOutCubic,
          alignment: Alignment.topCenter,
          child: _advancedExpanded
              ? Padding(
                  padding: const EdgeInsets.only(top: 10, left: 19),
                  child: Column(
                    children: [
                      _generationRow(
                        localizationKey: 'settings.ai.provider.maxTokens',
                        title: 'Max Tokens',
                        subtitle: 'Maximum tokens generated in one response. Leave blank to use the provider default (Anthropic falls back to 4096).',
                        controller: _maxTokensController,
                        fieldKey: const ValueKey(
                          'settings-ai-max-tokens-field',
                        ),
                        integer: true,
                        onChanged: (value) {
                          final parsed = int.tryParse(value);
                          if (parsed != null && parsed > 0) {
                            widget.onFieldChanged(maxTokens: parsed);
                          } else if (value.trim().isEmpty) {
                            widget.onFieldChanged(clearMaxTokens: true);
                          }
                        },
                      ),
                      _generationRow(
                        localizationKey: 'settings.ai.provider.temperature',
                        title: 'Temperature',
                        subtitle: 'Sampling randomness from 0 to 1. Leave blank to use the provider default.',
                        controller: _temperatureController,
                        decimal: true,
                        onChanged: _updateTemperature,
                      ),
                    ],
                  ),
                )
              : const SizedBox.shrink(),
        ),
      ],
    );
  }

  void _syncGenerationControllers(AiProviderEntry oldProvider) {
    if (oldProvider.id != widget.provider.id ||
        oldProvider.temperature != widget.provider.temperature) {
      _temperatureController.text =
          widget.provider.temperature?.toString() ?? '';
    }
  }

  Widget _generationRow({
    required String localizationKey,
    required String title,
    required String subtitle,
    required TextEditingController controller,
    required ValueChanged<String> onChanged,
    Key? fieldKey,
    bool integer = false,
    bool decimal = false,
  }) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: _SettingsRow(
        localizationKey: localizationKey,
        title: title,
        subtitle: subtitle,
        trailing: _SettingsTextField(
          key: fieldKey,
          controller: controller,
          keyboardType: integer
              ? TextInputType.number
              : decimal
              ? const TextInputType.numberWithOptions(
                  decimal: true,
                  signed: true,
                )
              : TextInputType.text,
          inputFormatters: integer
              ? [FilteringTextInputFormatter.digitsOnly]
              : null,
          onChanged: onChanged,
        ),
      ),
    );
  }

  void _updateTemperature(String value) {
    final normalized = value.trim();
    if (normalized.isEmpty) {
      widget.onFieldChanged(clearTemperature: true);
      return;
    }
    final parsed = double.tryParse(normalized);
    if (parsed != null && parsed.isFinite && parsed >= 0 && parsed <= 1) {
      widget.onFieldChanged(temperature: parsed);
    }
  }
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
