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
            ? 'settings.general.connectionLinks.queryFailed'
            : null;
      });
    } on Object {
      if (!mounted || revision != _queryRevision) return;
      setState(() {
        _handlers = const {};
        _loading = false;
        _errorKey = 'settings.general.connectionLinks.queryFailed';
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
            ? 'settings.general.connectionLinks.notRegistered'
            : 'settings.general.connectionLinks.setFailed';
      });
    } finally {
      if (mounted) setState(() => _setting = null);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          tr('settings.general.connectionLinks.description'),
          style: TextStyle(color: _faintText, fontSize: 12, height: 1.35),
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 28,
          runSpacing: 12,
          children: [
            for (final scheme in ConnectionUriChannel.schemes)
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    '$scheme://',
                    style: TextStyle(
                      color: _text,
                      fontSize: 14,
                      fontWeight: NautermFontWeights.medium,
                    ),
                  ),
                  const SizedBox(width: 8),
                  _statusButton(scheme),
                ],
              ),
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
    );
  }

  Widget _statusButton(String scheme) {
    final isDefault = _handlers[scheme];
    final busy = _setting == scheme || (_loading && isDefault == null);
    final tooltip = tr(
      busy
          ? 'settings.general.connectionLinks.checking'
          : isDefault == true
          ? 'settings.general.connectionLinks.isDefault'
          : defaultTargetPlatform == TargetPlatform.windows
          ? 'settings.general.connectionLinks.openSettings'
          : 'settings.general.connectionLinks.setDefault',
      args: {'scheme': scheme},
    );
    return Semantics(
      value: busy
          ? tr('settings.general.connectionLinks.checking')
          : isDefault == null
          ? tr('settings.general.connectionLinks.queryFailed')
          : tr(
              isDefault
                  ? 'settings.general.connectionLinks.isDefault'
                  : 'settings.general.connectionLinks.setDefault',
              args: {'scheme': scheme},
            ),
      child: IconButton(
        key: ValueKey('settings-default-handler:$scheme'),
        tooltip: tooltip,
        onPressed: _loading || _setting != null
            ? null
            : () => unawaited(_setDefault(scheme)),
        padding: const EdgeInsets.all(4),
        constraints: const BoxConstraints.tightFor(width: 32, height: 32),
        icon: Container(
          key: ValueKey('settings-default-handler-status:$scheme'),
          width: 22,
          height: 22,
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
                  isDefault == true ? Icons.check_rounded : Icons.close_rounded,
                  size: 16,
                  color: Colors.white,
                ),
        ),
      ),
    );
  }
}
