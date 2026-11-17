part of 'settings_panel.dart';

class _ConnectionLinkDefaults extends StatefulWidget {
  const _ConnectionLinkDefaults();

  @override
  State<_ConnectionLinkDefaults> createState() =>
      _ConnectionLinkDefaultsState();
}

class _ConnectionLinkDefaultsState extends State<_ConnectionLinkDefaults>
    with WidgetsBindingObserver {
  Map<String, bool?> _handlers = const {};
  String? _setting;
  String? _errorKey;
  bool _loading = true;
  int _queryRevision = 0;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    unawaited(_refresh());
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) unawaited(_refresh());
  }

  @override
  void didChangeViewFocus(ViewFocusEvent event) {
    if (event.state == ViewFocusState.focused) unawaited(_refresh());
  }

  Future<void> _refresh() async {
    final revision = ++_queryRevision;
    try {
      final handlers = await ConnectionUriChannel.defaultHandlers();
      if (!mounted || revision != _queryRevision) return;
      setState(() {
        _handlers = handlers;
        _loading = false;
        _errorKey = handlers.values.any((value) => value == null)
            ? 'settings.general.defaultUrlHandler.queryFailed'
            : null;
      });
    } on Object {
      if (!mounted || revision != _queryRevision) return;
      setState(() {
        _handlers = const {};
        _loading = false;
        _errorKey = 'settings.general.defaultUrlHandler.queryFailed';
      });
    }
  }

  Future<void> _setDefault(String scheme) async {
    if (_setting != null) return;
    setState(() {
      _setting = scheme;
      _errorKey = null;
    });
    try {
      await ConnectionUriChannel.setDefaultHandler(scheme);
      await _refresh();
    } on Object catch (error) {
      if (!mounted) return;
      setState(() {
        _errorKey = error is PlatformException && error.code == 'not_registered'
            ? 'settings.general.defaultUrlHandler.notRegistered'
            : 'settings.general.defaultUrlHandler.setFailed';
      });
    } finally {
      if (mounted) setState(() => _setting = null);
    }
  }

  @override
  Widget build(BuildContext context) {
    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: _settingsContentMaxWidth),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            tr('settings.general.defaultUrlHandler.description'),
            style: TextStyle(color: _mutedText, fontSize: 12, height: 1.35),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              for (final scheme in ConnectionUriChannel.schemes) ...[
                if (scheme != ConnectionUriChannel.schemes.first)
                  const SizedBox(width: 12),
                Expanded(child: _protocolButton(scheme)),
              ],
            ],
          ),
          if (_errorKey != null) ...[
            const SizedBox(height: 8),
            Text(
              tr(_errorKey!),
              key: const ValueKey('settings-connection-links-error'),
              style: const TextStyle(color: Color(0xffef4444), fontSize: 12),
            ),
          ],
        ],
      ),
    );
  }

  Widget _protocolButton(String scheme) {
    final isDefault = _handlers[scheme];
    final busy = _setting == scheme || (_loading && isDefault == null);
    final tooltip = tr(
      busy
          ? 'settings.general.defaultUrlHandler.checking'
          : isDefault == true
          ? 'settings.general.defaultUrlHandler.isDefault'
          : defaultTargetPlatform == TargetPlatform.windows
          ? 'settings.general.defaultUrlHandler.openSettings'
          : 'settings.general.defaultUrlHandler.setDefault',
      args: {'scheme': scheme},
    );
    return Tooltip(
      message: tooltip,
      child: Semantics(
        value: busy
            ? tr('settings.general.defaultUrlHandler.checking')
            : isDefault == null
            ? tr('settings.general.defaultUrlHandler.queryFailed')
            : tr(
                isDefault
                    ? 'settings.general.defaultUrlHandler.isDefault'
                    : 'settings.general.defaultUrlHandler.setDefault',
                args: {'scheme': scheme},
              ),
        child: TextButton(
          key: ValueKey('settings-default-handler:$scheme'),
          onPressed: _loading || _setting != null
              ? null
              : () => unawaited(_setDefault(scheme)),
          style: TextButton.styleFrom(
            foregroundColor: _text,
            disabledForegroundColor: _faintText,
            minimumSize: const Size.fromHeight(40),
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(7),
            ),
            visualDensity: VisualDensity.standard,
            tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            textStyle: TextStyle(
              fontSize: _settingsFieldFontSize,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
          child: Row(
            children: [
              Expanded(child: Text('$scheme://')),
              const SizedBox(width: 12),
              Container(
                key: ValueKey('settings-default-handler-status:$scheme'),
                width: 20,
                height: 20,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: busy
                      ? _faintText
                      : isDefault == true
                      ? const Color(0xff22c55e)
                      : const Color(0xffef4444),
                ),
                alignment: Alignment.center,
                child: busy
                    ? const SizedBox.square(
                        dimension: 12,
                        child: CircularProgressIndicator(
                          strokeWidth: 2,
                          color: Colors.white,
                        ),
                      )
                    : Icon(
                        isDefault == true
                            ? Icons.check_rounded
                            : Icons.close_rounded,
                        size: 16,
                        color: Colors.white,
                      ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
