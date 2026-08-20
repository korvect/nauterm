part of 'settings_panel.dart';

enum _UpdateUiState {
  idle,
  checking,
  available,
  current,
  downloading,
  ready,
  installing,
  restartPending,
  error,
}

class _AboutSettingsPage extends StatefulWidget {
  const _AboutSettingsPage();

  @override
  State<_AboutSettingsPage> createState() => _AboutSettingsPageState();
}

class _AboutSettingsPageState extends State<_AboutSettingsPage> {
  DesktopUpdateService? _service;
  final MacosSparkleUpdater _sparkle = const MacosSparkleUpdater();
  _UpdateUiState _state = _UpdateUiState.idle;
  String _currentVersion = '...';
  String _buildNumber = '...';
  String _packageName = '...';
  DesktopUpdateRelease? _release;
  File? _downloadedFile;
  double? _progress;
  String? _message;
  DesktopUpdateInstallDisposition? _installDisposition;

  bool get _busy =>
      _state == _UpdateUiState.checking ||
      _state == _UpdateUiState.downloading ||
      _state == _UpdateUiState.installing;

  @override
  void initState() {
    super.initState();
    unawaited(_loadVersion());
  }

  Future<void> _loadVersion() async {
    try {
      final info = await PackageInfo.fromPlatform();
      if (!mounted) return;
      setState(() {
        _currentVersion = info.version;
        _buildNumber = info.buildNumber;
        _packageName = info.packageName;
      });
    } on Object {
      if (!mounted) return;
      setState(() {
        _currentVersion = 'unknown';
        _state = _UpdateUiState.error;
        _message = 'Unable to read the installed application version.';
      });
    }
  }

  Future<void> _check() async {
    if (_busy || _currentVersion == '...' || _currentVersion == 'unknown') {
      return;
    }
    setState(() {
      _state = _UpdateUiState.checking;
      _message = null;
      _progress = null;
      _release = null;
      _downloadedFile = null;
      _installDisposition = null;
    });
    try {
      if (Platform.isMacOS) {
        await _sparkle.checkForUpdates();
        if (!mounted) return;
        setState(() {
          _state = _UpdateUiState.idle;
          _message = 'Sparkle is checking the signed macOS update feed.';
        });
        return;
      }
      final service = _service ??= DesktopUpdateService();
      final result = await service.check(_currentVersion);
      if (!mounted) return;
      setState(() {
        _release = result.release;
        _state = result.updateAvailable
            ? _UpdateUiState.available
            : _UpdateUiState.current;
        _message = result.updateAvailable
            ? 'Nauterm ${result.release!.version} is ready to download.'
            : 'You are using the latest version.';
      });
    } on Object catch (error) {
      _showError(error);
    }
  }

  Future<void> _download() async {
    final release = _release;
    if (release == null || _busy) return;
    setState(() {
      _state = _UpdateUiState.downloading;
      _progress = 0;
      _message = 'Downloading and verifying ${release.asset.name}...';
    });
    try {
      final file = await _service!.download(
        release,
        onProgress: (received, total) {
          if (!mounted) return;
          setState(() {
            _progress = total > 0 ? (received / total).clamp(0, 1) : null;
          });
        },
      );
      if (!mounted) return;
      setState(() {
        _downloadedFile = file;
        _state = _UpdateUiState.ready;
        _progress = 1;
        _message = 'Download complete. SHA-256 verification passed.';
      });
    } on Object catch (error) {
      _showError(error);
    }
  }

  Future<void> _install() async {
    final release = _release;
    final file = _downloadedFile;
    if (release == null || file == null) return;
    setState(() {
      _state = _UpdateUiState.installing;
      _message = tr(
        'settings.update.status.installing',
        fallback: 'Installing the update...',
      );
    });
    try {
      final disposition = await _service!.launchInstaller(
        file,
        release.asset.kind,
      );
      if (!mounted) return;
      if (disposition == DesktopUpdateInstallDisposition.installerLaunched) {
        setState(() {
          _state = _UpdateUiState.ready;
          _message = tr(
            'settings.update.status.installerLaunched',
            fallback: 'The installer is open. Follow its instructions to finish updating.',
          );
        });
        return;
      }
      final packageInstalled =
          disposition == DesktopUpdateInstallDisposition.restartRequired;
      setState(() {
        _installDisposition = disposition;
        _state = _UpdateUiState.restartPending;
        _message = tr(
          packageInstalled
              ? 'settings.update.status.installed'
              : 'settings.update.status.readyToRestart',
          fallback: packageInstalled
              ? 'Update installed successfully. Restart Nauterm to use the new version.'
              : 'Update ready. Restart Nauterm to finish applying it.',
        );
      });
      final restart = await showNautermDialog<bool>(
        context: context,
        barrierDismissible: false,
        builder: (_) => _UpdateInstalledDialog(
          version: release.version,
          packageInstalled: packageInstalled,
        ),
      );
      if (restart == true && mounted) {
        await _restartAfterUpdate();
      }
    } on Object catch (error) {
      _showError(error);
    }
  }

  Future<void> _restartAfterUpdate() async {
    final disposition = _installDisposition;
    if (disposition == null) return;
    await restartNautermApplication(
      launchInstalledExecutable:
          disposition == DesktopUpdateInstallDisposition.restartRequired,
    );
  }

  void _showError(Object error) {
    if (!mounted) return;
    setState(() {
      _state = _UpdateUiState.error;
      _progress = null;
      _message = error is DesktopUpdateException
          ? error.message
          : 'Update failed: $error';
    });
  }

  Future<void> _showThirdPartyLicenses() async {
    await showNautermDialog<void>(
      context: context,
      builder: (_) => _ThirdPartyLicensesDialog(
        applicationVersion: _buildNumber.isEmpty || _buildNumber == '...'
            ? _currentVersion
            : '$_currentVersion ($_buildNumber)',
      ),
    );
  }

  @override
  void dispose() {
    _service?.close();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final release = _release;
    final settingsState = context
        .findAncestorStateOfType<_SettingsPanelState>()!;
    return Scrollbar(
      controller: settingsState._contentScrollController,
      thumbVisibility: true,
      child: ListView(
        controller: settingsState._contentScrollController,
        padding: settingsState._contentPadding,
        children: [
          Text(
            tr('settings.pages.about.title', fallback: 'About Nauterm'),
            style: TextStyle(
              color: _text,
              fontSize: 20,
              height: 1.4,
              fontWeight: NautermFontWeights.semibold,
            ),
          ),
          const SizedBox(height: 3),
          Text(
            tr(
              'settings.pages.about.description',
              fallback:
                  'Application information, release details, and updates.',
            ),
            style: TextStyle(color: _mutedText, fontSize: 12),
          ),
          const SizedBox(height: 30),
          _SettingsSection(
            icon: LucideIcons.info,
            title: 'Application',
            child: ConstrainedBox(
              constraints: const BoxConstraints(
                maxWidth: _settingsContentMaxWidth,
              ),
              child: Column(
                children: [
                  _SettingsRow(
                    title: 'Version',
                    subtitle: 'Release channel: stable',
                    trailing: Align(
                      alignment: Alignment.centerRight,
                      child: Text(
                        _currentVersion,
                        key: const ValueKey('settings-update-current-version'),
                        style: TextStyle(
                          color: _text,
                          fontSize: 14,
                          fontWeight: NautermFontWeights.medium,
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 18),
                  _SettingsRow(
                    title: 'Build',
                    subtitle: _packageName,
                    trailing: Align(
                      alignment: Alignment.centerRight,
                      child: Text(
                        _buildNumber,
                        key: const ValueKey('settings-about-build-number'),
                        style: TextStyle(color: _text, fontSize: 13),
                      ),
                    ),
                  ),
                  const SizedBox(height: 18),
                  _SettingsRow(
                    title: 'Platform',
                    subtitle: 'Current operating system',
                    trailing: Align(
                      alignment: Alignment.centerRight,
                      child: Text(
                        _platformName,
                        key: const ValueKey('settings-about-platform'),
                        style: TextStyle(color: _text, fontSize: 13),
                      ),
                    ),
                  ),
                  if (release != null) ...[
                    const SizedBox(height: 18),
                    _SettingsRow(
                      title: 'Available version',
                      subtitle: release.asset.name,
                      trailing: Align(
                        alignment: Alignment.centerRight,
                        child: Text(
                          release.version,
                          style: TextStyle(color: _primary, fontSize: 14),
                        ),
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ),
          const SizedBox(height: 30),
          _SettingsSection(
            icon: LucideIcons.scale,
            title: 'License',
            child: ConstrainedBox(
              constraints: const BoxConstraints(
                maxWidth: _settingsContentMaxWidth,
              ),
              child: Column(
                children: [
                  _SettingsRow(
                    title: 'Source license',
                    subtitle: 'See LICENSE for the complete terms.',
                    trailing: Align(
                      alignment: Alignment.centerRight,
                      child: Text(
                        tr(
                          'settings.label.mitCommonsClause',
                          fallback: 'MIT + Commons Clause',
                        ),
                        style: TextStyle(color: _mutedText, fontSize: 12),
                      ),
                    ),
                  ),
                  const SizedBox(height: 18),
                  _SettingsRow(
                    title: 'Third-party licenses',
                    subtitle: 'Licenses for bundled Flutter, Dart, Rust, and native components.',
                    trailing: Align(
                      alignment: Alignment.centerRight,
                      child: _SettingsOutlineButton(
                        key: const ValueKey(
                          'settings-about-third-party-licenses',
                        ),
                        label: 'View',
                        onTap: _showThirdPartyLicenses,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 30),
          _SettingsSection(
            icon: LucideIcons.download,
            title: 'Update',
            child: ConstrainedBox(
              constraints: const BoxConstraints(
                maxWidth: _settingsContentMaxWidth,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (_progress != null) ...[
                    LinearProgressIndicator(
                      value: _progress,
                      minHeight: 4,
                      color: _primary,
                      backgroundColor: _softOutline,
                      borderRadius: BorderRadius.circular(3),
                    ),
                    const SizedBox(height: 14),
                  ],
                  if (_message != null) ...[
                    Text(
                      _message!,
                      key: const ValueKey('settings-update-status'),
                      style: TextStyle(
                        color: _state == _UpdateUiState.error
                            ? const Color(0xffdc2626)
                            : _mutedText,
                        fontSize: 12,
                        height: 1.45,
                      ),
                    ),
                    const SizedBox(height: 14),
                  ],
                  Align(
                    alignment: Alignment.centerRight,
                    child: _SettingsOutlineButton(
                      key: const ValueKey('settings-update-action'),
                      onTap: _busy
                          ? null
                          : switch (_state) {
                              _UpdateUiState.available => _download,
                              _UpdateUiState.ready => _install,
                              _UpdateUiState.restartPending =>
                                _restartAfterUpdate,
                              _ => _check,
                            },
                      leading: _busy
                          ? const SizedBox.square(
                              dimension: 14,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : Icon(
                              _state == _UpdateUiState.ready
                                  ? LucideIcons.packageOpen
                                  : _state == _UpdateUiState.restartPending
                                  ? LucideIcons.refreshCw
                                  : _state == _UpdateUiState.available
                                  ? LucideIcons.download
                                  : LucideIcons.refreshCw,
                              size: 15,
                            ),
                      label: switch (_state) {
                        _UpdateUiState.checking => tr(
                          'settings.update.action.checking',
                          fallback: 'Checking...',
                        ),
                        _UpdateUiState.downloading => tr(
                          'settings.update.action.downloading',
                          fallback: 'Downloading...',
                        ),
                        _UpdateUiState.installing => tr(
                          'settings.update.action.installing',
                          fallback: 'Installing...',
                        ),
                        _UpdateUiState.available => tr(
                          'settings.update.action.download',
                          fallback: 'Download update',
                        ),
                        _UpdateUiState.ready => tr(
                          'settings.update.action.install',
                          fallback: 'Install update',
                        ),
                        _UpdateUiState.restartPending => tr(
                          'settings.update.action.restart',
                          fallback: 'Restart Nauterm',
                        ),
                        _ => tr(
                          'settings.update.action.check',
                          fallback: 'Check for updates',
                        ),
                      },
                    ),
                  ),
                ],
              ),
            ),
          ),
          if (release != null && release.notes.trim().isNotEmpty) ...[
            const SizedBox(height: 30),
            _SettingsSection(
              icon: LucideIcons.scrollText,
              title: 'Release Notes',
              child: ConstrainedBox(
                constraints: const BoxConstraints(
                  maxWidth: _settingsContentMaxWidth,
                ),
                child: SelectableText(
                  release.notes.trim(),
                  maxLines: 12,
                  style: TextStyle(
                    color: _mutedText,
                    fontSize: 12,
                    height: 1.5,
                  ),
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _UpdateInstalledDialog extends StatelessWidget {
  const _UpdateInstalledDialog({
    required this.version,
    required this.packageInstalled,
  });

  final String version;
  final bool packageInstalled;

  @override
  Widget build(BuildContext context) {
    return _SyncDialogFrame(
      icon: LucideIcons.circleCheck,
      title: packageInstalled
          ? 'settings.update.dialog.installed.title'
          : 'settings.update.dialog.ready.title',
      subtitle: packageInstalled
          ? 'settings.update.dialog.installed.subtitle'
          : 'settings.update.dialog.ready.subtitle',
      width: 430,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            tr(
              packageInstalled
                  ? 'settings.update.dialog.installed.description'
                  : 'settings.update.dialog.ready.description',
              fallback: packageInstalled
                  ? 'Nauterm {version} was installed successfully. Restart now to use the new version.'
                  : 'Nauterm {version} is ready. Restart now to replace the current AppImage and launch the update.',
              args: {'version': version},
            ),
            style: TextStyle(color: _mutedText, fontSize: 12, height: 1.5),
          ),
          const SizedBox(height: 18),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: 8,
            runSpacing: 8,
            children: [
              _SettingsOutlineButton(
                label: tr('settings.update.action.later', fallback: 'Later'),
                onTap: () => Navigator.of(context).pop(false),
              ),
              _SettingsOutlineButton(
                key: const ValueKey('settings-update-restart-now'),
                label: tr(
                  'settings.update.action.restart',
                  fallback: 'Restart Nauterm',
                ),
                onTap: () => Navigator.of(context).pop(true),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

String get _platformName {
  if (Platform.isMacOS) return 'macOS';
  if (Platform.isWindows) return 'Windows';
  if (Platform.isLinux) return 'Linux';
  return Platform.operatingSystem;
}

class _ThirdPartyLicenseRecord {
  _ThirdPartyLicenseRecord(this.body);

  final String body;
  final Set<String> packages = {};
}

class _ThirdPartyPackageRecord {
  _ThirdPartyPackageRecord(this.packageName);

  final String packageName;
  final List<_ThirdPartyLicenseRecord> licenses = [];
}

class _ThirdPartyLicensesDialog extends StatefulWidget {
  const _ThirdPartyLicensesDialog({required this.applicationVersion});

  final String applicationVersion;

  @override
  State<_ThirdPartyLicensesDialog> createState() =>
      _ThirdPartyLicensesDialogState();
}

class _ThirdPartyLicensesDialogState extends State<_ThirdPartyLicensesDialog> {
  final TextEditingController _searchController = TextEditingController();
  List<_ThirdPartyPackageRecord> _records = const [];
  String? _selectedPackage;
  String _query = '';
  bool _loading = true;

  List<_ThirdPartyPackageRecord> get _filteredRecords {
    final query = _query.trim().toLowerCase();
    if (query.isEmpty) return _records;
    return _records
        .where((record) => record.packageName.toLowerCase().contains(query))
        .toList(growable: false);
  }

  _ThirdPartyPackageRecord? get _selectedRecord {
    final records = _filteredRecords;
    if (records.isEmpty) return null;
    for (final record in records) {
      if (record.packageName == _selectedPackage) return record;
    }
    return records.first;
  }

  @override
  void initState() {
    super.initState();
    unawaited(_loadLicenses());
  }

  Future<void> _loadLicenses() async {
    final grouped = <String, _ThirdPartyLicenseRecord>{};
    await for (final entry in LicenseRegistry.licenses) {
      final body = entry.paragraphs
          .map((paragraph) => paragraph.text.trim())
          .where((text) => text.isNotEmpty)
          .join('\n\n');
      if (body.isEmpty) continue;
      final record = grouped.putIfAbsent(
        body,
        () => _ThirdPartyLicenseRecord(body),
      );
      record.packages.addAll(
        entry.packages
            .map((package) => package.trim())
            .where((package) => package.isNotEmpty),
      );
    }
    final packages = <String, _ThirdPartyPackageRecord>{};
    for (final license in grouped.values) {
      for (final packageName in license.packages) {
        packages
            .putIfAbsent(
              packageName,
              () => _ThirdPartyPackageRecord(packageName),
            )
            .licenses
            .add(license);
      }
    }
    final records = packages.values.toList()
      ..sort(
        (left, right) => left.packageName.toLowerCase().compareTo(
          right.packageName.toLowerCase(),
        ),
      );
    if (!mounted) return;
    setState(() {
      _records = records;
      _selectedPackage = records.firstOrNull?.packageName;
      _loading = false;
    });
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final viewport = MediaQuery.sizeOf(context);
    final width = math.min(viewport.width - 48, 840.0);
    final height = math.min(viewport.height - 48, 680.0);
    final filteredRecords = _filteredRecords;
    final selectedRecord = _selectedRecord;
    return Dialog(
      key: const ValueKey('third-party-license-browser'),
      backgroundColor: Colors.transparent,
      surfaceTintColor: Colors.transparent,
      insetPadding: const EdgeInsets.all(24),
      child: SizedBox(
        width: width,
        height: height,
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
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                _buildHeader(context),
                Padding(
                  padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
                  child: _buildSearchField(),
                ),
                Divider(height: 1, color: _softOutline),
                Expanded(
                  child: _loading
                      ? const Center(
                          child: SizedBox.square(
                            dimension: 20,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                        )
                      : Row(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            SizedBox(
                              width: math.min(235, width * 0.34),
                              child: _buildPackageList(filteredRecords),
                            ),
                            VerticalDivider(width: 1, color: _softOutline),
                            Expanded(child: _buildLicenseBody(selectedRecord)),
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

  Widget _buildHeader(BuildContext context) {
    return Container(
      padding: const EdgeInsets.fromLTRB(15, 10, 8, 9),
      decoration: BoxDecoration(
        color: _surfaceContainer,
        border: Border(bottom: BorderSide(color: _softOutline)),
      ),
      child: Row(
        children: [
          Container(
            width: 28,
            height: 28,
            decoration: BoxDecoration(
              color: _primary.withValues(alpha: 0.09),
              borderRadius: BorderRadius.circular(7),
            ),
            alignment: Alignment.center,
            child: Icon(LucideIcons.scale, size: 14, color: _primary),
          ),
          const SizedBox(width: 9),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  tr(
                    'settings.label.thirdPartyLicenses',
                    fallback: 'Third-party licenses',
                  ),
                  style: TextStyle(
                    color: _text,
                    fontSize: 14,
                    fontWeight: NautermFontWeights.semibold,
                  ),
                ),
                const SizedBox(height: 1),
                Text(
                  tr('Nauterm ${widget.applicationVersion}'),
                  style: TextStyle(color: _mutedText, fontSize: 10),
                ),
              ],
            ),
          ),
          IconButton(
            tooltip: tr('common.action.close', fallback: 'Close'),
            onPressed: () => Navigator.of(context).pop(),
            icon: const Icon(LucideIcons.x, size: 15),
            color: _mutedText,
            visualDensity: VisualDensity.compact,
            style: IconButton.styleFrom(
              hoverColor: _softOutline,
              highlightColor: _softOutline,
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildSearchField() {
    return SizedBox(
      height: 32,
      child: TextField(
        key: const ValueKey('third-party-license-search'),
        controller: _searchController,
        autofocus: true,
        onChanged: (value) => setState(() {
          _query = value;
          _selectedPackage = null;
        }),
        style: TextStyle(color: _text, fontSize: 12),
        cursorColor: _primary,
        decoration: InputDecoration(
          hintText: tr(
            'settings.search.packages.hint',
            fallback: 'Search packages...',
          ),
          hintStyle: TextStyle(color: _faintText, fontSize: 12),
          prefixIcon: Icon(LucideIcons.search, size: 15, color: _faintText),
          prefixIconConstraints: const BoxConstraints(
            minWidth: 32,
            minHeight: 32,
          ),
          isDense: true,
          filled: true,
          fillColor: _surfaceContainer,
          contentPadding: const EdgeInsets.fromLTRB(0, 8, 10, 8),
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(7),
            borderSide: BorderSide(color: _softOutline),
          ),
          focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(7),
            borderSide: BorderSide(color: _primary),
          ),
        ),
      ),
    );
  }

  Widget _buildPackageList(List<_ThirdPartyPackageRecord> records) {
    if (records.isEmpty) {
      return Center(
        child: Text(
          tr(
            'settings.about.licenses.noMatches',
            fallback: 'No matching packages.',
          ),
          style: TextStyle(color: _mutedText, fontSize: 12),
        ),
      );
    }
    final selectedPackage = _selectedRecord?.packageName;
    return ListView.builder(
      key: const ValueKey('third-party-license-package-list'),
      padding: const EdgeInsets.all(8),
      itemCount: records.length,
      itemBuilder: (context, index) {
        final record = records[index];
        final selected = record.packageName == selectedPackage;
        return Padding(
          padding: const EdgeInsets.only(bottom: 2),
          child: Material(
            color: selected
                ? _primary.withValues(alpha: 0.09)
                : Colors.transparent,
            borderRadius: BorderRadius.circular(7),
            child: InkWell(
              key: ValueKey('third-party-license-${record.packageName}'),
              borderRadius: BorderRadius.circular(7),
              onTap: () => setState(() {
                _selectedPackage = record.packageName;
              }),
              child: Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 8,
                ),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(
                        record.packageName,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: selected ? _primary : _text,
                          fontSize: 12,
                          fontWeight: selected
                              ? NautermFontWeights.semibold
                              : NautermFontWeights.regular,
                        ),
                      ),
                    ),
                    if (record.licenses.length > 1)
                      Text(
                        tr('${record.licenses.length}'),
                        style: TextStyle(color: _faintText, fontSize: 10),
                      ),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }

  Widget _buildLicenseBody(_ThirdPartyPackageRecord? record) {
    if (record == null) {
      return Center(
        child: Text(
          tr(
            'settings.about.licenses.selectPrompt',
            fallback: 'Select a package to view its license.',
          ),
          style: TextStyle(color: _mutedText, fontSize: 12),
        ),
      );
    }
    return Column(
      key: const ValueKey('third-party-license-detail'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(18, 14, 18, 12),
          child: Text(
            record.packageName,
            style: TextStyle(
              color: _text,
              fontSize: 14,
              fontWeight: NautermFontWeights.semibold,
            ),
          ),
        ),
        if (record.licenses.length > 1)
          Padding(
            padding: const EdgeInsets.fromLTRB(18, 0, 18, 12),
            child: Text(
              tr('${record.licenses.length} license notices'),
              style: TextStyle(color: _mutedText, fontSize: 11, height: 1.35),
            ),
          ),
        Divider(height: 1, color: _softOutline),
        Expanded(
          child: ListView.separated(
            padding: const EdgeInsets.all(18),
            itemCount: record.licenses.length,
            separatorBuilder: (_, _) => Padding(
              padding: const EdgeInsets.symmetric(vertical: 18),
              child: Divider(height: 1, color: _softOutline),
            ),
            itemBuilder: (context, index) {
              return SelectableText(
                record.licenses[index].body,
                style: TextStyle(
                  color: _mutedText,
                  fontFamily: 'monospace',
                  fontSize: 11,
                  height: 1.5,
                ),
              );
            },
          ),
        ),
      ],
    );
  }
}
