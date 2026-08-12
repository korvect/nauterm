import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/system_font_catalog.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('com.korvect.nauterm/system_fonts');

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  test('loads, normalizes, and sorts system monospace font families', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          expect(call.method, 'listMonospaceFamilies');
          return ['JetBrains Mono', ' Menlo ', 'Menlo', ''];
        });

    expect(await loadMonospaceFontFamilies(), ['JetBrains Mono', 'Menlo']);
  });

  test('loads, normalizes, and sorts all system font families', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          expect(call.method, 'listFontFamilies');
          return [' PingFang SC ', 'Menlo', 'PingFang SC', ''];
        });

    expect(await loadSystemFontFamilies(), ['Menlo', 'PingFang SC']);
  });

  test('recognizes programming families without admitting CJK text fonts', () {
    for (final family in [
      'Cascadia Code',
      'Cascadia Mono',
      'JetBrains Mono',
      'Iosevka Term',
      'Sarasa Mono SC',
      'LXGW WenKai Mono',
    ]) {
      expect(isLikelyTerminalFontFamily(family), isTrue, reason: family);
    }
    for (final family in [
      'monospace',
      'Courier',
      'Fixedsys',
      'FangSong',
      'NSimSun',
      'KaiTi',
      'SimHei',
      '仿宋',
      '新宋体',
      '楷体',
      '黑体',
    ]) {
      expect(isLikelyTerminalFontFamily(family), isFalse, reason: family);
    }
  });

  test(
    'uses safe fallback fonts when native enumeration is unavailable',
    () async {
      expect(await loadMonospaceFontFamilies(), fallbackMonospaceFontFamilies);
    },
  );
}
