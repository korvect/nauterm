import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ai/ai_config.dart';
import 'package:nauterm/data/ai_provider_store.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/nauterm_paths.dart';

void main() {
  test('AI provider reads max tokens from extensible config', () {
    final provider = AiProviderEntry.fromJson({
      'name': 'Example',
      'protocol': 'openai',
      'base_url': 'https://api.example/v1',
      'model': 'example-model',
      'api_key': 'secret',
      'config': {'max_tokens': 8192, 'temperature': 0.25},
      'active': true,
    });

    expect(provider.maxTokens, 8192);
    expect(provider.temperature, 0.25);
    expect(provider.toJson()['config'], {
      'max_tokens': 8192,
      'temperature': 0.25,
    });
  });

  test('AI provider store persists standardized generation settings', () {
    final directory = Directory.systemTemp.createTempSync(
      'nauterm_ai_generation_config_test_',
    );
    addTearDown(() => directory.deleteSync(recursive: true));
    final store = AiProviderStore(
      NautermPaths(configDirectory: directory, dataDirectory: directory),
    );

    final saved = store.save(
      const AiAssistantConfig(
        model: 'model',
        apiKey: 'key',
        maxTokens: 2048,
        temperature: 0.5,
      ),
      existing: const AiProviderEntry(
        name: 'Example',
        protocol: 'openai',
        baseUrl: 'https://api.openai.com/v1',
        model: 'model',
        apiKey: 'key',
        config: {'future_setting': true},
      ),
    );

    expect(saved.config, {
      'future_setting': true,
      'max_tokens': 2048,
      'temperature': 0.5,
    });
    final loaded = store.load(const AiAssistantConfig()).config;
    expect(loaded.maxTokens, 2048);
    expect(loaded.temperature, 0.5);
  });

  test('database can reopen immediately after the previous handle closes', () {
    final directory = Directory.systemTemp.createTempSync(
      'nauterm_data_reopen_test_',
    );
    addTearDown(() => directory.deleteSync(recursive: true));
    final path = '${directory.path}${Platform.pathSeparator}nauterm.sqlite';

    final first = NautermDataStore.openPath(path);
    first.saveGroup(const HostGroup(name: 'Production'));
    first.dispose();

    final reopened = NautermDataStore.openPath(path);
    addTearDown(reopened.dispose);
    expect(reopened.listGroups().single.name, 'Production');
  });

  test('AI providers are physically deleted through FFI', () {
    final directory = Directory.systemTemp.createTempSync(
      'nauterm_ai_provider_delete_test_',
    );
    final store = NautermDataStore.openPath(
      '${directory.path}${Platform.pathSeparator}nauterm.sqlite',
    );
    addTearDown(() {
      store.dispose();
      directory.deleteSync(recursive: true);
    });

    final provider = store.saveAiProvider(
      const AiProviderEntry(
        name: 'Example',
        protocol: 'openai',
        baseUrl: 'https://api.example/v1',
        model: 'example-model',
        apiKey: 'secret',
      ),
    );

    expect(store.deleteAiProvider(provider.id!), 1);
    expect(store.listAiProviders(), isEmpty);
    expect(store.deleteAiProvider(provider.id!), 0);
  });

  test('Rust data store manages host data through FFI', () {
    final directory = Directory.systemTemp.createTempSync('nauterm_data_test_');
    late final NautermDataStore store;
    addTearDown(() {
      store.dispose();
      directory.deleteSync(recursive: true);
    });

    store = NautermDataStore.openPath(
      '${directory.path}${Platform.pathSeparator}nauterm.sqlite',
    );

    expect(store.schemaVersion, 3);
    final deviceId = store.deviceId;
    expect(
      deviceId,
      matches(
        RegExp(
          r'^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',
        ),
      ),
    );

    final groupId = store.saveGroup(const HostGroup(name: 'Production'));
    final keyId = store.saveKey(
      const KeyEntry(
        name: 'Main key',
        privateKey: 'private',
        publicKey: 'public',
      ),
    );
    final identityId = store.saveIdentity(
      IdentityEntry(
        name: 'admin',
        username: 'admin',
        password: 'secret',
        keyId: keyId,
      ),
    );
    final proxyId = store.saveProxy(
      ProxyEntry(
        name: 'office proxy',
        type: 'socks5',
        host: 'proxy.internal',
        port: 1080,
        identityId: identityId,
        username: 'proxy-user',
        password: 'proxy-secret',
      ),
    );
    final hostId = store.saveHost(
      HostEntry(
        name: 'cassandra',
        groupId: groupId,
        identityId: identityId,
        proxyId: proxyId,
        host: '10.0.0.12',
        port: 22,
        username: 'admin',
        type: NautermHostType.remote,
        keyId: keyId,
        moshEnabled: true,
        moshServerCommand: 'custom-mosh new -s -l LANG=en_US.UTF-8',
        telnetEnabled: true,
        telnetIdentityId: identityId,
        telnetUsername: 'telnet-user',
        telnetPassword: 'telnet-secret',
        telnetPort: 23,
        telnetThemeId: 'ayu-dark',
        environmentVariables: const [
          HostEnvironmentVariable(variable: 'LANG', value: 'en_US.UTF-8'),
          HostEnvironmentVariable(variable: 'NAUTERM_MODE', value: 'ops'),
        ],
        encoding: 'UTF-8',
        os: 'linux',
        distro: 'alpine',
      ),
    );
    final forwardId = store.savePortForward(
      PortForwardEntry(
        name: 'postgres',
        type: 'local',
        bindAddress: '127.0.0.1',
        bindPort: 5432,
        destinationHost: '127.0.0.1',
        destinationPort: 5432,
        connectionId: hostId,
      ),
    );
    expect(store.listGroups().single.name, 'Production');
    expect(store.listGroups().single.uuid, isNotEmpty);
    expect(store.listGroups().single.createdDeviceId, deviceId);
    expect(store.listGroups().single.updatedDeviceId, deviceId);
    expect(store.listKeys().single.publicKey, 'public');
    expect(store.listKeys().single.version, 1);
    expect(store.listIdentities().single.keyId, keyId);
    expect(store.listIdentities().single.keyUuid, store.listKeys().single.uuid);
    final certificateId = store.saveKey(
      const KeyEntry(
        name: 'Main certificate',
        privateKey: 'certificate-private',
        publicKey: 'certificate-public',
        certificate: 'ssh-ed25519-cert-v01@openssh.com certificate',
      ),
    );
    final keySummaries = store.listKeys();
    expect(
      keySummaries.firstWhere((entry) => entry.id == keyId).certificate,
      isNull,
    );
    expect(
      keySummaries.firstWhere((entry) => entry.id == certificateId).certificate,
      'ssh-ed25519-cert-v01@openssh.com',
    );
    expect(
      store.getKey(certificateId)?.certificate,
      'ssh-ed25519-cert-v01@openssh.com certificate',
    );
    expect(store.getIdentity(identityId)?.password, 'secret');
    expect(store.getHost(hostId)?.host, '10.0.0.12');
    expect(store.getHost(hostId)?.groupUuid, store.listGroups().single.uuid);
    expect(store.getHost(hostId)?.identityId, identityId);
    expect(
      store.getHost(hostId)?.identityUuid,
      store.listIdentities().single.uuid,
    );
    expect(store.getHost(hostId)?.proxyId, proxyId);
    expect(store.getHost(hostId)?.proxyUuid, store.getProxy(proxyId)?.uuid);
    expect(store.getHost(hostId)?.moshEnabled, isTrue);
    expect(
      store.getHost(hostId)?.moshServerCommand,
      'custom-mosh new -s -l LANG=en_US.UTF-8',
    );
    expect(store.getHost(hostId)?.telnetEnabled, isTrue);
    expect(store.getHost(hostId)?.telnetIdentityId, identityId);
    expect(store.getHost(hostId)?.telnetPort, 23);
    expect(store.getHost(hostId)?.os, 'linux');
    expect(store.getHost(hostId)?.distro, 'alpine');
    expect(store.getHost(hostId)?.telnetThemeId, 'ayu-dark');
    expect(store.getHost(hostId)?.environmentVariables.length, 2);
    expect(store.getHost(hostId)?.encoding, 'UTF-8');
    expect(store.listHosts(groupId: groupId).single.username, 'admin');
    expect(store.getPortForward(forwardId)?.bindPort, 5432);
    expect(
      store.getPortForward(forwardId)?.hostUuid,
      store.getHost(hostId)?.uuid,
    );
    expect(store.getProxy(proxyId)?.type, 'socks5');
    expect(store.getProxy(proxyId)?.identityId, identityId);
    expect(
      store.getProxy(proxyId)?.identityUuid,
      store.getIdentity(identityId)?.uuid,
    );
    expect(store.getProxy(proxyId)?.password, 'proxy-secret');
    expect(store.listProxies().single.uuid, isNotEmpty);
    expect(store.listProxies().single.password, isNull);

    final favoriteId = store.saveSftpFavoritePath(
      SftpFavoritePathEntry(
        scope: SftpFavoriteScope.remote,
        hostId: hostId,
        path: '/tmp',
      ),
    );
    final sameFavoriteId = store.saveSftpFavoritePath(
      SftpFavoritePathEntry(
        scope: SftpFavoriteScope.remote,
        hostId: hostId,
        path: '/tmp',
      ),
    );
    expect(sameFavoriteId, favoriteId);
    expect(
      store
          .listSftpFavoritePaths(
            scope: SftpFavoriteScope.remote,
            hostId: hostId,
          )
          .single
          .path,
      '/tmp',
    );
    expect(
      store
          .listSftpFavoritePaths(
            scope: SftpFavoriteScope.remote,
            hostId: hostId,
          )
          .single
          .uuid,
      isNotEmpty,
    );
    expect(
      store.deleteSftpFavoritePathByTarget(
        scope: SftpFavoriteScope.remote,
        hostId: hostId,
        path: '/tmp',
      ),
      1,
    );

    final taskHistoryNow = DateTime.now().toUtc();
    final recentTask = SftpTaskHistoryEntry(
      hostUuid: store.getHost(hostId)?.uuid,
      type: 'edit',
      host: 'example.com',
      username: 'admin',
      status: 'completed',
      displayName: 'Edit config.yaml',
      sourcePath: '/tmp/config.yaml',
      targetPath: '/etc/config.yaml',
      createdAt: taskHistoryNow.subtract(const Duration(days: 1, minutes: 1)),
      finishedAt: taskHistoryNow.subtract(const Duration(days: 1)),
      bytes: 128,
      totalBytes: 128,
      itemKind: 'file',
    );
    store.saveSftpTaskHistory(recentTask);
    store.saveSftpTaskHistory(
      SftpTaskHistoryEntry(
        type: 'download',
        host: 'example.com',
        username: 'admin',
        status: 'completed',
        displayName: 'old.txt',
        sourcePath: '/remote/old.txt',
        targetPath: '/tmp/old.txt',
        createdAt: taskHistoryNow.subtract(
          const Duration(days: 40, minutes: 1),
        ),
        finishedAt: taskHistoryNow.subtract(const Duration(days: 40)),
        bytes: 12,
        totalBytes: 12,
        itemKind: 'file',
      ),
    );
    final taskHistory = store.listSftpTaskHistory(
      cutoff: taskHistoryNow.subtract(const Duration(days: 30)),
    );
    expect(taskHistory.single.id, isNotNull);
    expect(taskHistory.single.uuid, isNotEmpty);
    expect(taskHistory.single.type, 'edit');
    expect(store.clearSftpTaskHistory(), 1);
    expect(
      store.listSftpTaskHistory(
        cutoff: taskHistoryNow.subtract(const Duration(days: 30)),
      ),
      isEmpty,
    );

    expect(store.listSnippetPackages(), isEmpty);

    final packageId = store.saveSnippetPackage(
      const SnippetPackageEntry(name: 'Ops'),
    );
    final snippetId = store.saveSnippet(
      SnippetEntry(
        packageId: packageId,
        scope: SnippetScope.targeted,
        description: 'Restart service',
        script: 'systemctl restart app',
        targetGroupIds: [groupId],
        targetHostIds: [hostId],
      ),
    );
    final snippet = store.getSnippet(snippetId);
    expect(snippet?.packageId, packageId);
    expect(snippet?.packageUuid, store.listSnippetPackages().single.uuid);
    expect(snippet?.scope, SnippetScope.targeted);
    expect(snippet?.targetGroupIds, [groupId]);
    expect(snippet?.targetHostIds, [hostId]);
    expect(store.listSnippets(packageId: packageId).single.id, snippetId);

    store.saveGroup(
      HostGroup(
        id: groupId,
        name: 'Production',
        startupSnippetId: snippetId,
        sshEnabled: true,
        moshEnabled: true,
        moshServerCommand: 'group-mosh new',
        port: 2202,
        username: 'group-admin',
        password: 'group-secret',
        encoding: 'GBK',
        telnetEnabled: true,
        telnetPort: 2323,
        telnetEncoding: 'BIG5',
        environmentVariables: const [
          HostEnvironmentVariable(variable: 'LANG', value: 'zh_CN.GBK'),
        ],
      ),
    );
    final configuredGroup = store.getGroup(groupId);
    expect(configuredGroup?.startupSnippetId, snippetId);
    expect(configuredGroup?.startupSnippetUuid, snippet?.uuid);
    expect(configuredGroup?.sshEnabled, isTrue);
    expect(configuredGroup?.password, 'group-secret');
    expect(configuredGroup?.encoding, 'GBK');
    expect(configuredGroup?.telnetEncoding, 'BIG5');

    final startupHostId = store.saveHost(
      HostEntry(
        name: 'snippet-host',
        groupId: groupId,
        host: '10.0.0.13',
        type: NautermHostType.remote,
        sshEnabled: true,
        startupSnippetId: snippetId,
      ),
    );
    expect(store.getHost(startupHostId)?.startupSnippetId, snippetId);
    expect(store.getHost(startupHostId)?.startupSnippetUuid, snippet?.uuid);

    final looseSnippetId = store.saveSnippet(
      const SnippetEntry(description: 'Loose command', script: 'pwd'),
    );
    expect(store.getSnippet(looseSnippetId)?.packageId, isNull);

    final logId = store.saveTerminalLog(
      TerminalLogEntry(
        id: 'session-1',
        title: 'cassandra',
        themeId: 'atom-one-dark',
        hostId: hostId,
        host: '10.0.0.12',
        port: 22,
        username: 'admin',
        workDir: '/srv/app',
        cwd: '/srv/app',
        captureFile: 'session-1.bin',
        captureBytes: 5,
        captureSha256:
            '2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824',
        columns: 100,
        rows: 30,
        startedAt: DateTime.utc(2026, 5, 20, 10),
      ),
      events: [
        TerminalLogEvent(
          timestamp: DateTime.utc(2026, 5, 20, 10, 0, 1),
          type: 'connection',
          message: 'Connected.',
          connectionKind: 'connected',
        ),
      ],
    );
    final logs = store.listTerminalLogs();
    expect(logId, isNot('session-1'));
    expect(
      logId,
      matches(
        RegExp(
          r'^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',
        ),
      ),
    );
    expect(logs.single.id, logId);
    expect(logs.single.hostId, hostId);
    expect(logs.single.hostUuid, store.getHost(hostId)?.uuid);
    expect(logs.single.captureBytes, 5);
    expect(store.listTerminalLogEvents(logId).single.type, 'connection');

    expect(store.deleteHost(hostId), 1);
    expect(store.listPortForwards(connectionId: hostId), isEmpty);
    expect(store.getSnippet(snippetId)?.targetHostIds, isEmpty);
    expect(store.deleteSnippet(snippetId), 1);
    expect(store.getGroup(groupId)?.startupSnippetId, isNull);
    expect(store.getHost(startupHostId)?.startupSnippetId, isNull);
  });
}
