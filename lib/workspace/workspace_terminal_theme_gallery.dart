part of 'nauterm_workspace.dart';

bool _terminalThemesMatch(TerminalTheme left, TerminalTheme right) {
  return left.toJson().toString() == right.toJson().toString();
}

class _TerminalThemeGallery extends StatefulWidget {
  const _TerminalThemeGallery({
    required this.colors,
    required this.loadThemes,
    required this.currentTheme,
    required this.currentFont,
    required this.onSelected,
    required this.onFontChanged,
  });

  final _AiAssistantColors colors;
  final Future<List<StoredTerminalTheme>> Function() loadThemes;
  final TerminalTheme Function() currentTheme;
  final TerminalFontConfig Function() currentFont;
  final ValueChanged<StoredTerminalTheme> onSelected;
  final ValueChanged<TerminalFontConfig> onFontChanged;

  @override
  State<_TerminalThemeGallery> createState() => _TerminalThemeGalleryState();
}

class _TerminalThemeGalleryState extends State<_TerminalThemeGallery> {
  final TextEditingController _searchController = TextEditingController();
  late Future<List<StoredTerminalTheme>> _themesFuture;
  late Future<List<String>> _fontFamiliesFuture;

  @override
  void initState() {
    super.initState();
    _themesFuture = widget.loadThemes();
    _fontFamiliesFuture = _loadFontFamilies();
    _searchController.addListener(_handleSearchChanged);
  }

  Future<List<String>> _loadFontFamilies() async {
    return loadMonospaceFontFamilies();
  }

  @override
  void dispose() {
    _searchController.removeListener(_handleSearchChanged);
    _searchController.dispose();
    super.dispose();
  }

  void _handleSearchChanged() {
    setState(() {});
  }

  void _reload() {
    setState(() => _themesFuture = widget.loadThemes());
  }

  @override
  Widget build(BuildContext context) {
    final colors = widget.colors;
    final currentFont = widget.currentFont();
    final fontSizeOptions = [
      ...terminalFontSizeOptions,
      if (!terminalFontSizeOptions.contains(currentFont.size)) currentFont.size,
    ]..sort();
    return Expanded(
      key: const ValueKey('terminal-theme-gallery'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 12, 12, 10),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  tr(
                    'workspace.label.terminalAppearance',
                    fallback: 'Terminal appearance',
                  ),
                  style: TextStyle(
                    color: colors.foreground,
                    fontSize: 13,
                    fontWeight: NautermFontWeights.semibold,
                    letterSpacing: 0,
                  ),
                ),
                const SizedBox(height: 10),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Expanded(
                      child: FutureBuilder<List<String>>(
                        future: _fontFamiliesFuture,
                        builder: (context, snapshot) {
                          final loadedFamilies =
                              snapshot.data ?? fallbackMonospaceFontFamilies;
                          final fontOptions = [...loadedFamilies];
                          return _TerminalAppearanceField(
                            label: 'Font',
                            colors: colors,
                            child: _TerminalToolDropdown<String>(
                              key: const ValueKey('terminal-font-family'),
                              value: currentFont.family,
                              options: [
                                for (final family in fontOptions)
                                  NautermContextMenuAction(
                                    value: family,
                                    label: family,
                                  ),
                              ],
                              colors: colors,
                              outlined: true,
                              searchable: true,
                              customValueBuilder: (query) => query,
                              fontSize: 11.5,
                              fontWeight: NautermFontWeights.medium,
                              onChanged: (family) => widget.onFontChanged(
                                currentFont.copyWith(family: family),
                              ),
                            ),
                          );
                        },
                      ),
                    ),
                    const SizedBox(width: 8),
                    SizedBox(
                      width: 82,
                      child: _TerminalAppearanceField(
                        label: 'Size',
                        colors: colors,
                        child: _TerminalToolDropdown<double>(
                          key: const ValueKey('terminal-font-size'),
                          value: currentFont.size,
                          options: [
                            for (final size in fontSizeOptions)
                              NautermContextMenuAction(
                                value: size,
                                label: _formatTerminalFontSize(size),
                              ),
                          ],
                          colors: colors,
                          outlined: true,
                          fontSize: 11.5,
                          fontWeight: NautermFontWeights.medium,
                          onChanged: (size) => widget.onFontChanged(
                            currentFont.copyWith(size: size),
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                SizedBox(
                  height: 34,
                  child: TextField(
                    key: const ValueKey('terminal-theme-search'),
                    controller: _searchController,
                    cursorColor: colors.accent,
                    style: TextStyle(
                      color: colors.foreground,
                      fontSize: 12,
                      fontWeight: NautermFontWeights.medium,
                      letterSpacing: 0,
                    ),
                    decoration: InputDecoration(
                      isDense: true,
                      hintText: tr(
                        'common.label.searchThemes',
                        fallback: 'Search themes',
                      ),
                      hintStyle: TextStyle(
                        color: colors.muted,
                        fontSize: 12,
                        letterSpacing: 0,
                      ),
                      prefixIcon: Icon(
                        LucideIcons.search,
                        size: 16,
                        color: colors.muted,
                      ),
                      prefixIconConstraints: const BoxConstraints.tightFor(
                        width: 32,
                        height: 32,
                      ),
                      suffixIcon: _searchController.text.isEmpty
                          ? null
                          : IconButton(
                              tooltip: tr(
                                'workspace.label.clearSearch',
                                fallback: 'Clear search',
                              ),
                              onPressed: _searchController.clear,
                              icon: const Icon(LucideIcons.x, size: 14),
                              color: colors.muted,
                              padding: EdgeInsets.zero,
                              constraints: const BoxConstraints.tightFor(
                                width: 30,
                                height: 30,
                              ),
                            ),
                      filled: true,
                      fillColor: colors.inputBackground,
                      contentPadding: const EdgeInsets.only(right: 8),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide: BorderSide(color: colors.border),
                      ),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide: BorderSide(color: colors.accent),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
          Divider(height: 1, color: colors.border),
          Expanded(
            child: ClipRect(
              clipBehavior: Clip.hardEdge,
              child: ColoredBox(
                color: colors.background,
                child: FutureBuilder<List<StoredTerminalTheme>>(
                  future: _themesFuture,
                  builder: (context, snapshot) {
                    if (snapshot.connectionState != ConnectionState.done) {
                      return Center(
                        child: SizedBox.square(
                          dimension: 20,
                          child: CircularProgressIndicator(
                            strokeWidth: 1.8,
                            color: colors.accent,
                          ),
                        ),
                      );
                    }
                    if (snapshot.hasError) {
                      return _TerminalThemeGalleryError(
                        colors: colors,
                        onRetry: _reload,
                      );
                    }

                    final query = _searchController.text.trim().toLowerCase();
                    final themes = [
                      for (final entry
                          in snapshot.data ?? const <StoredTerminalTheme>[])
                        if (query.isEmpty ||
                            entry.id.toLowerCase().contains(query) ||
                            entry.theme.name.toLowerCase().contains(query))
                          entry,
                    ];
                    if (themes.isEmpty) {
                      return Center(
                        child: Text(
                          tr(
                            'workspace.description.noMatchingThemes',
                            fallback: 'No matching themes.',
                          ),
                          style: TextStyle(
                            color: colors.muted,
                            fontSize: 12,
                            fontWeight: NautermFontWeights.medium,
                            letterSpacing: 0,
                          ),
                        ),
                      );
                    }

                    final currentTheme = widget.currentTheme();
                    return LayoutBuilder(
                      builder: (context, constraints) {
                        final columns = constraints.maxWidth >= 520 ? 2 : 1;
                        return GridView.builder(
                          clipBehavior: Clip.hardEdge,
                          padding: const EdgeInsets.fromLTRB(12, 10, 12, 16),
                          itemCount: themes.length,
                          gridDelegate:
                              SliverGridDelegateWithFixedCrossAxisCount(
                                crossAxisCount: columns,
                                mainAxisSpacing: 10,
                                crossAxisSpacing: 10,
                                mainAxisExtent: 108,
                              ),
                          itemBuilder: (context, index) {
                            final entry = themes[index];
                            return KeyedSubtree(
                              key: ValueKey(
                                'terminal-theme-option:${entry.id}',
                              ),
                              child: TerminalThemePreviewCard(
                                title: entry.theme.name,
                                theme: entry.theme,
                                compact: true,
                                selected: _terminalThemesMatch(
                                  currentTheme,
                                  entry.theme,
                                ),
                                onTap: () => widget.onSelected(entry),
                              ),
                            );
                          },
                        );
                      },
                    );
                  },
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _TerminalAppearanceField extends StatelessWidget {
  const _TerminalAppearanceField({
    required this.label,
    required this.colors,
    required this.child,
  });

  final String label;
  final _AiAssistantColors colors;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: TextStyle(
            color: colors.muted,
            fontSize: 10.5,
            fontWeight: NautermFontWeights.medium,
            letterSpacing: 0,
          ),
        ),
        const SizedBox(height: 5),
        child,
      ],
    );
  }
}

String _formatTerminalFontSize(double size) {
  return size == size.roundToDouble()
      ? size.toStringAsFixed(0)
      : size.toStringAsFixed(1);
}

class _TerminalThemeGalleryError extends StatelessWidget {
  const _TerminalThemeGalleryError({
    required this.colors,
    required this.onRetry,
  });

  final _AiAssistantColors colors;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            tr(
              'workspace.themeGallery.loadError.description',
              fallback: 'Failed to load themes.',
            ),
            style: TextStyle(
              color: colors.muted,
              fontSize: 12,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
          const SizedBox(height: 8),
          TextButton(
            onPressed: onRetry,
            style: TextButton.styleFrom(
              foregroundColor: colors.accent,
              overlayColor: colors.accent.withValues(alpha: 0.1),
            ),
            child: Text(tr('common.action.retry', fallback: 'Retry')),
          ),
        ],
      ),
    );
  }
}
