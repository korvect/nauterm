import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/ui/terminal_theme_preview.dart';

void main() {
  testWidgets('host search input uses medium control height', (tester) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.pump();

    final searchInput = find.byKey(const ValueKey('host-search-input'));
    expect(tester.getSize(searchInput).height, 32);

    final searchField = find.descendant(
      of: searchInput,
      matching: find.byType(TextField),
    );
    expect(tester.widget<TextField>(searchField).style?.fontSize, 14);
    expect(
      tester.widget<TextField>(searchField).decoration?.hintText,
      contains('tag:'),
    );
  });

  testWidgets('sidebar and workspace toolbar do not participate in tab', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.pump();

    final sidebar = find.byKey(
      const ValueKey('workspace-sidebar-tab-exclusion'),
    );
    final toolbar = find.byKey(
      const ValueKey('workspace-toolbar-tab-exclusion'),
    );
    expect(sidebar, findsOneWidget);
    expect(toolbar, findsOneWidget);

    final searchField = find.descendant(
      of: find.byKey(const ValueKey('host-search-input')),
      matching: find.byType(TextField),
    );
    await tester.tap(searchField);
    await tester.pump();

    for (var index = 0; index < 20; index++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(
        _primaryFocusIsInside(tester, sidebar),
        isFalse,
        reason: 'Tab entered the workspace sidebar at step $index.',
      );
      expect(
        _primaryFocusIsInside(tester, toolbar),
        isFalse,
        reason: 'Tab entered the workspace toolbar at step $index.',
      );
    }
  });

  testWidgets('host relation selects support text search', (tester) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final nameDecorator = _decoratorForLabel('Name');
    final typeDecorator = _decoratorForLabel('Type');
    final parentGroupDecorator = _decoratorForLabel('Parent group');
    expect(tester.getSize(nameDecorator).height, 40);
    expect(
      tester.getSize(parentGroupDecorator).height,
      tester.getSize(nameDecorator).height,
    );
    expect(
      tester.getRect(typeDecorator).top - tester.getRect(nameDecorator).bottom,
      10,
    );
    final nameContainerRect = _containerRectForLabel(tester, 'Name');
    final parentGroupContainerRect = _containerRectForLabel(
      tester,
      'Parent group',
    );
    expect(nameContainerRect.height, 40);
    expect(parentGroupContainerRect.height, nameContainerRect.height);
    final parentGroupArrow = find.descendant(
      of: _selectSuffixForLabel('Parent group'),
      matching: find.byIcon(Icons.keyboard_arrow_down_rounded),
    );
    expect(
      tester.getCenter(parentGroupArrow).dy,
      parentGroupContainerRect.center.dy,
    );
    expect(
      parentGroupContainerRect.right - tester.getCenter(parentGroupArrow).dx,
      19,
    );

    final nameField = _textFieldForLabel('Name');
    expect(tester.widget<TextField>(nameField).style?.fontSize, 14);
    expect(tester.widget<TextField>(nameField).textAlignVertical, isNull);
    expect(
      tester.widget<TextField>(nameField).style?.fontWeight,
      FontWeight.w400,
    );
    await tester.tapAt(
      Offset(nameContainerRect.right - 2, nameContainerRect.center.dy),
    );
    await tester.pump();
    expect(tester.widget<TextField>(nameField).focusNode?.hasFocus, isTrue);
    await tester.enterText(nameField, 'drone-runner01');
    await tester.pump();

    expect(find.byTooltip('Clear Name'), findsOneWidget);
    final nameClearSurface = find.descendant(
      of: find.byTooltip('Clear Name'),
      matching: find.byWidgetPredicate(
        (widget) => widget is Material && widget.shape is CircleBorder,
      ),
    );
    expect(tester.getSize(nameClearSurface), const Size.square(16));
    expect(tester.getCenter(nameClearSurface).dy, nameContainerRect.center.dy);
    expect(nameContainerRect.right - tester.getCenter(nameClearSurface).dx, 19);
    expect(nameContainerRect.right - tester.getRect(nameField).right, 31);
    final nameClearInkWell = find.descendant(
      of: find.byTooltip('Clear Name'),
      matching: find.byType(InkWell),
    );
    expect(tester.widget<InkWell>(nameClearInkWell).canRequestFocus, isFalse);
    expect(tester.widget<InkWell>(nameClearInkWell).hoverColor, isNotNull);
    expect(tester.widget<InkWell>(nameClearInkWell).highlightColor, isNotNull);
    expect(tester.widget<InkWell>(nameClearInkWell).splashColor, isNotNull);

    await tester.tap(find.byTooltip('Clear Name'));
    await tester.pump();

    expect(tester.widget<TextField>(nameField).controller!.text, isEmpty);
    expect(tester.widget<TextField>(nameField).focusNode?.hasFocus, isTrue);
    expect(find.byTooltip('Clear Name'), findsNothing);
    await tester.enterText(nameField, 'host-draft');
    await tester.pump();

    final parentGroupField = _textFieldForLabel('Parent group');
    expect(parentGroupField, findsOneWidget);
    expect(
      tester.widget<TextField>(parentGroupField).style?.fontWeight,
      FontWeight.w400,
    );

    await tester.tapAt(
      Offset(
        parentGroupContainerRect.right - 2,
        parentGroupContainerRect.center.dy,
      ),
    );
    await tester.pump();
    expect(
      tester.widget<TextField>(parentGroupField).focusNode?.hasFocus,
      isTrue,
    );
    expect(find.byTooltip('Clear Parent group'), findsNothing);
    await tester.enterText(parentGroupField, 'does-not-match');
    await tester.pump();
    await tester.pump();

    expect(find.text('Create group does-not-match'), findsOneWidget);
    expect(find.text('No matches'), findsNothing);
    expect(find.text('No group'), findsNothing);
    expect(find.byTooltip('Clear Parent group'), findsOneWidget);

    final selectMenu = find.byKey(
      const ValueKey('workspace-select-menu:Parent group'),
    );
    final menuRectBeforeScroll = tester.getRect(selectMenu);
    final fieldRectBeforeScroll = _containerRectForLabel(
      tester,
      'Parent group',
    );
    expect(menuRectBeforeScroll.top - fieldRectBeforeScroll.bottom, 4);

    final editorScrollable = find
        .descendant(
          of: find.byKey(const ValueKey('workspace-editor-scroll-view')),
          matching: find.byType(Scrollable),
        )
        .first;
    final scrollPosition = tester
        .state<ScrollableState>(editorScrollable)
        .position;
    scrollPosition.jumpTo(40);
    await tester.pump();
    await tester.pump();

    final menuRectAfterScroll = tester.getRect(selectMenu);
    final fieldRectAfterScroll = _containerRectForLabel(tester, 'Parent group');
    expect(menuRectAfterScroll.top - fieldRectAfterScroll.bottom, 4);
    expect(
      menuRectAfterScroll.top - menuRectBeforeScroll.top,
      fieldRectAfterScroll.top - fieldRectBeforeScroll.top,
    );

    await tester.tap(find.byTooltip('Clear Parent group'));
    await tester.pump();

    expect(
      tester.widget<TextField>(parentGroupField).controller!.text,
      isEmpty,
    );
    expect(find.byTooltip('Clear Parent group'), findsNothing);
    expect(find.text('Create group does-not-match'), findsNothing);

    await tester.enterText(parentGroupField, 'new-group');
    await tester.pump();
    await tester.tap(find.text('Create group new-group'));
    await tester.pump();

    expect(find.text('New Group'), findsOneWidget);
    final newGroupDrawer = find.byKey(const ValueKey<Object>('group:new:root'));
    final newGroupNameField = find
        .descendant(of: newGroupDrawer, matching: find.byType(TextField))
        .first;
    expect(
      tester.widget<TextField>(newGroupNameField).controller!.text,
      'new-group',
    );

    final addGroupProtocol = find.descendant(
      of: newGroupDrawer,
      matching: find.byKey(const ValueKey('add-protocol')),
    );
    tester.widget<InkWell>(addGroupProtocol).onTap!();
    await tester.pump();
    await tester.tap(find.text('SSH').last);
    await tester.pump();

    final groupThemePreview = find.descendant(
      of: newGroupDrawer,
      matching: find.byType(TerminalThemePreviewCard),
    );
    expect(groupThemePreview, findsOneWidget);
    await tester.tap(groupThemePreview);
    await tester.pump(const Duration(milliseconds: 160));
    expect(find.text('Themes'), findsOneWidget);
    await tester.tap(find.byTooltip('Back'));
    await tester.pump(const Duration(milliseconds: 160));

    await tester.tap(
      find.descendant(of: newGroupDrawer, matching: find.byTooltip('Close')),
    );
    await tester.pump();

    expect(find.text('New Group'), findsNothing);
    expect(find.text('New Host'), findsOneWidget);
    expect(
      tester.widget<TextField>(_textFieldForLabel('Name')).controller!.text,
      'host-draft',
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('workspace inputs and selected selects traverse with tab', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final nameField = _textFieldForLabel('Name');
    await tester.tap(nameField);
    await tester.enterText(nameField, 'filled-host-name');
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();

    final typeFocus = tester.widget<Focus>(
      find.byKey(const ValueKey('workspace-select-focus:Type')),
    );
    expect(typeFocus.focusNode?.hasFocus, isTrue);

    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    final parentGroupField = _textFieldForLabel('Parent group');
    expect(
      tester.widget<TextField>(parentGroupField).focusNode?.hasFocus,
      isTrue,
    );

    await tester.enterText(parentGroupField, 'existing search text');
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    final tagsField = _textFieldForLabel('Tags');
    expect(tester.widget<TextField>(tagsField).focusNode?.hasFocus, isTrue);

    await _expandSshMore(tester);
    final startupField = _textFieldForLabel('Startup snippet');
    for (
      var index = 0;
      tester.widget<TextField>(startupField).focusNode?.hasFocus != true &&
          index < 8;
      index++
    ) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    }
    expect(tester.widget<TextField>(startupField).focusNode?.hasFocus, isTrue);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pump();
    expect(tester.widget<TextField>(startupField).focusNode?.hasFocus, isFalse);
  });

  testWidgets('host name follows its active protocol until edited', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump(const Duration(milliseconds: 250));

    final host = _textFieldForLabel('Host');
    final hostController = tester.widget<TextField>(host).controller!;
    final portController = tester
        .widget<TextField>(_textFieldForLabel('Port'))
        .controller!;
    await tester.enterText(host, 'server.example.com');
    await tester.pump();
    final name = _textFieldForLabel('Name');
    final nameController = tester.widget<TextField>(name).controller!;
    expect(nameController.text, 'server.example.com:22');
    await _scrollEditorFieldIntoView(tester, 'Username');
    final username = _textFieldForLabel('Username');
    await tester.enterText(username, 'admin');
    await tester.pump();

    expect(nameController.text, 'admin:server.example.com:22');

    nameController.text = 'Production';
    hostController.text = 'other.example.com';
    await tester.pump();
    expect(nameController.text, 'Production');

    nameController.clear();
    await tester.pump();
    expect(nameController.text, isEmpty);

    portController.text = '2200';
    await tester.pump();
    expect(nameController.text, 'admin:other.example.com:2200');

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('host editor keeps tab traversal inside its focus region', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final focusRegion = find.byKey(
      const ValueKey('workspace-editor-focus-region:hosts:0'),
    );
    expect(focusRegion, findsOneWidget);
    expect(_primaryFocusIsInside(tester, focusRegion), isTrue);

    for (var index = 0; index < 24; index++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(
        _primaryFocusIsInside(tester, focusRegion),
        isTrue,
        reason: 'Tab left the editor focus region at step $index.',
      );
    }

    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    for (var index = 0; index < 24; index++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(
        _primaryFocusIsInside(tester, focusRegion),
        isTrue,
        reason: 'Shift+Tab left the editor focus region at step $index.',
      );
    }
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    FocusManager.instance.primaryFocus?.unfocus();
    final editorScrollable = find
        .descendant(
          of: find.byKey(const ValueKey('workspace-editor-scroll-view')),
          matching: find.byType(Scrollable),
        )
        .first;
    tester.state<ScrollableState>(editorScrollable).position.jumpTo(0);
    await tester.pump();
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump();
  });

  testWidgets('host relation selects can clear and create related records', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    await _expectCreateAndClear(
      tester,
      label: 'Parent group',
      createLabel: 'Create group',
    );
    await _expectCreateAndClear(
      tester,
      label: 'Username',
      createLabel: 'Create identity',
    );
    await _expandSshMore(tester);
    await _expectCreateAndClear(
      tester,
      label: 'Proxy',
      createLabel: 'Create proxy',
    );
    final addCredential = find.byKey(const ValueKey('ssh-add-credential'));
    await tester.ensureVisible(addCredential);
    expect(tester.getSize(addCredential).height, lessThan(36));
    await tester.tap(addCredential);
    await tester.pump();
    await tester.tap(find.text('Key').last);
    await tester.pump();
    final keyField = _textFieldForLabel('Key');
    tester.widget<TextField>(keyField).onTap!();
    await tester.enterText(keyField, 'missing-key');
    await tester.pump();
    expect(find.text('Create key missing-key'), findsOneWidget);
    expect(find.byKey(const ValueKey('ssh-remove-credential')), findsNothing);
    final clearKey = find.descendant(
      of: find.byTooltip('Clear Key'),
      matching: find.byType(InkWell),
    );
    tester.widget<InkWell>(clearKey).onTap!();
    await tester.pump();
    expect(find.byKey(const ValueKey('ssh-add-credential')), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('SSH username and password both expose identity search', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    await _scrollEditorFieldIntoView(tester, 'Username');
    final username = _textFieldForLabel('Username');
    final password = _textFieldForLabel('Password');
    expect(_selectSuffixForLabel('Username'), findsOneWidget);
    expect(_selectSuffixForLabel('Password'), findsOneWidget);
    expect(tester.widget<TextField>(password).obscureText, isTrue);

    tester.widget<TextField>(username).onTap!();
    await tester.enterText(username, 'shared-admin');
    await tester.pump();
    await tester.tap(find.text('Create identity shared-admin'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('New Identity'), findsOneWidget);
    expect(
      tester.widget<TextField>(_textFieldForLabel('Name')).controller!.text,
      'shared-admin',
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('SSH credential creation selects a key or certificate flow', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final addCredential = find.byKey(const ValueKey('ssh-add-credential'));
    await tester.ensureVisible(addCredential);
    await tester.tap(addCredential);
    await tester.pump();
    expect(find.text('Key'), findsWidgets);
    expect(find.text('Certificate'), findsOneWidget);
    expect(find.text('Key or Certificate'), findsOneWidget);

    await tester.tap(find.text('Certificate'));
    await tester.pump();
    expect(find.byKey(const ValueKey('ssh-remove-credential')), findsNothing);
    final certificate = _textFieldForLabel('Certificate');
    tester.widget<TextField>(certificate).onTap!();
    await tester.enterText(certificate, 'work-cert');
    await tester.pump();
    await tester.tap(find.text('Create certificate work-cert'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('New Key'), findsOneWidget);
    expect(find.text('Paste'), findsOneWidget);
    expect(find.text('Generate'), findsOneWidget);
    expect(
      tester.widget<TextField>(_textFieldForLabel('Name')).controller!.text,
      'work-cert',
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('related key creation exposes paste import and generate', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final addCredential = find.byKey(const ValueKey('ssh-add-credential'));
    await tester.ensureVisible(addCredential);
    await tester.tap(addCredential);
    await tester.pump();
    await tester.tap(find.text('Key'));
    await tester.pump();

    final key = _textFieldForLabel('Key');
    tester.widget<TextField>(key).onTap!();
    await tester.enterText(key, 'generated-test-key');
    await tester.pump();
    await tester.tap(find.text('Create key generated-test-key'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('New Key'), findsOneWidget);
    expect(find.text('Paste'), findsOneWidget);
    expect(find.text('Import'), findsOneWidget);
    expect(find.text('Generate'), findsOneWidget);

    await tester.tap(find.text('Generate'));
    await tester.pump();
    expect(find.text('Name'), findsOneWidget);
    expect(find.text('Key type'), findsOneWidget);
    expect(find.text('Generate & save'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('startup snippet supports nested creation', (tester) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));
    await _expandSshMore(tester);

    await _scrollEditorFieldIntoView(tester, 'Startup snippet');
    final startupSnippet = _textFieldForLabel('Startup snippet');
    tester.widget<TextField>(startupSnippet).onTap!();
    await tester.enterText(startupSnippet, 'Deploy app');
    await tester.pump();
    await tester.tap(find.text('Create snippet Deploy app'));
    await tester.pump();

    expect(find.text('New Snippet'), findsOneWidget);
    expect(
      tester.widget<TextField>(_textFieldForLabel('Script *')).controller!.text,
      'Deploy app',
    );
    await tester.tap(find.byTooltip('Close'));
    await tester.pump();
    expect(find.text('New Host'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('related editor drawers can nest and preserve parent drafts', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    await _expandSshMore(tester);
    await _scrollEditorFieldIntoView(tester, 'Proxy');
    final hostProxyField = _textFieldForLabel('Proxy');
    await tester.tap(hostProxyField);
    await tester.enterText(hostProxyField, 'proxy-draft');
    await tester.pump();
    await tester.tap(find.text('Create proxy proxy-draft'));
    await tester.pump();

    expect(find.text('New Proxy'), findsOneWidget);
    expect(
      tester
          .widget<TextField>(_textFieldForLabel('Proxy name *'))
          .controller!
          .text,
      'proxy-draft',
    );
    final proxyIdentityField = _textFieldForLabel('Username');
    await tester.tap(proxyIdentityField);
    await tester.enterText(proxyIdentityField, 'identity-draft');
    await tester.pump();
    await tester.tap(find.text('Create identity identity-draft'));
    await tester.pump();

    expect(find.text('New Identity'), findsOneWidget);
    expect(
      tester.widget<TextField>(_textFieldForLabel('Name')).controller!.text,
      'identity-draft',
    );
    final identityUsernameField = _textFieldForLabel('Username *');
    await tester.enterText(identityUsernameField, 'identity-user-draft');

    final addCredential = find.byKey(const ValueKey('ssh-add-credential'));
    await tester.tap(addCredential);
    await tester.pump();
    await tester.tap(find.text('Key').last);
    await tester.pump();
    final identityKeyField = _textFieldForLabel('Key');
    await tester.tap(identityKeyField);
    await tester.enterText(identityKeyField, 'key-draft');
    await tester.pump();
    await tester.tap(find.text('Create key key-draft'));
    await tester.pump();

    expect(find.text('New Key'), findsOneWidget);
    expect(
      tester.widget<TextField>(_textFieldForLabel('Name')).controller!.text,
      'key-draft',
    );

    await tester.tap(find.byTooltip('Close'));
    await tester.pump();

    expect(find.text('New Identity'), findsOneWidget);
    expect(
      tester
          .widget<TextField>(_textFieldForLabel('Username *'))
          .controller!
          .text,
      'identity-user-draft',
    );

    await tester.tap(find.byTooltip('Close'));
    await tester.pump();

    expect(find.text('New Proxy'), findsOneWidget);
    expect(
      tester.widget<TextField>(_textFieldForLabel('Username')).controller!.text,
      'identity-draft',
    );

    await tester.tap(find.byTooltip('Close'));
    await tester.pump();

    expect(find.text('New Host'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('sidebar pages retain independent editor drawer drafts', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump(const Duration(milliseconds: 250));

    await tester.enterText(_textFieldForLabel('Name'), 'host-draft');
    await tester.tap(_sidebarSectionFinder('Keychain'));
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('New Host'), findsNothing);
    await tester.tap(find.text('New key'));
    await tester.pump(const Duration(milliseconds: 250));
    await tester.enterText(_textFieldForLabel('Name'), 'key-draft');

    await tester.tap(_sidebarSectionFinder('Hosts'));
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('New Host'), findsOneWidget);
    expect(
      tester.widget<TextField>(_textFieldForLabel('Name')).controller!.text,
      'host-draft',
    );

    await tester.tap(_sidebarSectionFinder('Keychain'));
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('New Key'), findsOneWidget);
    expect(
      tester.widget<TextField>(_textFieldForLabel('Name')).controller!.text,
      'key-draft',
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('host environment variables edit in a nested drawer', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    await _expandSshMore(tester);
    final environmentField = find.byKey(
      const ValueKey('host-environment-field'),
    );
    final editorScrollable = find
        .descendant(
          of: find.byKey(const ValueKey('workspace-editor-scroll-view')),
          matching: find.byType(Scrollable),
        )
        .first;
    await tester.scrollUntilVisible(
      environmentField,
      500,
      scrollable: editorScrollable,
    );
    final editorPosition = tester
        .state<ScrollableState>(editorScrollable)
        .position;
    editorPosition.jumpTo(editorPosition.maxScrollExtent);
    await tester.pump();

    expect(tester.getSize(environmentField).height, 40);
    expect(
      find.descendant(
        of: environmentField,
        matching: find.text('Environment variables'),
      ),
      findsOneWidget,
    );
    final environmentButton = find
        .descendant(
          of: environmentField,
          matching: find.byType(GestureDetector),
        )
        .first;
    tester.widget<GestureDetector>(environmentButton).onTap!();
    await tester.pump();

    final environmentEditor = find.byKey(
      const ValueKey('host-environment-editor'),
    );
    expect(environmentEditor, findsOneWidget);
    expect(find.text('Environment Variables'), findsOneWidget);

    final addVariable = find.text('Add variable');
    final addVariableButton = find
        .ancestor(of: addVariable, matching: find.byType(InkWell))
        .first;
    expect(tester.getSize(addVariableButton).height, 40);
    await tester.tap(addVariable);
    await tester.pump();

    final removeButton = find.byTooltip('Remove variable');
    final removeOpacity = find
        .ancestor(of: removeButton, matching: find.byType(AnimatedOpacity))
        .first;
    expect(tester.widget<AnimatedOpacity>(removeOpacity).opacity, 0);

    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    addTearDown(mouse.removePointer);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(removeButton));
    await tester.pump(const Duration(milliseconds: 130));
    expect(tester.widget<AnimatedOpacity>(removeOpacity).opacity, 1);

    final environmentFields = find.descendant(
      of: environmentEditor,
      matching: find.byType(TextField),
    );
    expect(environmentFields, findsNWidgets(2));
    expect(
      tester
          .getSize(
            find
                .ancestor(
                  of: environmentFields.first,
                  matching: find.byType(InputDecorator),
                )
                .first,
          )
          .height,
      40,
    );
    await tester.enterText(environmentFields.at(0), 'LANG');
    await tester.enterText(environmentFields.at(1), 'en_US.UTF-8');

    await tester.tap(find.text('Save'));
    await tester.pump();

    expect(environmentEditor, findsNothing);
    expect(find.text('LANG=en_US.UTF-8'), findsOneWidget);

    tester.widget<GestureDetector>(environmentButton).onTap!();
    await tester.pump();
    final restoredFields = find.descendant(
      of: find.byKey(const ValueKey('host-environment-editor')),
      matching: find.byType(TextField),
    );
    expect(
      tester.widget<TextField>(restoredFields.at(0)).controller!.text,
      'LANG',
    );
    expect(
      tester.widget<TextField>(restoredFields.at(1)).controller!.text,
      'en_US.UTF-8',
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });
}

bool _primaryFocusIsInside(WidgetTester tester, Finder region) {
  final focusedContext = FocusManager.instance.primaryFocus?.context;
  if (focusedContext == null) {
    return false;
  }
  final regionElement = tester.element(region);
  if (identical(focusedContext, regionElement)) {
    return true;
  }
  var found = false;
  focusedContext.visitAncestorElements((element) {
    found = identical(element, regionElement);
    return !found;
  });
  return found;
}

Finder _textFieldForLabel(String label) {
  final decorator = _decoratorForLabel(label);
  return find.descendant(of: decorator, matching: find.byType(TextField)).first;
}

Finder _decoratorForLabel(String label) {
  return find
      .ancestor(of: find.text(label), matching: find.byType(InputDecorator))
      .first;
}

Rect _containerRectForLabel(WidgetTester tester, String label) {
  final field = _textFieldForLabel(label);
  final container = InputDecorator.containerOf(tester.element(field))!;
  return container.localToGlobal(Offset.zero) & container.size;
}

Finder _selectSuffixForLabel(String label) {
  return find.byKey(ValueKey('workspace-select-suffix:$label'));
}

Finder _sidebarSectionFinder(String label) {
  final tooltip = find.byTooltip(label);
  return tooltip.evaluate().isNotEmpty ? tooltip : find.text(label);
}

Future<void> _scrollEditorFieldIntoView(
  WidgetTester tester,
  String label,
) async {
  final targets = find.ancestor(
    of: find.text(label),
    matching: find.byType(InputDecorator),
  );
  final editorScrollable = find
      .descendant(
        of: find.byKey(const ValueKey('workspace-editor-scroll-view')),
        matching: find.byType(Scrollable),
      )
      .first;
  final position = tester.state<ScrollableState>(editorScrollable).position;
  position.jumpTo(0);
  await tester.pump();

  for (var index = 0; targets.evaluate().isEmpty && index < 12; index++) {
    final nextOffset = (position.pixels + 250)
        .clamp(0.0, position.maxScrollExtent)
        .toDouble();
    if (nextOffset == position.pixels) {
      break;
    }
    position.jumpTo(nextOffset);
    await tester.pump();
  }

  expect(targets, findsOneWidget);
  await tester.ensureVisible(targets.first);
  await tester.pump();
}

Future<void> _expandSshMore(WidgetTester tester) async {
  final button = find.byKey(const ValueKey('protocol-show-more')).first;
  if (button.evaluate().isEmpty) {
    return;
  }
  tester.widget<InkWell>(button).onTap!();
  await tester.pump();
}

Future<void> _expectCreateAndClear(
  WidgetTester tester, {
  required String label,
  required String createLabel,
}) async {
  final decorator = _decoratorForLabel(label);
  await tester.ensureVisible(decorator);
  await tester.pump();
  final editorScrollable = find
      .descendant(
        of: find.byKey(const ValueKey('workspace-editor-scroll-view')),
        matching: find.byType(Scrollable),
      )
      .first;
  final position = tester.state<ScrollableState>(editorScrollable).position;
  final fieldTop = tester.getRect(decorator).top;
  final visibleTop = tester.getRect(editorScrollable).top + 8;
  if (fieldTop < visibleTop) {
    position.jumpTo(
      (position.pixels - (visibleTop - fieldTop))
          .clamp(0.0, position.maxScrollExtent)
          .toDouble(),
    );
    await tester.pump();
  }
  await tester.pump();

  final field = _textFieldForLabel(label);
  tester.widget<TextField>(field).onTap!();
  await tester.pump();
  final query = 'missing-${label.toLowerCase()}';
  await tester.enterText(field, query);
  await tester.pump();
  await tester.pump();

  expect(find.text('$createLabel $query'), findsOneWidget);
  expect(find.byTooltip('Clear $label'), findsOneWidget);
  expect(find.text('No $label'), findsNothing);

  final clearInkWell = find.descendant(
    of: find.byTooltip('Clear $label'),
    matching: find.byType(InkWell),
  );
  tester.widget<InkWell>(clearInkWell).onTap!();
  await tester.pump();

  expect(tester.widget<TextField>(field).controller!.text, isEmpty);
  expect(find.text('$createLabel $query'), findsNothing);
  expect(find.byTooltip('Clear $label'), findsNothing);

  FocusManager.instance.primaryFocus?.unfocus();
  await tester.pump();
}
