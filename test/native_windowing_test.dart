import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/window_config.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/window/native_windowing.dart';
import 'package:nativeapi/nativeapi.dart' as native;
import 'package:nativeapi_flutter/nativeapi_flutter.dart' show SizeToNative;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const titleBarChannel = MethodChannel(
    'com.korvect.nauterm/title_bar_regions',
  );

  setUp(() {
    final previousWidth = terminalWindowWidth;
    final previousHeight = terminalWindowHeight;
    final previousX = terminalWindowX;
    final previousY = terminalWindowY;
    terminalWindowWidth = null;
    terminalWindowHeight = null;
    terminalWindowX = null;
    terminalWindowY = null;
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(titleBarChannel, (_) async => null);
    addTearDown(() {
      forgetMainWindow();
      terminalWindowWidth = previousWidth;
      terminalWindowHeight = previousHeight;
      terminalWindowX = previousX;
      terminalWindowY = previousY;
      debugDefaultTargetPlatformOverride = null;
      messenger.setMockMethodCallHandler(titleBarChannel, null);
    });
  });

  test(
    'new untitled main window receives native minimum size and geometry',
    () {
      final window = _FakeWindow();
      terminalWindowWidth = 600;
      terminalWindowHeight = 300;

      initializeMainNativeWindow(window, _FakeDisplay());

      expect(window.title, isEmpty);
      expect(window.minimumSize, mainWindowMinSize.toNative());
      expect(window.size, mainWindowMinSize.toNative());
      expect(window.contentUnderTitleBar, isTrue);
      expect(window.position.x, (1920 - mainWindowMinSize.width) / 2);
      expect(window.position.y, (1080 - mainWindowMinSize.height) / 2);
    },
  );

  test('new untitled settings window receives native minimum size', () {
    final window = _FakeWindow();

    initializeSettingsNativeWindow(window, _FakeDisplay());

    expect(window.title, isEmpty);
    expect(window.minimumSize, settingsWindowMinSize.toNative());
    expect(window.size, settingsWindowSize.toNative());
    expect(window.sizeUpdates, 1);
  });

  test(
    'showing an initialized window reapplies limits without resetting geometry',
    () {
      final display = _FakeDisplay();
      final main = _FakeWindow();
      final settings = _FakeWindow();
      initializeMainNativeWindow(main, display);
      initializeSettingsNativeWindow(settings, display);
      for (final window in [main, settings]) {
        window.minimumSize = const native.Size(width: 0, height: 0);
        window.setSize(const native.Size(width: 1200, height: 800), false);
        window.position = const native.Point(x: 30, y: 40);
      }

      initializeMainNativeWindow(main, display);
      initializeSettingsNativeWindow(settings, display);

      expect(main.minimumSize, mainWindowMinSize.toNative());
      expect(settings.minimumSize, settingsWindowMinSize.toNative());
      for (final window in [main, settings]) {
        expect(window.size, const native.Size(width: 1200, height: 800));
        expect(window.position, const native.Point(x: 30, y: 40));
        expect(window.sizeUpdates, 2);
      }
    },
  );

  test(
    'recreated settings window gets limits independently of the closed window',
    () {
      final display = _FakeDisplay();
      final first = _FakeWindow();
      initializeSettingsNativeWindow(first, display);
      final reopened = _FakeWindow();

      initializeSettingsNativeWindow(reopened, display);

      expect(reopened.id, isNot(first.id));
      expect(reopened.minimumSize, settingsWindowMinSize.toNative());
      expect(reopened.size, settingsWindowSize.toNative());
      expect(reopened.sizeUpdates, 1);
    },
  );

  test('minimum sizes do not depend on display availability', () {
    final main = _FakeWindow();
    final settings = _FakeWindow();

    initializeMainNativeWindow(main, null);
    initializeSettingsNativeWindow(settings, null);

    expect(main.minimumSize, mainWindowMinSize.toNative());
    expect(settings.minimumSize, settingsWindowMinSize.toNative());
    expect(main.sizeUpdates, 0);
    expect(settings.sizeUpdates, 0);

    initializeMainNativeWindow(main, _FakeDisplay());
    initializeSettingsNativeWindow(settings, _FakeDisplay());

    expect(main.sizeUpdates, 1);
    expect(settings.sizeUpdates, 1);
  });
}

class _FakeWindow extends Fake implements native.Window {
  _FakeWindow() : id = _nextId++;

  static int _nextId = 1;

  @override
  final int id;

  @override
  String title = '';

  @override
  native.Size minimumSize = const native.Size(width: 0, height: 0);

  @override
  native.Size size = const native.Size(width: 0, height: 0);

  @override
  native.Point position = const native.Point(x: 0, y: 0);

  @override
  bool isWindowControlButtonsVisible = false;

  bool contentUnderTitleBar = false;
  int sizeUpdates = 0;

  @override
  bool setContentUnderTitleBar(bool value) {
    contentUnderTitleBar = value;
    return true;
  }

  @override
  void setSize(native.Size value, bool animate) {
    size = value;
    sizeUpdates++;
  }
}

class _FakeDisplay extends Fake implements native.Display {
  @override
  native.Rectangle get workArea =>
      const native.Rectangle(x: 0, y: 0, width: 1920, height: 1080);
}
