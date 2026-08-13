import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:math';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart' show compute;
import 'package:flutter/material.dart';

import '../app/nauterm_log.dart';
import 'terminal_driver.dart';
import 'terminal_config.dart';
import 'terminal_models.dart';
import 'terminal_selection.dart';
import 'terminal_theme.dart';

part 'terminal_ffi_native_types.dart';
part 'terminal_ffi_operations.dart';
part 'native_terminal_driver.dart';
part 'native_replay_terminal_driver.dart';
part 'terminal_ffi_bindings.dart';

bool _nativeTerminalRuntimeShutdown = false;

void initializeNativeTerminalRuntime() {
  final operation = NautermLog.begin('native', 'Initialize terminal runtime');
  try {
    _TerminalBindings.open().initializeRuntime();
    _nativeTerminalRuntimeShutdown = false;
    operation.succeed();
  } on Object catch (error, stackTrace) {
    operation.fail(error, stackTrace: stackTrace);
    // Missing native artifacts are reported when a terminal is opened.
  }
}

void shutdownNativeTerminalRuntime() {
  if (_nativeTerminalRuntimeShutdown) {
    return;
  }
  _nativeTerminalRuntimeShutdown = true;
  final operation = NautermLog.begin('native', 'Shut down terminal runtime');
  try {
    _TerminalBindings.open().prepareRuntimeShutdown();
    operation.succeed();
  } on Object catch (error, stackTrace) {
    operation.fail(error, stackTrace: stackTrace);
    // Shutdown remains best-effort when native artifacts are unavailable.
  }
}
