import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ui/nauterm_context_menu.dart';
import 'package:nauterm/ui/nauterm_overlay.dart';

void main() {
  testWidgets('dialogs are mutually exclusive within the same overlay', (
    tester,
  ) async {
    final controller = NautermOverlayController();
    addTearDown(controller.dispose);
    late BuildContext dialogContext;

    await tester.pumpWidget(
      MaterialApp(
        home: NautermOverlayScope(
          controller: controller,
          child: Builder(
            builder: (context) {
              dialogContext = context;
              return const SizedBox();
            },
          ),
        ),
      ),
    );

    final first = showNautermDialog<bool>(
      context: dialogContext,
      builder: (_) => const Center(child: Text('First dialog')),
    );
    await tester.pump();
    expect(find.text('First dialog'), findsOneWidget);

    final second = showNautermDialog<bool>(
      context: dialogContext,
      builder: (_) => const Center(child: Text('Second dialog')),
    );
    await tester.pump();

    expect(find.text('First dialog'), findsOneWidget);
    expect(find.text('Second dialog'), findsNothing);
    expect(await second, isNull);

    controller.dismissTransientOverlays();
    await tester.pump();
    expect(await first, isNull);
  });

  testWidgets('defers overlay refreshes requested during build', (
    tester,
  ) async {
    final hostKey = GlobalKey<_OverlayRefreshHostState>();

    await tester.pumpWidget(
      MaterialApp(home: _OverlayRefreshHost(key: hostKey)),
    );

    hostKey.currentState!.showOverlay();
    await tester.pump();
    expect(find.text('Overlay 0'), findsOneWidget);

    hostKey.currentState!.refreshOverlayDuringBuild();
    await tester.pump();
    await tester.pump();

    expect(tester.takeException(), isNull);
    expect(find.text('Overlay 1'), findsOneWidget);
  });

  testWidgets('dropdown menus cap visible rows and scroll overflow', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) {
            return Center(
              child: FilledButton(
                onPressed: () {
                  showNautermDropdownMenu<int>(
                    context: context,
                    anchor: const Rect.fromLTWH(100, 100, 120, 32),
                    width: 160,
                    entries: [
                      for (var index = 0; index < 20; index++)
                        NautermContextMenuAction(
                          value: index,
                          label: 'Option $index',
                        ),
                    ],
                    showScrollbarOnHover: true,
                  );
                },
                child: const Text('Open'),
              ),
            );
          },
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pump();

    final menu = find.byType(NautermContextMenu<int>);
    expect(menu, findsOneWidget);
    expect(find.byType(NautermAnchoredDropdownOverlay), findsOneWidget);
    expect(tester.getSize(menu).height, lessThanOrEqualTo(236));
    expect(
      find.descendant(of: menu, matching: find.byType(ListView)),
      findsOneWidget,
    );
    final scrollbar = find.descendant(
      of: menu,
      matching: find.byType(Scrollbar),
    );
    expect(scrollbar, findsOneWidget);
    expect(tester.widget<Scrollbar>(scrollbar).thumbVisibility, isFalse);
    expect(tester.getTopLeft(scrollbar).dx, tester.getTopLeft(menu).dx);
    expect(tester.getBottomRight(scrollbar).dx, tester.getBottomRight(menu).dx);

    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer();
    await mouse.moveTo(tester.getCenter(menu));
    await tester.pump();
    expect(tester.widget<Scrollbar>(scrollbar).thumbVisibility, isTrue);

    await mouse.moveTo(Offset.zero);
    await tester.pump();
    expect(tester.widget<Scrollbar>(scrollbar).thumbVisibility, isFalse);
    await mouse.removePointer();
  });

  testWidgets('context menu can reopen on consecutive secondary clicks', (
    tester,
  ) async {
    var openCount = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) {
            return Center(
              child: GestureDetector(
                key: const ValueKey('context-target'),
                behavior: HitTestBehavior.opaque,
                onSecondaryTapDown: (details) {
                  openCount++;
                  showNautermContextMenu<int>(
                    context: context,
                    position: details.globalPosition,
                    entries: const [
                      NautermContextMenuAction(value: 1, label: 'Action'),
                    ],
                  );
                },
                child: const SizedBox(width: 120, height: 60),
              ),
            );
          },
        ),
      ),
    );

    final target = find.byKey(const ValueKey('context-target'));
    await tester.tap(target, buttons: kSecondaryMouseButton);
    await tester.pump();
    expect(openCount, 1);
    expect(find.byType(NautermContextMenu<int>), findsOneWidget);

    await tester.tap(target, buttons: kSecondaryMouseButton);
    await tester.pump();
    expect(openCount, 2);
    expect(find.byType(NautermContextMenu<int>), findsOneWidget);
  });

  testWidgets('context menus stay below the main-window top bar', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: NautermOverlaySafeAreaScope(
          padding: const EdgeInsets.only(top: 44),
          child: Builder(
            builder: (context) => FilledButton(
              onPressed: () {
                showNautermContextMenu<int>(
                  context: context,
                  position: const Offset(10, 10),
                  animate: false,
                  entries: [
                    for (var index = 0; index < 30; index++)
                      NautermContextMenuAction(
                        value: index,
                        label: 'Safe action $index',
                      ),
                  ],
                );
              },
              child: const Text('Open safely'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open safely'));
    await tester.pump();

    final menuRect = tester.getRect(find.byType(NautermContextMenu<int>));
    expect(menuRect.top, greaterThanOrEqualTo(52));
    expect(menuRect.bottom, lessThanOrEqualTo(592));
    expect(
      find.descendant(
        of: find.byType(NautermContextMenu<int>),
        matching: find.byType(ListView),
      ),
      findsOneWidget,
    );
  });

  testWidgets('context menu fades in without scaling by default', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) {
            return Center(
              child: FilledButton(
                onPressed: () {
                  showNautermContextMenu<int>(
                    context: context,
                    position: const Offset(100, 100),
                    entries: const [
                      NautermContextMenuAction(value: 1, label: 'Action'),
                    ],
                  );
                },
                child: const Text('Open immediately'),
              ),
            );
          },
        ),
      ),
    );

    await tester.tap(find.text('Open immediately'));
    await tester.pump();

    final menu = find.byType(NautermContextMenu<int>);
    expect(menu, findsOneWidget);
    expect(
      find.ancestor(
        of: menu,
        matching: find.byWidgetPredicate(
          (widget) => widget is TweenAnimationBuilder<double>,
        ),
      ),
      findsOneWidget,
    );
    expect(
      find.ancestor(of: menu, matching: find.byType(Transform)),
      findsNothing,
    );
  });

  testWidgets('dropdown menu fades in without scaling by default', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) {
            return Center(
              child: FilledButton(
                onPressed: () {
                  showNautermDropdownMenu<int>(
                    context: context,
                    anchor: const Rect.fromLTWH(100, 100, 120, 32),
                    width: 160,
                    entries: const [
                      NautermContextMenuAction(value: 1, label: 'Action'),
                    ],
                  );
                },
                child: const Text('Open dropdown'),
              ),
            );
          },
        ),
      ),
    );

    await tester.tap(find.text('Open dropdown'));
    await tester.pump();

    final menu = find.byType(NautermContextMenu<int>);
    expect(menu, findsOneWidget);
    expect(
      find.ancestor(
        of: menu,
        matching: find.byWidgetPredicate(
          (widget) => widget is TweenAnimationBuilder<double>,
        ),
      ),
      findsOneWidget,
    );
    expect(
      find.ancestor(of: menu, matching: find.byType(Transform)),
      findsNothing,
    );
  });

  testWidgets('dropdown menus expose nested actions', (tester) async {
    int? selected;
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => Center(
            child: FilledButton(
              onPressed: () {
                showNautermDropdownMenu<int>(
                  context: context,
                  anchor: const Rect.fromLTWH(100, 100, 120, 32),
                  width: 180,
                  entries: const [
                    NautermContextMenuAction(
                      value: 0,
                      label: 'With sudo',
                      children: [
                        NautermContextMenuAction(value: 1, label: 'Upload...'),
                      ],
                    ),
                  ],
                ).then((value) => selected = value);
              },
              child: const Text('Open action menu'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open action menu'));
    await tester.pump();
    await tester.tap(find.text('With sudo'));
    await tester.pump();

    expect(find.byType(NautermContextMenu<int>), findsNWidgets(2));
    expect(find.text('Upload...'), findsOneWidget);

    await tester.tap(find.text('Upload...'));
    await tester.pump();
    expect(selected, 1);
  });

  testWidgets('context menu submenu returns the selected child action', (
    tester,
  ) async {
    int? selected;
    final applicationIcon = base64Decode(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk'
      '+A8AAQUBAScY42YAAAAASUVORK5CYII=',
    );
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => Center(
            child: FilledButton(
              onPressed: () {
                showNautermContextMenu<int>(
                  context: context,
                  position: const Offset(100, 100),
                  animate: false,
                  entries: [
                    NautermContextMenuAction(
                      value: 0,
                      label: 'Open With',
                      children: [
                        NautermContextMenuAction(
                          value: 1,
                          label: 'Preview',
                          iconBytes: applicationIcon,
                        ),
                      ],
                    ),
                  ],
                ).then((value) => selected = value);
              },
              child: const Text('Show submenu'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Show submenu'));
    await tester.pump();
    await tester.tap(find.text('Open With'));
    await tester.pump();

    expect(find.byType(NautermContextMenu<int>), findsNWidgets(2));
    expect(find.text('Preview'), findsOneWidget);
    expect(find.byType(Image), findsOneWidget);

    await tester.tap(find.text('Preview'));
    await tester.pump();
    expect(selected, 1);
    expect(find.byType(NautermContextMenu<int>), findsNothing);
  });

  testWidgets('context menu supports nested submenus at any depth', (
    tester,
  ) async {
    int? selected;
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => Center(
            child: FilledButton(
              onPressed: () {
                showNautermContextMenu<int>(
                  context: context,
                  position: const Offset(100, 100),
                  animate: false,
                  entries: const [
                    NautermContextMenuAction(
                      value: 0,
                      label: 'With sudo',
                      children: [
                        NautermContextMenuAction(
                          value: 1,
                          label: 'Open With',
                          children: [
                            NautermContextMenuAction(
                              value: 2,
                              label: 'Preview',
                            ),
                          ],
                        ),
                      ],
                    ),
                  ],
                ).then((value) => selected = value);
              },
              child: const Text('Show nested submenu'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Show nested submenu'));
    await tester.pump();
    await tester.tap(find.text('With sudo'));
    await tester.pump();
    await tester.tap(find.text('Open With'));
    await tester.pump();

    expect(find.byType(NautermContextMenu<int>), findsNWidgets(3));
    expect(find.text('Preview'), findsOneWidget);

    await tester.tap(find.text('Preview'));
    await tester.pump();
    expect(selected, 2);
    expect(find.byType(NautermContextMenu<int>), findsNothing);
  });

  testWidgets('submenu stays open during diagonal pointer travel', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => Center(
            child: FilledButton(
              onPressed: () {
                showNautermContextMenu<int>(
                  context: context,
                  position: const Offset(100, 100),
                  animate: false,
                  entries: const [
                    NautermContextMenuAction(
                      value: 0,
                      label: 'Open With',
                      children: [
                        NautermContextMenuAction(value: 1, label: 'Preview'),
                      ],
                    ),
                    NautermContextMenuAction(value: 2, label: 'Rename'),
                  ],
                );
              },
              child: const Text('Show hover submenu'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Show hover submenu'));
    await tester.pump();

    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer();
    await mouse.moveTo(tester.getCenter(find.text('Open With')));
    await tester.pump();
    expect(find.byType(NautermContextMenu<int>), findsNWidgets(2));

    await mouse.moveTo(tester.getCenter(find.text('Rename')));
    await tester.pump(const Duration(milliseconds: 30));
    expect(find.byType(NautermContextMenu<int>), findsNWidgets(2));

    await mouse.moveTo(tester.getCenter(find.text('Preview')));
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(NautermContextMenu<int>), findsNWidgets(2));

    await mouse.moveTo(tester.getCenter(find.text('Rename')));
    await tester.pump(const Duration(milliseconds: 80));
    expect(find.byType(NautermContextMenu<int>), findsOneWidget);
  });
}

class _OverlayRefreshHost extends StatefulWidget {
  const _OverlayRefreshHost({super.key});

  @override
  State<_OverlayRefreshHost> createState() => _OverlayRefreshHostState();
}

class _OverlayRefreshHostState extends State<_OverlayRefreshHost> {
  NautermTransientOverlayHandle? _overlay;
  var _revision = 0;
  var _refreshDuringBuild = false;

  void showOverlay() {
    _overlay = showNautermTransientOverlay(
      context: context,
      token: Object(),
      builder: (_) => Material(child: Text('Overlay $_revision')),
    );
  }

  void refreshOverlayDuringBuild() {
    setState(() {
      _revision++;
      _refreshDuringBuild = true;
    });
  }

  @override
  void dispose() {
    _overlay?.dismiss(notify: false);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_refreshDuringBuild) {
      _refreshDuringBuild = false;
      _overlay?.markNeedsBuild();
    }
    return const SizedBox.shrink();
  }
}
