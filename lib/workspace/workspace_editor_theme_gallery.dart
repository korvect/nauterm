part of 'nauterm_workspace.dart';

class _ThemeGalleryDrawer extends StatefulWidget {
  const _ThemeGalleryDrawer({
    required this.themesFuture,
    required this.selectedThemeId,
    this.dark = false,
    required this.onClose,
    required this.onSelected,
  });

  final Future<List<StoredTerminalTheme>> themesFuture;
  final String? selectedThemeId;
  final bool dark;
  final VoidCallback onClose;
  final void Function(String? themeId, StoredTerminalTheme? selectedTheme)
  onSelected;

  @override
  State<_ThemeGalleryDrawer> createState() => _ThemeGalleryDrawerState();
}

class _ThemeGalleryDrawerState extends State<_ThemeGalleryDrawer> {
  final TextEditingController _searchController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _searchController.addListener(_handleSearchChanged);
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

  @override
  Widget build(BuildContext context) {
    return Material(
      color: _surface,
      elevation: 18,
      shadowColor: const Color(0x26000000),
      child: SafeArea(
        top: false,
        bottom: false,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Container(
              height: _workspaceDrawerHeaderHeight,
              padding: const EdgeInsets.fromLTRB(16, 5, 8, 5),
              decoration: BoxDecoration(
                color: _card,
                border: Border(bottom: BorderSide(color: _sidebarDivider)),
              ),
              child: Row(
                children: [
                  _WorkspaceDrawerHeaderButton(
                    icon: Icons.arrow_back_rounded,
                    tooltip: tr('common.action.back', fallback: 'Back'),
                    onPressed: widget.onClose,
                  ),
                  SizedBox(width: 4),
                  Expanded(
                    child: Text(
                      tr('common.label.themes', fallback: 'Themes'),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: _text,
                        fontSize: NautermFontSizes.titleSmall,
                        fontWeight: NautermFontWeights.semibold,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(14, 12, 14, 10),
              child: _ThemeGallerySearchField(controller: _searchController),
            ),
            Expanded(
              child: FutureBuilder<List<StoredTerminalTheme>>(
                future: widget.themesFuture,
                builder: (context, snapshot) {
                  if (snapshot.connectionState != ConnectionState.done) {
                    return Center(
                      child: SizedBox.square(
                        dimension: 22,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      ),
                    );
                  }

                  if (snapshot.hasError) {
                    return Center(
                      child: Padding(
                        padding: const EdgeInsets.all(18),
                        child: Text(
                          tr(
                            'workspace.themeGallery.loadError.description',
                            fallback: 'Failed to load themes.',
                          ),
                          textAlign: TextAlign.center,
                          style: TextStyle(
                            color: _mutedText,
                            fontSize: NautermFontSizes.labelMedium,
                            fontWeight: NautermFontWeights.medium,
                            letterSpacing: 0,
                          ),
                        ),
                      ),
                    );
                  }

                  final themes = snapshot.data ?? const <StoredTerminalTheme>[];
                  final allEntries = <_ThemeGalleryEntry>[
                    _ThemeGalleryEntry(
                      id: null,
                      title: tr('common.label.default', fallback: 'Default'),
                      theme: widget.dark
                          ? terminalDarkCustomTheme
                          : terminalThemeId == null
                          ? defaultTerminalTheme
                          : terminalCustomTheme,
                      storedTheme: null,
                    ),
                    for (final theme in themes)
                      _ThemeGalleryEntry(
                        id: theme.id,
                        title: theme.theme.name,
                        theme: theme.theme,
                        storedTheme: theme,
                      ),
                  ];
                  final entries = _filterThemeGalleryEntries(
                    allEntries,
                    _searchController.text,
                  );

                  if (entries.isEmpty) {
                    return Center(
                      child: Text(
                        tr(
                          'workspace.description.noMatchingThemes',
                          fallback: 'No matching themes.',
                        ),
                        style: TextStyle(
                          color: _mutedText,
                          fontSize: NautermFontSizes.labelMedium,
                          fontWeight: NautermFontWeights.medium,
                          letterSpacing: 0,
                        ),
                      ),
                    );
                  }

                  return LayoutBuilder(
                    builder: (context, constraints) {
                      final columns = constraints.maxWidth < 300 ? 1 : 2;
                      return GridView.builder(
                        padding: const EdgeInsets.fromLTRB(10, 10, 10, 18),
                        itemCount: entries.length,
                        gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                          crossAxisCount: columns,
                          mainAxisSpacing: 10,
                          crossAxisSpacing: 10,
                          mainAxisExtent: 132,
                        ),
                        itemBuilder: (context, index) {
                          final entry = entries[index];
                          final selected = entry.id == widget.selectedThemeId;
                          return Material(
                            color: Colors.transparent,
                            child: InkWell(
                              borderRadius: BorderRadius.circular(8),
                              onTap: () => widget.onSelected(
                                entry.id,
                                entry.storedTheme,
                              ),
                              child: TerminalThemePreviewCard(
                                title: entry.title,
                                theme: entry.theme,
                                selected: selected,
                              ),
                            ),
                          );
                        },
                      );
                    },
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ThemeGalleryEntry {
  const _ThemeGalleryEntry({
    required this.id,
    required this.title,
    required this.theme,
    required this.storedTheme,
  });

  final String? id;
  final String title;
  final TerminalTheme theme;
  final StoredTerminalTheme? storedTheme;
}

class _ThemeGallerySearchField extends StatelessWidget {
  const _ThemeGallerySearchField({required this.controller});

  final TextEditingController controller;

  @override
  Widget build(BuildContext context) {
    const height = 38.0;
    return SizedBox(
      height: height,
      child: Material(
        color: _card,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8),
          side: BorderSide(color: _sidebarDivider),
        ),
        clipBehavior: Clip.antiAlias,
        child: Row(
          children: [
            SizedBox(width: 12),
            Expanded(
              child: TextField(
                controller: controller,
                style: TextStyle(
                  color: _text,
                  fontSize: NautermFontSizes.labelLarge,
                  fontWeight: NautermFontWeights.medium,
                  letterSpacing: 0,
                ),
                decoration: InputDecoration.collapsed(
                  hintText: tr(
                    'common.label.searchThemes',
                    fallback: 'Search themes',
                  ),
                  hintStyle: TextStyle(
                    color: Color(0xff81979e),
                    fontSize: NautermFontSizes.labelLarge,
                    fontWeight: NautermFontWeights.medium,
                    letterSpacing: 0,
                  ),
                ),
              ),
            ),
            SizedBox(width: 6),
            if (controller.text.isEmpty)
              Padding(
                padding: EdgeInsets.only(right: 10),
                child: Icon(Icons.search_rounded, size: 18, color: _mutedText),
              )
            else
              Padding(
                padding: const EdgeInsets.only(right: 6),
                child: _WorkspaceButton(
                  icon: Icons.close_rounded,
                  size: _WorkspaceControlSize.tiny,
                  variant: _WorkspaceButtonVariant.text,
                  height: 26,
                  minWidth: 26,
                  onPressed: controller.clear,
                ),
              ),
          ],
        ),
      ),
    );
  }
}

List<_ThemeGalleryEntry> _filterThemeGalleryEntries(
  List<_ThemeGalleryEntry> entries,
  String query,
) {
  final normalizedQuery = query.trim().toLowerCase();
  if (normalizedQuery.isEmpty) {
    return entries;
  }

  final tokens = normalizedQuery
      .split(RegExp(r'\s+'))
      .where((token) => token.isNotEmpty)
      .toList(growable: false);

  return entries
      .where((entry) {
        final haystack = [
          entry.title,
          entry.id ?? '',
          entry.theme.name,
          entry.theme.type.storageValue,
        ].join(' ').toLowerCase();
        return tokens.every(haystack.contains);
      })
      .toList(growable: false);
}

List<String> _availableShellOptions(String? currentShellPath) {
  final shells = <String>{};
  for (final shell in _systemShells()) {
    shells.add(shell);
  }
  final currentShell = _emptyToNull(currentShellPath);
  if (currentShell != null) {
    shells.add(currentShell);
  }
  final sorted = shells.toList(growable: false);
  return sorted..sort();
}

List<String> _systemShells() {
  return discoverSystemShells();
}
