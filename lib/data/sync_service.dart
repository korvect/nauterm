import 'dart:async';
import 'dart:isolate';

import 'package:flutter/foundation.dart';

import '../app/nauterm_log.dart';
import 'nauterm_config_store.dart';
import 'nauterm_data_store.dart';
import 'nauterm_paths.dart';

class SyncService {
  SyncService(this.paths, {this._syncNow, this._onSyncCompleted});

  final NautermPaths paths;
  final Future<void> Function(String strategy)? _syncNow;
  final VoidCallback? _onSyncCompleted;
  Timer? _timer;
  DateTime? _lastAttempt;
  Future<void>? _running;
  bool _closed = false;
  bool _refreshPending = false;

  void start() {
    if (_closed || _timer != null || _running != null) return;
    _lastAttempt = DateTime.now();
    NautermLog.info('sync', 'Automatic sync scheduler started.');
    _schedule(Duration.zero);
  }

  void _schedule(Duration delay) {
    if (_closed) return;
    _timer?.cancel();
    _timer = Timer(delay, () {
      _timer = null;
      _tick();
    });
  }

  void _tick() {
    if (_closed) return;
    if (_running != null) {
      _refreshPending = true;
      return;
    }
    final running = _runOnce();
    _running = running;
    unawaited(
      running.whenComplete(() {
        _running = null;
        if (_closed) return;
        if (_refreshPending) {
          _refreshPending = false;
          _schedule(Duration.zero);
        }
      }),
    );
  }

  Future<void> _runOnce() async {
    final config = await NautermConfigStore(paths).loadSyncConfig();
    if (!config.automatic) return;

    final interval = Duration(milliseconds: config.interval);
    final now = DateTime.now();
    final lastAttempt = _lastAttempt;
    if (lastAttempt != null) {
      final remaining = interval - now.difference(lastAttempt);
      if (remaining > Duration.zero) {
        _schedule(remaining);
        return;
      }
    }

    _lastAttempt = now;
    final operation = NautermLog.begin(
      'sync',
      'Automatic sync',
      fields: {'merge_strategy': config.mergeStrategy},
    );
    try {
      final syncNow = _syncNow;
      if (syncNow != null) {
        await syncNow(config.mergeStrategy);
        operation.succeed(fields: const {'provider': 'custom'});
        _onSyncCompleted?.call();
      } else {
        final provider = await _syncInBackground(
          config.mergeStrategy,
          config.backupCount,
        );
        operation.succeed(
          fields: {'provider': provider ?? 'none', 'synced': provider != null},
        );
        if (provider != null) _onSyncCompleted?.call();
      }
    } on Object catch (error, stackTrace) {
      operation.fail(error, stackTrace: stackTrace);
    } finally {
      if (!_closed) _schedule(interval);
    }
  }

  Future<String?> _syncInBackground(String strategy, int backupCount) {
    final databasePath = paths.databasePath;
    return Isolate.run(() {
      final store = NautermDataStore.openPath(databasePath);
      try {
        final preferences = store.syncPreferences();
        final active = preferences['active_provider_id'] as String?;
        if (active == null || active.isEmpty || !store.hasLocalSyncKey()) {
          return null;
        }
        String provider;
        if (active == 'github_repository') {
          provider = 'github_repository';
          store.githubSync(strategy: strategy, backupCount: backupCount);
        } else if (active == 'github_gist') {
          provider = 'github_gist';
          store.githubGistSync(strategy: strategy, backupCount: backupCount);
        } else if (active == 's3') {
          provider = 's3';
          store.s3Sync(strategy: strategy, backupCount: backupCount);
        } else if (active.startsWith('cloud:')) {
          provider = 'cloud';
          store.cloudSync(
            providerId: active.substring(6),
            strategy: strategy,
            backupCount: backupCount,
          );
        } else {
          return null;
        }
        return provider;
      } finally {
        store.dispose();
      }
    });
  }

  void preferencesChanged() {
    if (_closed) return;
    _lastAttempt = DateTime.now();
    _timer?.cancel();
    _timer = null;
    if (_running != null) {
      _refreshPending = true;
    } else {
      _schedule(Duration.zero);
    }
  }

  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    _timer?.cancel();
    _timer = null;
    _refreshPending = false;
    await _running;
  }
}
