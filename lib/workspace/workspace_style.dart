part of 'nauterm_workspace.dart';

bool get _workspaceDark {
  return switch (appThemeMode) {
    AppThemeMode.dark => true,
    AppThemeMode.light => false,
    AppThemeMode.system =>
      WidgetsBinding.instance.platformDispatcher.platformBrightness ==
          Brightness.dark,
  };
}

Color get _topBar =>
    _workspaceDark ? const Color(0xff282828) : const Color(0xff363c51);
Color get _topBarForeground =>
    _workspaceDark ? NautermPalette.dark.mutedText : const Color(0xffaeb6c8);
Color get _topBarTabActive =>
    _workspaceDark ? const Color(0xff3a3a3a) : const Color(0xff4a5065);
Color get _topBarTabInactive =>
    _workspaceDark ? const Color(0xff303030) : const Color(0xff40465b);
Color get _sidebar =>
    _workspaceDark ? const Color(0xff282828) : const Color(0xfff6f9f9);
Color get _sidebarDivider =>
    _workspaceDark ? NautermPalette.dark.outline : const Color(0xffdbe6e8);
Color get _sidebarHover =>
    _workspaceDark ? const Color(0xff383838) : const Color(0xffedf4f5);
Color get _sidebarPressed =>
    _workspaceDark ? const Color(0xff444444) : const Color(0xffc7d5d8);
Color get _surface =>
    _workspaceDark ? NautermPalette.dark.background : const Color(0xffedf3f3);
Color get _card =>
    _workspaceDark ? NautermPalette.dark.surface : const Color(0xffffffff);
Color get _cardHover => _workspaceDark
    ? NautermPalette.dark.surfaceContainer
    : const Color(0xfffbfdfd);
Color get _mutedText =>
    _workspaceDark ? NautermPalette.dark.mutedText : const Color(0xff8ca0a6);
Color get _text =>
    _workspaceDark ? NautermPalette.dark.text : const Color(0xff151927);
Color get _blue =>
    _workspaceDark ? NautermPalette.dark.primary : const Color(0xff168df2);
Color get _orange =>
    _workspaceDark ? const Color(0xffff9f0a) : const Color(0xffff5425);
Color get _green =>
    _workspaceDark ? NautermPalette.dark.secondary : const Color(0xff35d394);
Color get _topBarDestructiveHover =>
    _workspaceDark ? const Color(0xffe05545) : const Color(0xffe05545);
const double _topBarHeight = 44;
const double _macTrafficLightInset = 76;
const double _linuxWindowControlsEndPadding = 6;
const double _linuxWindowControlsWidth = 108;
const double _sidebarHorizontalInset = 10;
const double _sidebarIconSlotSize = 36;
const double _sidebarIconLabelGap = 10;
const double _sidebarExpandedWidth = 190;
const double _sidebarCollapsedWidth =
    _sidebarHorizontalInset * 2 + _sidebarIconSlotSize;
const double _sidebarCollapseBreakpoint = 760;
const double _sidebarItemHeight = 40;
const double _workspaceToolbarHeight = 48;
const double _workspaceDrawerHeaderHeight = _workspaceToolbarHeight;
const double _workspaceEditorDrawerWidth = 360;
const double _workspaceFormFieldGap = 10;
const EdgeInsets _workspacePanePadding = EdgeInsets.fromLTRB(26, 22, 26, 30);

const double _contextMenuRowHeight = 35;
const double _contextMenuDividerHeight = 13;
const double _contextMenuVerticalPadding = 20;
const double _contextMenuWidth = 286;
const double _contextSubmenuWidth = 190;
const Duration _contextMenuAnimationDuration = Duration(milliseconds: 78);
const Duration _workspacePageTransitionDuration = Duration(milliseconds: 130);
Color get _workspaceMenuBackground => _workspaceDark
    ? NautermPalette.dark.surfaceContainer
    : const Color(0xffffffff);
Color get _workspaceMenuBorder =>
    _workspaceDark ? NautermPalette.dark.outline : const Color(0xffd9e3e6);
Color get _workspaceMenuHover =>
    _workspaceDark ? const Color(0xff3e3e3e) : const Color(0xfff0f4f5);
Color get _workspaceMenuPressed =>
    _workspaceDark ? const Color(0xff484848) : const Color(0xffe9f1f3);
Color get _workspaceMenuDisabledText =>
    _workspaceDark ? const Color(0xff737373) : const Color(0xffc2d0d5);
ButtonStyle? get _workspaceIconButtonInteractionStyle => _workspaceDark
    ? IconButton.styleFrom(
        hoverColor: _sidebarHover,
        highlightColor: _workspaceMenuPressed,
      )
    : null;
const double _workspacePopupMenuRowHeight = 34;
const double _workspaceMenuHorizontalInset = 7;
const double _workspaceMenuVerticalInset = 5;
List<BoxShadow> get _workspaceMenuShadows => _workspaceDark
    ? const [
        BoxShadow(
          color: Color(0x66202020),
          blurRadius: 18,
          offset: Offset(0, 8),
        ),
        BoxShadow(
          color: Color(0x33202020),
          blurRadius: 4,
          offset: Offset(0, 1),
        ),
      ]
    : const [
        BoxShadow(
          color: Color(0x1f22313f),
          blurRadius: 18,
          offset: Offset(0, 8),
        ),
        BoxShadow(
          color: Color(0x0f22313f),
          blurRadius: 4,
          offset: Offset(0, 1),
        ),
      ];

double _contextMenuHeightForRows(Iterable<Object> rows) {
  return rows.fold<double>(
    _contextMenuVerticalPadding,
    (height, row) =>
        height +
        (row == _MenuDivider.instance
            ? _contextMenuDividerHeight
            : _contextMenuRowHeight),
  );
}

double _contextMenuTopForRow(List<Object> rows, _ContextMenuAction target) {
  var top = _contextMenuVerticalPadding / 2;
  for (final row in rows) {
    if (row is _ContextMenuAction && _sameContextMenuAction(row, target)) {
      return top;
    }
    top += row == _MenuDivider.instance
        ? _contextMenuDividerHeight
        : _contextMenuRowHeight;
  }
  return _contextMenuVerticalPadding / 2;
}

bool _sameContextMenuAction(
  _ContextMenuAction action,
  _ContextMenuAction other,
) {
  return action.id == other.id &&
      action.label == other.label &&
      action.icon == other.icon &&
      action.shortcut == other.shortcut &&
      action.destructive == other.destructive &&
      action.submenuActions.length == other.submenuActions.length;
}

Color _blend(Color background, Color foreground, double amount) {
  return Color.lerp(background, foreground, amount) ?? background;
}
