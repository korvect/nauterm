part of 'nauterm_workspace.dart';

const _hostIconNames = <String, String>{
  'linux': 'Linux',
  'windows': 'Windows',
  'apple': 'macOS',
  'ubuntu': 'Ubuntu',
  'debian': 'Debian',
  'fedora': 'Fedora',
  'redhat': 'Red Hat',
  'centos': 'CentOS',
  'almalinux': 'AlmaLinux',
  'rockylinux': 'Rocky Linux',
  'alpine': 'Alpine Linux',
  'arch': 'Arch Linux',
  'manjaro': 'Manjaro',
  'gentoo': 'Gentoo',
  'opensuse': 'openSUSE',
  'suse': 'SUSE',
  'linuxmint': 'Linux Mint',
  'kalilinux': 'Kali Linux',
  'nixos': 'NixOS',
  'popos': 'Pop!_OS',
  'elementary': 'elementary OS',
  'zorin': 'Zorin OS',
  'mxlinux': 'MX Linux',
  'voidlinux': 'Void Linux',
  'slackware': 'Slackware',
  'garudalinux': 'Garuda Linux',
  'endeavouros': 'EndeavourOS',
  'artixlinux': 'Artix Linux',
  'cachyos': 'CachyOS',
  'nobaralinux': 'Nobara Linux',
  'asahilinux': 'Asahi Linux',
  'grapheneos': 'GrapheneOS',
  'qubesos': 'Qubes OS',
  'reactos': 'ReactOS',
  'freebsd': 'FreeBSD',
  'openbsd': 'OpenBSD',
  'netbsd': 'NetBSD',
  'bsd': 'BSD',
  'proxmox': 'Proxmox',
  'vmware': 'VMware',
  'truenas': 'TrueNAS',
  'unraid': 'Unraid',
  'synology': 'Synology',
  'qnap': 'QNAP',
  'openwrt': 'OpenWrt',
  'opnsense': 'OPNsense',
  'pfsense': 'pfSense',
  'mikrotik': 'MikroTik',
  'vyos': 'VyOS',
  'cisco': 'Cisco',
  'huawei': 'Huawei',
  'fortinet': 'Fortinet',
  'h3c': 'H3C',
  'ruijie': 'Ruijie',
  'zte': 'ZTE',
  'network': 'Network device',
  'android': 'Android',
  'raspberrypi': 'Raspberry Pi',
};

const _hostIconAliases = <String, String>{
  'apple': 'apple darwin osx mac os 苹果',
  'windows': 'microsoft win 微软',
  'linux': 'gnu',
  'redhat': 'rhel',
  'popos': 'pop os pop_os',
  'vmware': 'esxi',
  'truenas': 'freenas',
  'mikrotik': 'routeros',
  'fortinet': 'fortigate',
  'huawei': '华为',
  'cisco': '思科',
  'h3c': 'comware 新华三 华三',
  'ruijie': 'rgos 锐捷',
  'zte': '中兴',
  'synology': '群晖',
  'qnap': '威联通',
  'raspberrypi': 'raspi 树莓派',
  'network': '网络设备',
};

String? _resolveHostIconSlug(String? customIcon, String? os, String? distro) =>
    _hostIconNames.containsKey(customIcon)
    ? customIcon
    : _BrandIcon._resolveOsSlug(os, distro);

@visibleForTesting
Widget buildHostIconPreviewForTesting(HostEntry host, {bool editing = false}) {
  final item = _mapHost(host, const [], const {});
  return _BrandIcon(
    icon: item.icon,
    color: item.color,
    name: item.name,
    os: item.os,
    distro: item.distro,
    customIcon: item.customIcon,
    mode: editing ? HostIconMode.osIcon : null,
  );
}

class _HostIconPickerButton extends StatefulWidget {
  const _HostIconPickerButton({
    required this.name,
    required this.icon,
    required this.color,
    required this.value,
    required this.onChanged,
    this.os,
    this.distro,
  });

  final String name;
  final IconData icon;
  final Color color;
  final String? value;
  final String? os;
  final String? distro;
  final ValueChanged<String?> onChanged;

  @override
  State<_HostIconPickerButton> createState() => _HostIconPickerButtonState();
}

class _HostIconPickerButtonState extends State<_HostIconPickerButton> {
  final _anchorKey = GlobalKey();
  final _overlayToken = Object();
  final _focusNode = FocusNode();
  NautermTransientOverlayHandle? _menu;
  bool _hovered = false;

  @override
  void didUpdateWidget(covariant _HostIconPickerButton oldWidget) {
    super.didUpdateWidget(oldWidget);
    _menu?.markNeedsBuild();
  }

  @override
  void dispose() {
    _menu?.dismiss(notify: false);
    _focusNode.dispose();
    super.dispose();
  }

  void _toggle() {
    if (_menu != null) {
      _menu!.dismiss();
      return;
    }
    _focusNode.requestFocus();
    _menu = showNautermTransientOverlay(
      context: context,
      token: _overlayToken,
      dismissExisting: true,
      builder: _buildMenu,
      onDismissed: () {
        _menu = null;
        if (mounted) setState(() {});
      },
    );
    setState(() {});
  }

  Widget _buildMenu(BuildContext context) {
    final anchor = _anchorKey.currentContext?.findRenderObject() as RenderBox?;
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    if (anchor == null || !anchor.attached || !anchor.hasSize) {
      return const SizedBox.shrink();
    }
    final origin = anchor.localToGlobal(Offset.zero, ancestor: overlay);
    final padding = nautermTransientOverlaySafePadding(
      context,
      base: MediaQuery.paddingOf(context),
    );
    final width = math.min(296.0, overlay.size.width - padding.horizontal - 16);
    final below = math.max(
      0.0,
      overlay.size.height -
          padding.bottom -
          origin.dy -
          anchor.size.height -
          16,
    );
    final above = math.max(0.0, origin.dy - padding.top - 16);
    final showAbove = below < 396 && above > below;
    final height = math.min(396.0, showAbove ? above : below);
    final rect = positionNautermTransientOverlay(
      context: context,
      overlaySize: overlay.size,
      safePadding: padding,
      preferredRect: Rect.fromLTWH(
        origin.dx,
        showAbove ? origin.dy - height - 8 : origin.dy + anchor.size.height + 8,
        width,
        height,
      ),
    );
    void close() => _menu?.dismiss();
    return Stack(
      children: [
        Positioned.fill(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: close,
          ),
        ),
        Positioned.fromRect(
          rect: rect,
          child: CallbackShortcuts(
            bindings: {const SingleActivator(LogicalKeyboardKey.escape): close},
            child: _HostIconPickerMenu(
              value: widget.value,
              onSelected: (icon) {
                widget.onChanged(icon);
                close();
                _focusNode.requestFocus();
              },
            ),
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final highlighted = _hovered || _menu != null;
    final label = tr('workspace.action.changeIcon', fallback: 'Change icon');
    return CallbackShortcuts(
      bindings: {
        if (_menu != null)
          const SingleActivator(LogicalKeyboardKey.escape): () =>
              _menu?.dismiss(),
      },
      child: Tooltip(
        message: label,
        excludeFromSemantics: true,
        waitDuration: const Duration(milliseconds: 350),
        child: Semantics(
          button: true,
          label: label,
          child: SizedBox(
            key: _anchorKey,
            width: 40,
            height: 40,
            child: Material(
              color: Colors.transparent,
              child: InkWell(
                key: const ValueKey('host-icon-picker'),
                focusNode: _focusNode,
                borderRadius: BorderRadius.circular(10),
                onHover: (value) => setState(() => _hovered = value),
                onTap: _toggle,
                child: Stack(
                  clipBehavior: Clip.none,
                  children: [
                    AnimatedContainer(
                      duration: const Duration(milliseconds: 150),
                      foregroundDecoration: BoxDecoration(
                        borderRadius: BorderRadius.circular(10),
                        border: Border.all(
                          color: highlighted ? _blue : Colors.transparent,
                          width: 1.5,
                        ),
                      ),
                      child: _BrandIcon(
                        icon: widget.icon,
                        color: widget.color,
                        name: widget.name,
                        os: widget.os,
                        distro: widget.distro,
                        customIcon: widget.value,
                        mode: HostIconMode.osIcon,
                      ),
                    ),
                    Positioned(
                      top: -4,
                      right: -4,
                      child: AnimatedContainer(
                        key: const ValueKey('host-icon-edit-indicator'),
                        duration: const Duration(milliseconds: 150),
                        width: 17,
                        height: 17,
                        decoration: BoxDecoration(
                          color: _card,
                          shape: BoxShape.circle,
                          border: Border.all(
                            color: highlighted ? _blue : _sidebarDivider,
                          ),
                        ),
                        child: Icon(
                          Icons.edit_outlined,
                          size: 10,
                          color: highlighted ? _blue : _mutedText,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _HostIconPickerMenu extends StatefulWidget {
  const _HostIconPickerMenu({required this.value, required this.onSelected});

  final String? value;
  final ValueChanged<String?> onSelected;

  @override
  State<_HostIconPickerMenu> createState() => _HostIconPickerMenuState();
}

class _HostIconPickerMenuState extends State<_HostIconPickerMenu> {
  final _search = TextEditingController();
  final _scroll = ScrollController();

  @override
  void initState() {
    super.initState();
    _search.addListener(_filter);
  }

  void _filter() {
    if (_scroll.hasClients) _scroll.jumpTo(0);
    setState(() {});
  }

  @override
  void dispose() {
    _search.removeListener(_filter);
    _search.dispose();
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final query = _search.text.trim().toLowerCase();
    final entries =
        <MapEntry<String?, String>>[
              MapEntry(
                null,
                tr(
                  'workspace.action.useAutomaticIcon',
                  fallback: 'Use automatic icon',
                ),
              ),
              ..._hostIconNames.entries,
            ]
            .where((entry) {
              final text =
                  '${entry.key ?? 'auto automatic 自动'} ${entry.value} ${_hostIconAliases[entry.key] ?? ''}'
                      .toLowerCase();
              return query.isEmpty || text.contains(query);
            })
            .toList(growable: false);
    return Material(
      color: Colors.transparent,
      child: NautermDropdownSurface(
        child: Column(
          key: const ValueKey('host-icon-menu'),
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 12, 12, 8),
              child: Text(
                tr('workspace.label.hostIcon', fallback: 'Host icon'),
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: _text,
                ),
              ),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10),
              child: KeyedSubtree(
                key: const ValueKey('host-icon-search'),
                child: _WorkspaceInput(
                  controller: _search,
                  label: tr(
                    'workspace.label.searchIcons',
                    fallback: 'Search icons',
                  ),
                  floatingLabel: false,
                  autofocus: true,
                  size: _WorkspaceControlSize.medium,
                  onSubmitted: (_) {
                    if (entries.length == 1) {
                      widget.onSelected(entries.single.key);
                    }
                  },
                ),
              ),
            ),
            const SizedBox(height: 8),
            Expanded(
              child: entries.isEmpty
                  ? Center(
                      child: Text(
                        tr(
                          'workspace.label.noMatchingIcons',
                          fallback: 'No matching icons',
                        ),
                        style: TextStyle(fontSize: 13, color: _mutedText),
                      ),
                    )
                  : Scrollbar(
                      controller: _scroll,
                      child: GridView.builder(
                        controller: _scroll,
                        padding: const EdgeInsets.fromLTRB(10, 2, 10, 10),
                        gridDelegate:
                            const SliverGridDelegateWithMaxCrossAxisExtent(
                              maxCrossAxisExtent: 46,
                              mainAxisSpacing: 6,
                              crossAxisSpacing: 6,
                            ),
                        itemCount: entries.length,
                        itemBuilder: (context, index) {
                          final entry = entries[index];
                          final automatic = entry.key == null;
                          final label = entry.value;
                          final selected = widget.value == entry.key;
                          return Tooltip(
                            message: label,
                            excludeFromSemantics: true,
                            waitDuration: const Duration(milliseconds: 350),
                            child: Semantics(
                              label: label,
                              selected: selected,
                              button: true,
                              child: InkWell(
                                key: automatic
                                    ? const ValueKey('host-icon-automatic')
                                    : ValueKey('host-icon-option:${entry.key}'),
                                borderRadius: BorderRadius.circular(9),
                                onTap: () => widget.onSelected(entry.key),
                                child: Container(
                                  decoration: BoxDecoration(
                                    color: automatic
                                        ? _sidebarHover
                                        : _BrandIcon._osBrandColor(entry.key!),
                                    borderRadius: BorderRadius.circular(9),
                                  ),
                                  foregroundDecoration: BoxDecoration(
                                    borderRadius: BorderRadius.circular(9),
                                    border: Border.all(
                                      color: selected
                                          ? _blue
                                          : Colors.transparent,
                                      width: 1.5,
                                    ),
                                  ),
                                  child: Padding(
                                    padding: const EdgeInsets.all(5),
                                    child: automatic
                                        ? Icon(
                                            Icons.auto_mode_rounded,
                                            size: 28,
                                            color: selected
                                                ? _blue
                                                : _mutedText,
                                          )
                                        : SvgPicture.asset(
                                            'assets/icons/os/system-${entry.key}.svg',
                                            colorFilter: const ColorFilter.mode(
                                              Colors.white,
                                              BlendMode.srcIn,
                                            ),
                                          ),
                                  ),
                                ),
                              ),
                            ),
                          );
                        },
                      ),
                    ),
            ),
          ],
        ),
      ),
    );
  }
}
