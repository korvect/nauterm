part of 'nauterm_workspace.dart';

List<_GroupItem> _mapGroups(List<HostGroup> groups, List<HostEntry> hosts) {
  return groups
      .map((group) {
        final hostCount = hosts
            .where((host) => host.groupId == group.id)
            .length;
        return _GroupItem(
          id: group.id ?? 0,
          parentId: group.parentId,
          name: group.name,
          subtitle: '$hostCount ${hostCount == 1 ? 'Host' : 'Hosts'}',
          icon: Icons.dashboard_customize_rounded,
          color: const Color(0xff075e92),
          createdAt: group.createdAt,
          updatedAt: group.updatedAt,
        );
      })
      .toList(growable: false);
}

List<_HostItem> _mapHosts(
  Iterable<HostEntry> hosts,
  Iterable<HostGroup> _,
  List<IdentityEntry> identities,
  List<TagEntry> tags,
) {
  final tagsByUuid = _tagNamesByUuid(tags);
  return [for (final host in hosts) _mapHost(host, identities, tagsByUuid)];
}

_HostItem _mapHost(
  HostEntry host,
  List<IdentityEntry> identities,
  Map<String, String> tagsByUuid,
) {
  final type = host.type.storageValue;
  final identity = host.identityId == null
      ? null
      : identities
            .where((identity) => identity.id == host.identityId)
            .firstOrNull;
  final username = _firstNonEmpty([identity?.username, host.username]);
  final telnetIdentity = host.telnetIdentityId == null
      ? null
      : identities
            .where((identity) => identity.id == host.telnetIdentityId)
            .firstOrNull;
  final telnetUsername = host.telnetEnabled
      ? _firstNonEmpty([telnetIdentity?.username, host.telnetUsername])
      : null;
  final tagNames = [for (final uuid in host.tagUuids) ?tagsByUuid[uuid]];
  final protocols = [
    if (host.type == NautermHostType.local) type,
    if (host.sshEnabled && host.type == NautermHostType.remote) 'ssh',
    if (host.moshEnabled && host.type == NautermHostType.remote) 'mosh',
    if (host.telnetEnabled && host.type == NautermHostType.remote) 'telnet',
  ];
  final details = [
    ...protocols,
    if (username != null && username.isNotEmpty) username,
    if (telnetUsername != null && telnetUsername != username) telnetUsername,
    ...tagNames,
  ];
  return _HostItem(
    id: host.id ?? 0,
    uuid: host.uuid,
    name: host.name,
    subtitle: details.join(', '),
    icon: host.type == NautermHostType.local
        ? LucideIcons.squareTerminal
        : Icons.public_rounded,
    color: host.type == NautermHostType.local ? _green : _orange,
    createdAt: host.createdAt,
    updatedAt: host.updatedAt,
    type: type,
    groupId: host.groupId,
    host: host.host,
    port: host.port,
    username: username,
    os: host.os,
    distro: host.distro,
    sshEnabled: host.sshEnabled,
    moshEnabled: host.moshEnabled,
    moshServerCommand: host.moshServerCommand,
    telnetEnabled: host.telnetEnabled,
    tagUuids: host.tagUuids,
  );
}

_KeyItem _mapKey(KeyEntry key) {
  return _KeyItem(
    id: key.id ?? 0,
    name: key.name,
    subtitle: _keyTypeLabel(
      publicKey: key.publicKey,
      privateKey: key.privateKey,
      certificate: key.certificate,
    ),
    icon: Icons.key_rounded,
    color: const Color(0xff075e92),
    privateKey: key.privateKey,
    publicKey: key.publicKey,
    certificate: key.certificate,
    createdAt: key.createdAt,
    updatedAt: key.updatedAt,
  );
}

String _keyTypeLabel({
  String? publicKey,
  String? privateKey,
  String? certificate,
}) {
  final certificateType = _sshCertificateBaseWireType(certificate);
  if (certificateType != null) {
    return '${_keyTypeLabelFromWireName(certificateType)} SSH Certificate';
  }

  final text = _emptyToNull(publicKey);
  if (certificate != null) {
    if (text == null) {
      return 'SSH Certificate';
    }
    final type = text.split(RegExp(r'\s+')).first.trim();
    return '${_keyTypeLabelFromWireName(type)} SSH Certificate';
  }

  if (text != null) {
    final type = text.split(RegExp(r'\s+')).first.trim();
    return _keyTypeLabelFromWireName(type);
  }

  final privateText = _emptyToNull(privateKey);
  if (privateText == null) {
    return 'Unknown key type';
  }

  if (privateText.contains('BEGIN OPENSSH PRIVATE KEY')) {
    return 'OpenSSH key';
  }
  if (privateText.contains('BEGIN RSA PRIVATE KEY')) {
    return 'RSA';
  }
  if (privateText.contains('BEGIN EC PRIVATE KEY')) {
    return 'ECDSA';
  }
  if (privateText.contains('BEGIN DSA PRIVATE KEY')) {
    return 'DSA';
  }
  if (privateText.contains('BEGIN PRIVATE KEY')) {
    return 'Private key';
  }

  return 'Unknown key type';
}

String? _sshCertificateBaseWireType(String? value) {
  const suffix = '-cert-v01@openssh.com';
  final text = _emptyToNull(value);
  if (text == null) {
    return null;
  }
  for (final line in text.split(RegExp(r'\r?\n'))) {
    final trimmed = line.trim();
    if (trimmed.isEmpty) {
      continue;
    }
    final type = trimmed.split(RegExp(r'\s+')).first;
    if (type.endsWith(suffix) && type.length > suffix.length) {
      return type.substring(0, type.length - suffix.length);
    }
  }
  return null;
}

String? _sshCertificateSummaryMarker(String? value) {
  if (value == null) {
    return null;
  }
  const suffix = '-cert-v01@openssh.com';
  final baseType = _sshCertificateBaseWireType(value);
  return baseType == null ? '' : '$baseType$suffix';
}

String _keyTypeLabelFromWireName(String type) {
  return switch (type) {
    'ssh-ed25519' => 'ED25519',
    'ssh-rsa' || 'rsa-sha2-256' || 'rsa-sha2-512' => 'RSA',
    'ecdsa-sha2-nistp256' => 'ECDSA 256',
    'ecdsa-sha2-nistp384' => 'ECDSA 384',
    'ecdsa-sha2-nistp521' => 'ECDSA 521',
    'sk-ssh-ed25519' || 'sk-ssh-ed25519@openssh.com' => 'FIDO2 ED25519',
    'sk-ecdsa-sha2-nistp256' ||
    'sk-ecdsa-sha2-nistp256@openssh.com' => 'FIDO2 ECDSA 256',
    'ssh-dss' => 'DSA',
    _ when type.contains('mldsa44') || type.contains('ml-dsa44') => 'ML-DSA 44',
    _ when type.contains('mldsa65') || type.contains('ml-dsa65') => 'ML-DSA 65',
    _ when type.contains('mldsa87') || type.contains('ml-dsa87') => 'ML-DSA 87',
    _ => type,
  };
}

_IdentityItem _mapIdentity(IdentityEntry identity) {
  final username = identity.username;
  return _IdentityItem(
    id: identity.id ?? 0,
    name: identity.name,
    subtitle: username == null || username.isEmpty ? 'No username' : username,
    icon: Icons.badge_rounded,
    color: const Color(0xff075e92),
    createdAt: identity.createdAt,
    updatedAt: identity.updatedAt,
    username: username,
    keyId: identity.keyId,
  );
}

_PortForwardItem _mapPortForward(
  PortForwardEntry portForward,
  List<HostEntry> hosts,
  Set<int> runningPortForwardIds,
  Map<int, FfiPortForwardStatus> portForwardStatuses,
) {
  final type = portForward.type.trim().isEmpty ? 'local' : portForward.type;
  final id = portForward.id ?? 0;
  final status = portForwardStatuses[id];
  final statusError = status?.error?.trim().isEmpty ?? true
      ? null
      : status?.error;
  final host = hosts
      .where((host) => host.id == portForward.connectionId)
      .firstOrNull;
  final hostAddress = _emptyToNull(host?.host);
  final hostLabel = host == null
      ? 'Host #${portForward.connectionId}'
      : hostAddress == null
      ? host.name
      : '${host.name} ($hostAddress:${host.port ?? 22})';
  return _PortForwardItem(
    id: id,
    name: portForward.name,
    subtitle:
        '$type, ${portForward.bindAddress}:${portForward.bindPort} -> '
        '${portForward.destinationHost}:${portForward.destinationPort}',
    icon: Icons.alt_route_rounded,
    color: const Color(0xff075e92),
    createdAt: portForward.createdAt,
    updatedAt: portForward.updatedAt,
    type: type,
    enabled:
        portForward.id != null &&
        runningPortForwardIds.contains(portForward.id),
    activeConnections: status?.activeConnections ?? 0,
    statusError: statusError,
    bindAddress: portForward.bindAddress,
    bindPort: portForward.bindPort,
    destinationHost: portForward.destinationHost,
    destinationPort: portForward.destinationPort,
    connectionId: portForward.connectionId,
    intermediateHostName: hostLabel,
  );
}

_ProxyItem _mapProxy(ProxyEntry proxy, List<IdentityEntry> identities) {
  final type = _normalizeProxyType(proxy.type);
  final identity = proxy.identityId == null
      ? null
      : identities
            .where((identity) => identity.id == proxy.identityId)
            .firstOrNull;
  final username = _firstNonEmpty([proxy.username, identity?.username]);
  final authLabel = username == null || username.isEmpty
      ? 'No authentication'
      : 'Auth: $username';
  return _ProxyItem(
    id: proxy.id ?? 0,
    name: proxy.name,
    subtitle:
        '${_proxyTypeLabel(type)} ${proxy.host}:${proxy.port}, $authLabel',
    icon: Icons.lan_rounded,
    color: const Color(0xff075e92),
    createdAt: proxy.createdAt,
    updatedAt: proxy.updatedAt,
    type: type,
    host: proxy.host,
    port: proxy.port,
    identityId: proxy.identityId,
    identityName: identity?.name,
    username: username,
  );
}

_SnippetItem _mapSnippet(SnippetEntry snippet) {
  return _SnippetItem(
    id: snippet.id ?? 0,
    packageId: snippet.packageId,
    scope: snippet.scope,
    description: snippet.description,
    script: snippet.script,
    targetGroupIds: snippet.targetGroupIds,
    targetHostIds: snippet.targetHostIds,
    icon: Icons.data_object_rounded,
    color: const Color(0xff075e92),
    createdAt: snippet.createdAt,
    updatedAt: snippet.updatedAt,
  );
}

class _PortForwardSshAuth {
  const _PortForwardSshAuth({
    required this.host,
    required this.port,
    required this.username,
    this.password,
    this.privateKey,
    this.certificate,
    this.passphrase,
    this.proxy,
  });

  final String host;
  final int port;
  final String username;
  final String? password;
  final String? privateKey;
  final String? certificate;
  final String? passphrase;
  final TerminalProxyConfig? proxy;
}

class _CdCompletionQuery {
  const _CdCompletionQuery({
    required this.directoryPath,
    required this.parentArgument,
    required this.namePrefix,
  });

  final String directoryPath;
  final String parentArgument;
  final String namePrefix;
}
