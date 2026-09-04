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
import 'terminal_shell_integration.dart';
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

bool writeNativeShellIntegrationResources(String dataDirectory) {
  final nativeDirectory = dataDirectory.toNativeUtf8();
  try {
    return _TerminalBindings.open().writeShellIntegrationResources(
      nativeDirectory,
    );
  } finally {
    calloc.free(nativeDirectory);
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

@immutable
class Fido2DeviceInfo {
  const Fido2DeviceInfo({
    required this.id,
    required this.name,
    required this.vendorId,
    required this.productId,
    required this.hasPin,
  });

  factory Fido2DeviceInfo.fromJson(Map<String, dynamic> json) {
    return Fido2DeviceInfo(
      id: json['id'] as String,
      name: json['name'] as String,
      vendorId: json['vendorId'] as int,
      productId: json['productId'] as int,
      hasPin: json['hasPin'] as bool? ?? false,
    );
  }

  final String id;
  final String name;
  final int vendorId;
  final int productId;
  final bool hasPin;
}

@immutable
class Fido2GeneratedKey {
  const Fido2GeneratedKey({
    required this.privateKey,
    required this.publicKey,
    required this.keyType,
    required this.application,
  });

  factory Fido2GeneratedKey.fromJson(Map<String, dynamic> json) {
    return Fido2GeneratedKey(
      privateKey: json['privateKey'] as String,
      publicKey: json['publicKey'] as String,
      keyType: json['keyType'] as String,
      application: json['application'] as String,
    );
  }

  final String privateKey;
  final String publicKey;
  final String keyType;
  final String application;
}

Future<List<Fido2DeviceInfo>> listFido2Devices() {
  return Isolate.run(_listFido2DevicesSync);
}

Future<void> verifyFido2Pin({required String deviceId, required String pin}) {
  return Isolate.run(
    () =>
        _verifyFido2PinSync(<String, Object>{'deviceId': deviceId, 'pin': pin}),
  );
}

Future<Fido2GeneratedKey> generateFido2Key({
  required String deviceId,
  required String label,
  String keyType = 'ecdsa',
  String pin = '',
  bool requireUserPresence = true,
  bool requireUserVerification = false,
  bool resident = false,
  String passphrase = '',
  String cipher = 'aes256-ctr',
  int rounds = 100,
}) {
  return Isolate.run(
    () => _generateFido2KeySync(<String, Object>{
      'deviceId': deviceId,
      'label': label,
      'keyType': keyType,
      'pin': pin,
      'requireUserPresence': requireUserPresence,
      'requireUserVerification': requireUserVerification,
      'resident': resident,
      'passphrase': passphrase,
      'cipher': cipher,
      'rounds': rounds,
    }),
  );
}

List<Fido2DeviceInfo> _listFido2DevicesSync() {
  final bindings = _TerminalBindings.open();
  final response = _readFido2Response(bindings, bindings.fido2ListDevices());
  final value = response['value'];
  if (value is! List<Object?>) {
    return const <Fido2DeviceInfo>[];
  }
  return value
      .whereType<Map<Object?, Object?>>()
      .map((item) => Fido2DeviceInfo.fromJson(item.cast<String, dynamic>()))
      .toList(growable: false);
}

void _verifyFido2PinSync(Map<String, Object> request) {
  final bindings = _TerminalBindings.open();
  final nativeRequest = jsonEncode(request).toNativeUtf8();
  try {
    _readFido2Response(bindings, bindings.fido2VerifyPin(nativeRequest));
  } finally {
    malloc.free(nativeRequest);
  }
}

Fido2GeneratedKey _generateFido2KeySync(Map<String, Object> request) {
  final bindings = _TerminalBindings.open();
  final nativeRequest = jsonEncode(request).toNativeUtf8();
  try {
    final response = _readFido2Response(
      bindings,
      bindings.fido2Generate(nativeRequest),
    );
    final value = response['value'];
    if (value is! Map<Object?, Object?>) {
      throw StateError('The authenticator returned an invalid key.');
    }
    return Fido2GeneratedKey.fromJson(value.cast<String, dynamic>());
  } finally {
    malloc.free(nativeRequest);
  }
}

Map<String, dynamic> _readFido2Response(
  _TerminalBindings bindings,
  Pointer<Utf8> pointer,
) {
  if (pointer == nullptr) {
    throw StateError('The FIDO2 native operation failed.');
  }
  try {
    final decoded = jsonDecode(pointer.toDartString());
    if (decoded is! Map<Object?, Object?>) {
      throw const FormatException('Invalid FIDO2 native response.');
    }
    final response = decoded.cast<String, dynamic>();
    if (response['ok'] != true) {
      throw StateError(
        response['error'] as String? ?? 'The FIDO2 operation failed.',
      );
    }
    return response;
  } finally {
    bindings.freeString(pointer);
  }
}
