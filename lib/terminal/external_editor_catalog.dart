import 'dart:io';

import 'package:flutter/services.dart';

import 'terminal_config.dart';

const MethodChannel _externalEditorsChannel = MethodChannel(
  'com.korvect.nauterm/external_editors',
);

const _unsafeSystemDefaultApplicationExtensions = <String>{
  'app',
  'appimage',
  'appref-ms',
  'appx',
  'appxbundle',
  'apk',
  'bash',
  'bat',
  'cmd',
  'com',
  'command',
  'cpl',
  'csh',
  'deb',
  'desktop',
  'exe',
  'fish',
  'gadget',
  'hta',
  'jar',
  'js',
  'jse',
  'ksh',
  'lnk',
  'msi',
  'msp',
  'msix',
  'msixbundle',
  'mpkg',
  'package',
  'pif',
  'pkg',
  'pl',
  'ps1',
  'py',
  'pyw',
  'rb',
  'reg',
  'rpm',
  'run',
  'scf',
  'scr',
  'sh',
  'url',
  'vb',
  'vbe',
  'vbs',
  'workflow',
  'wsf',
  'wsh',
  'zsh',
};

class SystemFileApplication {
  const SystemFileApplication({
    required this.name,
    required this.command,
    this.iconBytes,
    this.isDefault = false,
  });

  final String name;
  final SftpExternalEditorCommand command;
  final Uint8List? iconBytes;
  final bool isDefault;
}

/// Whether opening [fileName] through the operating system's implicit file
/// association is safe enough to offer.
///
/// Explicitly choosing an editor is intentionally outside this policy.
bool canOpenFileWithSystemDefaultApplication(
  String fileName, {
  String permissions = '',
}) {
  final normalizedPermissions = permissions.trim();
  if (normalizedPermissions.length >= 10) {
    for (final index in const [3, 6, 9]) {
      final permission = normalizedPermissions[index];
      if (permission == 'x' || permission == 's' || permission == 't') {
        return false;
      }
    }
  }

  final leafName = fileName.replaceAll('\\', '/').split('/').last;
  final dotIndex = leafName.lastIndexOf('.');
  if (dotIndex < 0 || dotIndex == leafName.length - 1) {
    return true;
  }
  final extension = leafName.substring(dotIndex + 1).toLowerCase();
  return !_unsafeSystemDefaultApplicationExtensions.contains(extension);
}

Future<List<SystemFileApplication>> loadSystemFileApplications(
  List<String> fileExtensions,
) async {
  if (!Platform.isMacOS) {
    return const [];
  }
  final values = await _externalEditorsChannel.invokeMethod<List<Object?>>(
    'listFileApplications',
    {'extensions': fileExtensions},
  );
  return decodeSystemFileApplications(values);
}

Future<List<SystemFileApplication>> loadSystemApplicationsForFileName(
  String fileName,
) {
  final dotIndex = fileName.lastIndexOf('.');
  if (dotIndex < 0 || dotIndex == fileName.length - 1) {
    return Future.value(const []);
  }
  return loadSystemFileApplications([
    fileName.substring(dotIndex + 1).toLowerCase(),
  ]);
}

String? systemApplicationBundleIdentifier(SftpExternalEditorCommand command) {
  final executable = command.executable.trim();
  if (executable != 'open' && executable != '/usr/bin/open') {
    return null;
  }
  for (var index = 0; index < command.arguments.length - 1; index++) {
    if (command.arguments[index] == '-b') {
      final bundleIdentifier = command.arguments[index + 1].trim();
      return bundleIdentifier.isEmpty ? null : bundleIdentifier;
    }
  }
  return null;
}

Future<SystemFileApplication?> loadSystemApplication(
  SftpExternalEditorCommand command,
) async {
  if (!Platform.isMacOS) {
    return null;
  }
  final bundleIdentifier = systemApplicationBundleIdentifier(command);
  if (bundleIdentifier == null) {
    return null;
  }
  try {
    final value = await _externalEditorsChannel
        .invokeMapMethod<String, Object?>('fileApplication', {
          'bundleIdentifier': bundleIdentifier,
        });
    if (value == null) return null;
    final applications = decodeSystemFileApplications([value]);
    return applications.isEmpty ? null : applications.single;
  } on MissingPluginException {
    return null;
  } on PlatformException {
    return null;
  }
}

Future<SystemFileApplication?> chooseSystemFileApplication() async {
  if (!Platform.isMacOS) {
    return null;
  }
  try {
    final value = await _externalEditorsChannel
        .invokeMapMethod<String, Object?>('chooseFileApplication');
    if (value == null) return null;
    final applications = decodeSystemFileApplications([value]);
    return applications.isEmpty ? null : applications.single;
  } on MissingPluginException {
    return null;
  } on PlatformException {
    return null;
  }
}

Future<void> openFileWithSystemApplication(
  String path, {
  SftpExternalEditorCommand? application,
}) async {
  final bundleIdentifier = application == null
      ? null
      : systemApplicationBundleIdentifier(application);
  if (Platform.isMacOS && (application == null || bundleIdentifier != null)) {
    final arguments = <String, Object?>{'path': path};
    if (bundleIdentifier != null) {
      arguments['bundleIdentifier'] = bundleIdentifier;
    }
    await _externalEditorsChannel.invokeMethod<void>('openFile', arguments);
    return;
  }
  if (application != null) {
    final executable = resolveSftpExternalEditorExecutable(
      application.executable,
    );
    await Process.start(executable, [
      ...application.arguments,
      path,
    ], mode: ProcessStartMode.detached);
    return;
  }
  if (Platform.isWindows) {
    await Process.start('explorer.exe', [
      path,
    ], mode: ProcessStartMode.detached);
    return;
  }
  await Process.start('xdg-open', [path], mode: ProcessStartMode.detached);
}

List<SystemFileApplication> decodeSystemFileApplications(
  List<Object?>? values,
) {
  if (values == null) {
    return const [];
  }
  final applications = <SystemFileApplication>[];
  final seen = <String>{};
  for (final value in values) {
    if (value is! Map) {
      continue;
    }
    final name = value['name']?.toString().trim() ?? '';
    final bundleIdentifier = value['bundleIdentifier']?.toString().trim() ?? '';
    if (name.isEmpty ||
        bundleIdentifier.isEmpty ||
        !seen.add(bundleIdentifier)) {
      continue;
    }
    final rawIcon = value['icon'];
    applications.add(
      SystemFileApplication(
        name: name,
        command: SftpExternalEditorCommand(
          label: name,
          executable: '/usr/bin/open',
          arguments: ['-b', bundleIdentifier],
        ),
        iconBytes: rawIcon is Uint8List ? rawIcon : null,
        isDefault: value['isDefault'] == true,
      ),
    );
  }
  applications.sort(
    (left, right) =>
        left.name.toLowerCase().compareTo(right.name.toLowerCase()),
  );
  return applications;
}
