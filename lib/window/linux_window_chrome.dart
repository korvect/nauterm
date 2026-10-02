import 'dart:ffi' as ffi;

import 'package:flutter/services.dart';

const _windowChannel = MethodChannel('com.korvect.nauterm/window');

Future<void> startLinuxWindowDrag(
  ffi.Pointer<ffi.Void> window,
  ffi.Pointer<ffi.Void> view,
) => _windowChannel.invokeMethod<void>('startWindowDrag', {
  'window': window.address,
  'view': view.address,
});

final _gtk = ffi.DynamicLibrary.open('libgtk-3.so.0');
final _newBox = _gtk
    .lookupFunction<
      ffi.Pointer<ffi.Void> Function(ffi.Int32, ffi.Int32),
      ffi.Pointer<ffi.Void> Function(int, int)
    >('gtk_box_new');
final _setTitlebar = _gtk
    .lookupFunction<
      ffi.Void Function(ffi.Pointer<ffi.Void>, ffi.Pointer<ffi.Void>),
      void Function(ffi.Pointer<ffi.Void>, ffi.Pointer<ffi.Void>)
    >('gtk_window_set_titlebar');

void configureLinuxWindowChrome(ffi.Pointer<ffi.Void> gtkWindow) {
  if (gtkWindow != ffi.nullptr) {
    // A custom titlebar enables GTK's native CSD border, shadow and resize grips.
    // Leave it hidden: even an empty visible box gets titlebar height from the
    // GTK theme. Flutter supplies the visible top bar instead. Configure this
    // before rendering content and before nativeapi wraps the GDK surface.
    final titlebar = _newBox(0, 0);
    _setTitlebar(gtkWindow, titlebar);
  }
}
