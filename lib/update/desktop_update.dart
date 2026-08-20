import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:http/http.dart' as http;

import '../app/nauterm_log.dart';

const String defaultUpdateRepository = String.fromEnvironment(
  'NAUTERM_UPDATE_REPOSITORY',
);

enum DesktopUpdatePlatform { macos, windows, linux }

enum DesktopUpdateArchitecture { x86_64, arm64 }

enum DesktopUpdateAssetKind {
  macosArchive,
  windowsInstaller,
  deb,
  rpm,
  appImage,
}

enum DesktopUpdateInstallDisposition {
  installerLaunched,
  restartRequired,
  relaunchOnQuit,
}

typedef DesktopUpdateProcessRunner = Future<ProcessResult> Function(
  String executable,
  List<String> arguments,
);

class DesktopUpdateTarget {
  const DesktopUpdateTarget({
    required this.platform,
    required this.architecture,
    this.linuxPackagePreference,
  });

  final DesktopUpdatePlatform platform;
  final DesktopUpdateArchitecture architecture;
  final DesktopUpdateAssetKind? linuxPackagePreference;

  static DesktopUpdateTarget current() {
    final platform = switch (Platform.operatingSystem) {
      'macos' => DesktopUpdatePlatform.macos,
      'windows' => DesktopUpdatePlatform.windows,
      'linux' => DesktopUpdatePlatform.linux,
      final value => throw UnsupportedError(
        'Application updates are not supported on $value.',
      ),
    };
    final architecture = switch (Abi.current()) {
      Abi.macosArm64 => DesktopUpdateArchitecture.arm64,
      Abi.macosX64 => DesktopUpdateArchitecture.x86_64,
      Abi.linuxArm64 => DesktopUpdateArchitecture.arm64,
      Abi.windowsArm64 => DesktopUpdateArchitecture.arm64,
      Abi.windowsX64 || Abi.linuxX64 => DesktopUpdateArchitecture.x86_64,
      final abi => throw UnsupportedError(
        'Application updates are not supported on $abi.',
      ),
    };
    return DesktopUpdateTarget(
      platform: platform,
      architecture: architecture,
      linuxPackagePreference: platform == DesktopUpdatePlatform.linux
          ? _detectLinuxPackagePreference()
          : null,
    );
  }

  static DesktopUpdateAssetKind _detectLinuxPackagePreference() {
    if ((Platform.environment['APPIMAGE'] ?? '').isNotEmpty) {
      return DesktopUpdateAssetKind.appImage;
    }
    try {
      final osRelease = File('/etc/os-release')
          .readAsStringSync()
          .toLowerCase();
      if (osRelease.contains('id=ubuntu') ||
          osRelease.contains('id=debian') ||
          osRelease.contains('id=linuxmint') ||
          osRelease.contains('id_like=debian') ||
          osRelease.contains('id_like="debian')) {
        return DesktopUpdateAssetKind.deb;
      }
      if (osRelease.contains('id=fedora') ||
          osRelease.contains('id=rhel') ||
          osRelease.contains('id_like=fedora') ||
          osRelease.contains('id_like="fedora')) {
        return DesktopUpdateAssetKind.rpm;
      }
    } on FileSystemException {
      // AppImage is the portable fallback when distro metadata is unavailable.
    }
    return DesktopUpdateAssetKind.appImage;
  }
}

class DesktopUpdateAsset {
  const DesktopUpdateAsset({
    required this.name,
    required this.downloadUri,
    required this.size,
    required this.kind,
  });

  final String name;
  final Uri downloadUri;
  final int size;
  final DesktopUpdateAssetKind kind;
}

class DesktopUpdateRelease {
  const DesktopUpdateRelease({
    required this.version,
    required this.title,
    required this.notes,
    required this.releaseUri,
    required this.asset,
    required this.checksumsUri,
  });

  final String version;
  final String title;
  final String notes;
  final Uri releaseUri;
  final DesktopUpdateAsset asset;
  final Uri checksumsUri;
}

class DesktopUpdateCheck {
  const DesktopUpdateCheck({
    required this.currentVersion,
    required this.release,
  });

  final String currentVersion;
  final DesktopUpdateRelease? release;
  bool get updateAvailable => release != null;
}

class DesktopUpdateException implements Exception {
  const DesktopUpdateException(this.message);

  final String message;

  @override
  String toString() => message;
}

class DesktopUpdateService {
  DesktopUpdateService({
    http.Client? client,
    this.repository = defaultUpdateRepository,
    DesktopUpdateTarget? target,
    DesktopUpdateProcessRunner? processRunner,
  }) : _client = client ?? http.Client(),
       target = target ?? DesktopUpdateTarget.current(),
       _processRunner = processRunner ?? _runProcess;

  final http.Client _client;
  final DesktopUpdateProcessRunner _processRunner;
  final String repository;
  final DesktopUpdateTarget target;
  static const Duration _requestTimeout = Duration(seconds: 20);
  static const Duration _downloadIdleTimeout = Duration(seconds: 30);

  Future<DesktopUpdateCheck> check(String currentVersion) async {
    final operation = NautermLog.begin(
      'update',
      'Check for updates',
      fields: {
        'platform': target.platform.name,
        'architecture': target.architecture.name,
        'current_version': currentVersion,
      },
    );
    try {
      final result = await _check(currentVersion);
      operation.succeed(
        fields: {
          'update_available': result.updateAvailable,
          if (result.release != null) 'latest_version': result.release!.version,
        },
      );
      return result;
    } on Object catch (error, stackTrace) {
      operation.fail(error, stackTrace: stackTrace);
      rethrow;
    }
  }

  Future<DesktopUpdateCheck> _check(String currentVersion) async {
    if (repository.trim().isEmpty) {
      throw const DesktopUpdateException(
        'Application updates are not configured. Build with '
        '--dart-define=NAUTERM_UPDATE_REPOSITORY=owner/repository.',
      );
    }
    final response = await _client
        .get(
          Uri.https('api.github.com', '/repos/$repository/releases/latest'),
          headers: const {
            'Accept': 'application/vnd.github+json',
            'X-GitHub-Api-Version': '2022-11-28',
            'User-Agent': 'Nauterm-Desktop-Updater',
          },
        )
        .timeout(_requestTimeout);
    if (response.statusCode != 200) {
      throw DesktopUpdateException(
        'Unable to check for updates (HTTP ${response.statusCode}).',
      );
    }

    final json = jsonDecode(response.body);
    if (json is! Map<String, dynamic>) {
      throw const DesktopUpdateException('The release response is invalid.');
    }
    final release = parseGitHubRelease(json, target);
    if (compareVersions(release.version, currentVersion) <= 0) {
      return DesktopUpdateCheck(currentVersion: currentVersion, release: null);
    }
    return DesktopUpdateCheck(currentVersion: currentVersion, release: release);
  }

  Future<File> download(
    DesktopUpdateRelease release, {
    void Function(int received, int total)? onProgress,
  }) async {
    final operation = NautermLog.begin(
      'update',
      'Download update',
      fields: {
        'version': release.version,
        'asset_kind': release.asset.kind.name,
      },
    );
    try {
      final file = await _download(release, onProgress: onProgress);
      operation.succeed(fields: {'size_bytes': await file.length()});
      return file;
    } on Object catch (error, stackTrace) {
      operation.fail(error, stackTrace: stackTrace);
      rethrow;
    }
  }

  Future<File> _download(
    DesktopUpdateRelease release, {
    void Function(int received, int total)? onProgress,
  }) async {
    final checksums = await _loadChecksums(release.checksumsUri);
    final expected = checksums[release.asset.name];
    if (expected == null) {
      throw DesktopUpdateException(
        'The release does not contain a checksum for ${release.asset.name}.',
      );
    }

    final directory = await _downloadDirectory();
    final file = File('${directory.path}/${release.asset.name}');
    final request = http.Request('GET', release.asset.downloadUri)
      ..headers['User-Agent'] = 'Nauterm-Desktop-Updater';
    final response = await _client.send(request).timeout(_requestTimeout);
    if (response.statusCode != 200) {
      throw DesktopUpdateException(
        'Unable to download the update (HTTP ${response.statusCode}).',
      );
    }

    final sink = file.openWrite();
    final digestSink = _DigestSink();
    final hashSink = sha256.startChunkedConversion(digestSink);
    var received = 0;
    try {
      await for (final chunk in response.stream.timeout(_downloadIdleTimeout)) {
        sink.add(chunk);
        hashSink.add(chunk);
        received += chunk.length;
        onProgress?.call(
          received,
          response.contentLength ?? release.asset.size,
        );
      }
      await sink.flush();
    } on Object {
      await sink.close();
      hashSink.close();
      if (await file.exists()) await file.delete();
      rethrow;
    }
    await sink.close();
    hashSink.close();
    final actual = digestSink.value!.toString();
    if (actual.toLowerCase() != expected.toLowerCase()) {
      await file.delete();
      throw const DesktopUpdateException(
        'The downloaded update failed SHA-256 verification.',
      );
    }
    return file;
  }

  Future<DesktopUpdateInstallDisposition> launchInstaller(
    File file,
    DesktopUpdateAssetKind kind,
  ) async {
    final operation = NautermLog.begin(
      'update',
      'Launch update installer',
      fields: {'asset_kind': kind.name},
    );
    try {
      final disposition = await _launchInstaller(file, kind);
      operation.succeed();
      return disposition;
    } on Object catch (error, stackTrace) {
      operation.fail(error, stackTrace: stackTrace);
      rethrow;
    }
  }

  Future<DesktopUpdateInstallDisposition> _launchInstaller(
    File file,
    DesktopUpdateAssetKind kind,
  ) async {
    switch (kind) {
      case DesktopUpdateAssetKind.macosArchive:
        throw UnsupportedError('macOS updates are installed through Sparkle.');
      case DesktopUpdateAssetKind.windowsInstaller:
        await Process.start(
          file.path,
          const [],
          mode: ProcessStartMode.detached,
        );
        return DesktopUpdateInstallDisposition.installerLaunched;
      case DesktopUpdateAssetKind.deb:
        final result = await _processRunner('pkexec', [
          'dpkg',
          '-i',
          file.path,
        ]);
        _requireSuccessfulInstallation(result);
        return DesktopUpdateInstallDisposition.restartRequired;
      case DesktopUpdateAssetKind.rpm:
        final result = await _processRunner('pkexec', ['rpm', '-U', file.path]);
        _requireSuccessfulInstallation(result);
        return DesktopUpdateInstallDisposition.restartRequired;
      case DesktopUpdateAssetKind.appImage:
        final appImage = await _prepareAppImage(file);
        final currentAppImage = Platform.environment['APPIMAGE'];
        if (currentAppImage != null && currentAppImage.isNotEmpty) {
          final helperDirectory = await Directory.systemTemp.createTemp(
            'nauterm-appimage-update-',
          );
          final extractedDirectory = appImage.path == file.path
              ? ''
              : appImage.parent.path;
          final helper = File('${helperDirectory.path}/install.sh');
          await helper.writeAsString('''#!/bin/sh
pid="\$1"
download="\$2"
current="\$3"
helper="\$4"
helper_dir="\$5"
extracted_dir="\$6"
while kill -0 "\$pid" 2>/dev/null; do sleep 1; done
chmod +x "\$download"
if mv -f "\$download" "\$current"; then
  "\$current" >/dev/null 2>&1 &
else
  "\$download" >/dev/null 2>&1 &
fi
[ -n "\$extracted_dir" ] && rmdir "\$extracted_dir" 2>/dev/null || true
rm -f "\$helper"
rmdir "\$helper_dir"
''');
          await Process.start('sh', [
            helper.path,
            pid.toString(),
            appImage.path,
            currentAppImage,
            helper.path,
            helperDirectory.path,
            extractedDirectory,
          ], mode: ProcessStartMode.detached);
          return DesktopUpdateInstallDisposition.relaunchOnQuit;
        }
        await Process.start(
          appImage.path,
          const [],
          mode: ProcessStartMode.detached,
        );
        return DesktopUpdateInstallDisposition.relaunchOnQuit;
    }
  }

  static Future<ProcessResult> _runProcess(
    String executable,
    List<String> arguments,
  ) {
    return Process.run(executable, arguments);
  }

  static void _requireSuccessfulInstallation(ProcessResult result) {
    if (result.exitCode == 0) return;
    throw DesktopUpdateException(
      'The package installer failed (exit code ${result.exitCode}).',
    );
  }

  static Future<File> _prepareAppImage(File downloadedFile) async {
    if (!downloadedFile.path.toLowerCase().endsWith('.appimage.tar.gz')) {
      await _makeExecutable(downloadedFile);
      return downloadedFile;
    }

    final listing = await Process.run('tar', ['-tzf', downloadedFile.path]);
    if (listing.exitCode != 0) {
      throw const DesktopUpdateException(
        'Unable to inspect the AppImage archive.',
      );
    }
    final entries = (listing.stdout as String)
        .split('\n')
        .map((entry) => entry.trim())
        .where((entry) => entry.isNotEmpty)
        .toList();
    if (entries.length != 1 ||
        entries.single.contains('/') ||
        entries.single.contains(r'\') ||
        !entries.single.toLowerCase().endsWith('.appimage')) {
      throw const DesktopUpdateException(
        'The AppImage archive has an unexpected layout.',
      );
    }

    final directory = await Directory.systemTemp.createTemp(
      'nauterm-appimage-',
    );
    final extraction = await Process.run('tar', [
      '-xzf',
      downloadedFile.path,
      '-C',
      directory.path,
      entries.single,
    ]);
    if (extraction.exitCode != 0) {
      await directory.delete(recursive: true);
      throw const DesktopUpdateException(
        'Unable to extract the AppImage archive.',
      );
    }
    final appImage = File('${directory.path}/${entries.single}');
    if (!await appImage.exists()) {
      await directory.delete(recursive: true);
      throw const DesktopUpdateException(
        'The AppImage archive did not contain the expected application.',
      );
    }
    await _makeExecutable(appImage);
    return appImage;
  }

  static Future<void> _makeExecutable(File file) async {
    final result = await Process.run('chmod', ['0755', file.path]);
    if (result.exitCode != 0) {
      throw const DesktopUpdateException(
        'Unable to make the AppImage executable.',
      );
    }
  }

  Future<Map<String, String>> _loadChecksums(Uri uri) async {
    final response = await _client
        .get(uri, headers: const {'User-Agent': 'Nauterm-Desktop-Updater'})
        .timeout(_requestTimeout);
    if (response.statusCode != 200) {
      throw DesktopUpdateException(
        'Unable to verify the update (HTTP ${response.statusCode}).',
      );
    }
    return parseSha256Sums(response.body);
  }

  static Future<Directory> _downloadDirectory() async {
    final home =
        Platform.environment[Platform.isWindows ? 'USERPROFILE' : 'HOME'];
    if (home != null && home.isNotEmpty) {
      final downloads = Directory('$home${Platform.pathSeparator}Downloads');
      if (await downloads.exists()) return downloads;
    }
    final fallback = Directory(
      '${Directory.systemTemp.path}${Platform.pathSeparator}nauterm-updates',
    );
    await fallback.create(recursive: true);
    return fallback;
  }

  void close() => _client.close();
}

DesktopUpdateRelease parseGitHubRelease(
  Map<String, dynamic> json,
  DesktopUpdateTarget target,
) {
  final tag = (json['tag_name'] as String? ?? '').trim();
  final releaseUrl = Uri.tryParse(json['html_url'] as String? ?? '');
  final rawAssets = json['assets'];
  if (tag.isEmpty || releaseUrl == null || rawAssets is! List) {
    throw const DesktopUpdateException('The release response is incomplete.');
  }

  final assets = rawAssets
      .whereType<Map>()
      .map((raw) {
        final map = Map<String, dynamic>.from(raw);
        return (
          name: map['name'] as String? ?? '',
          uri: Uri.tryParse(map['browser_download_url'] as String? ?? ''),
          size: map['size'] as int? ?? 0,
        );
      })
      .where((asset) => asset.name.isNotEmpty && asset.uri != null)
      .toList();

  final checksum = assets
      .where((asset) => asset.name == 'SHA256SUMS.txt')
      .firstOrNull;
  if (checksum == null) {
    throw const DesktopUpdateException(
      'This release has no SHA-256 checksum manifest.',
    );
  }

  final match = selectUpdateAsset(assets, target);
  if (match == null) {
    throw const DesktopUpdateException(
      'No update package is available for this platform and architecture.',
    );
  }
  return DesktopUpdateRelease(
    version: tag.replaceFirst(RegExp(r'^[vV]'), ''),
    title: (json['name'] as String?)?.trim().isNotEmpty == true
        ? (json['name'] as String).trim()
        : tag,
    notes: json['body'] as String? ?? '',
    releaseUri: releaseUrl,
    asset: DesktopUpdateAsset(
      name: match.$1.name,
      downloadUri: match.$1.uri!,
      size: match.$1.size,
      kind: match.$2,
    ),
    checksumsUri: checksum.uri!,
  );
}

(({String name, Uri? uri, int size}), DesktopUpdateAssetKind)?
selectUpdateAsset(
  List<({String name, Uri? uri, int size})> assets,
  DesktopUpdateTarget target,
) {
  final platform = target.platform.name;
  final arch = target.architecture.name;
  bool baseMatch(({String name, Uri? uri, int size}) asset) {
    final name = asset.name.toLowerCase();
    if (target.platform == DesktopUpdatePlatform.macos) {
      final archNames = target.architecture == DesktopUpdateArchitecture.x86_64
          ? const ['x86_64']
          : const ['arm64', 'aarch64'];
      return name.contains('macos') && archNames.any(name.contains);
    }
    if (target.platform == DesktopUpdatePlatform.windows) {
      return name.contains(platform) && name.contains(arch);
    }
    final archNames = target.architecture == DesktopUpdateArchitecture.x86_64
        ? const ['x86_64', 'amd64']
        : const ['arm64', 'aarch64'];
    return archNames.any(name.contains) &&
        (name.contains('linux') ||
            name.endsWith('.deb') ||
            name.endsWith('.rpm'));
  }

  final candidates = assets.where(baseMatch);
  final extensions = switch (target.platform) {
    DesktopUpdatePlatform.macos => const [
      ('.app.zip', DesktopUpdateAssetKind.macosArchive),
    ],
    DesktopUpdatePlatform.windows => const [
      ('-setup.exe', DesktopUpdateAssetKind.windowsInstaller),
    ],
    DesktopUpdatePlatform.linux => [
      switch (target.linuxPackagePreference) {
        DesktopUpdateAssetKind.deb => ('.deb', DesktopUpdateAssetKind.deb),
        DesktopUpdateAssetKind.rpm => ('.rpm', DesktopUpdateAssetKind.rpm),
        _ => ('.appimage.tar.gz', DesktopUpdateAssetKind.appImage),
      },
      ('.appimage.tar.gz', DesktopUpdateAssetKind.appImage),
      ('.appimage', DesktopUpdateAssetKind.appImage),
      ('.deb', DesktopUpdateAssetKind.deb),
      ('.rpm', DesktopUpdateAssetKind.rpm),
    ],
  };
  for (final extension in extensions) {
    for (final candidate in candidates) {
      if (candidate.name.toLowerCase().endsWith(extension.$1)) {
        return (candidate, extension.$2);
      }
    }
  }
  return null;
}

Map<String, String> parseSha256Sums(String contents) {
  final result = <String, String>{};
  for (final line in const LineSplitter().convert(contents)) {
    final match = RegExp(r'^([a-fA-F0-9]{64})\s+\*?(.+)$')
        .firstMatch(line.trim());
    if (match != null) result[match.group(2)!] = match.group(1)!;
  }
  return result;
}

int compareVersions(String left, String right) {
  final a = _versionParts(left);
  final b = _versionParts(right);
  final length = a.length > b.length ? a.length : b.length;
  for (var index = 0; index < length; index++) {
    final av = index < a.length ? a[index] : 0;
    final bv = index < b.length ? b[index] : 0;
    if (av != bv) return av.compareTo(bv);
  }
  return 0;
}

List<int> _versionParts(String value) {
  final normalized = value
      .trim()
      .replaceFirst(RegExp(r'^[vV]'), '')
      .split('+')
      .first;
  final core = normalized.split('-').first;
  final parts = core.split('.').map(int.tryParse).toList();
  if (parts.isEmpty || parts.any((part) => part == null)) {
    throw DesktopUpdateException('Invalid application version: $value');
  }
  return parts.cast<int>();
}

class _DigestSink implements Sink<Digest> {
  Digest? value;

  @override
  void add(Digest data) => value = data;

  @override
  void close() {}
}
