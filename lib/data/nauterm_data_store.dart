import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

import '../app/nauterm_log.dart';

const String defaultMoshServerCommand =
    'mosh-server new -s -l LANG=en_US.UTF-8';

enum NautermHostType {
  local('local'),
  remote('remote');

  const NautermHostType(this.storageValue);

  final String storageValue;

  static NautermHostType fromJson(Object? value) {
    return switch (value) {
      'local' => NautermHostType.local,
      'remote' => NautermHostType.remote,
      _ => throw FormatException('Unknown host type: $value'),
    };
  }
}

enum SnippetScope {
  global('global'),
  targeted('targeted');

  const SnippetScope(this.storageValue);

  final String storageValue;

  static SnippetScope fromJson(Object? value) {
    return switch (value) {
      'targeted' => SnippetScope.targeted,
      _ => SnippetScope.global,
    };
  }
}

enum SftpFavoriteScope {
  remote('remote');

  const SftpFavoriteScope(this.storageValue);

  final String storageValue;

  static SftpFavoriteScope fromJson(Object? value) {
    return SftpFavoriteScope.remote;
  }
}

const String defaultHostEncoding = 'UTF-8';

class HostEnvironmentVariable {
  const HostEnvironmentVariable({required this.variable, required this.value});

  final String variable;
  final String value;

  Map<String, Object?> toJson() => {'variable': variable, 'value': value};

  static HostEnvironmentVariable fromJson(Object? json) {
    final map = _jsonMap(json);
    return HostEnvironmentVariable(
      variable: _string(map['variable']),
      value: _string(map['value']),
    );
  }
}

class HostGroup {
  const HostGroup({
    this.id,
    this.uuid,
    required this.name,
    this.parentId,
    this.parentUuid,
    this.identityId,
    this.identityUuid,
    this.proxyId,
    this.proxyUuid,
    this.port,
    this.username,
    this.password,
    this.themeId,
    this.startupSnippetId,
    this.startupSnippetUuid,
    this.sshEnabled,
    this.moshEnabled,
    this.moshServerCommand,
    this.telnetEnabled,
    this.telnetIdentityId,
    this.telnetIdentityUuid,
    this.telnetUsername,
    this.telnetPassword,
    this.telnetPort,
    this.telnetThemeId,
    this.environmentVariables = const [],
    this.encoding,
    this.telnetEncoding,
    this.keyId,
    this.keyUuid,
    this.createdAt,
    this.updatedAt,
    this.deletedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  });

  final int? id;
  final String? uuid;
  final String name;
  final int? parentId;
  final String? parentUuid;
  final int? identityId;
  final String? identityUuid;
  final int? proxyId;
  final String? proxyUuid;
  final int? port;
  final String? username;
  final String? password;
  final String? themeId;
  final int? startupSnippetId;
  final String? startupSnippetUuid;
  final bool? sshEnabled;
  final bool? moshEnabled;
  final String? moshServerCommand;
  final bool? telnetEnabled;
  final int? telnetIdentityId;
  final String? telnetIdentityUuid;
  final String? telnetUsername;
  final String? telnetPassword;
  final int? telnetPort;
  final String? telnetThemeId;
  final List<HostEnvironmentVariable> environmentVariables;
  final String? encoding;
  final String? telnetEncoding;
  final int? keyId;
  final String? keyUuid;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final DateTime? deletedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'name': name,
    'parent_id': parentId,
    'parent_uuid': parentUuid,
    'identity_id': identityId,
    'identity_uuid': identityUuid,
    'proxy_id': proxyId,
    'proxy_uuid': proxyUuid,
    'port': port,
    'username': username,
    'password': password,
    'theme_id': _normalizeThemeId(themeId),
    'startup_snippet_id': startupSnippetId,
    'startup_snippet_uuid': startupSnippetUuid,
    'ssh_enabled': sshEnabled,
    'mosh_enabled': moshEnabled,
    'mosh_server_command': moshServerCommand,
    'telnet_enabled': telnetEnabled,
    'telnet_identity_id': telnetIdentityId,
    'telnet_identity_uuid': telnetIdentityUuid,
    'telnet_username': telnetUsername,
    'telnet_password': telnetPassword,
    'telnet_port': telnetPort,
    'telnet_theme_id': _normalizeThemeId(telnetThemeId),
    'environment_variables': [
      for (final variable in environmentVariables) variable.toJson(),
    ],
    'encoding': encoding,
    'telnet_encoding': telnetEncoding,
    'key_id': keyId,
    'key_uuid': keyUuid,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'deleted_at': _dateTimeToJson(deletedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  static HostGroup fromJson(Object? json) {
    final map = _jsonMap(json);
    return HostGroup(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      name: _string(map['name']),
      parentId: _intOrNull(map['parent_id']),
      parentUuid: _stringOrNull(map['parent_uuid']),
      identityId: _intOrNull(map['identity_id']),
      identityUuid: _stringOrNull(map['identity_uuid']),
      proxyId: _intOrNull(map['proxy_id']),
      proxyUuid: _stringOrNull(map['proxy_uuid']),
      port: _intOrNull(map['port']),
      username: _stringOrNull(map['username']),
      password: _stringOrNull(map['password']),
      themeId: _normalizeThemeId(_stringOrNull(map['theme_id'])),
      startupSnippetId: _intOrNull(map['startup_snippet_id']),
      startupSnippetUuid: _stringOrNull(map['startup_snippet_uuid']),
      sshEnabled: _boolOrNull(map['ssh_enabled']),
      moshEnabled: _boolOrNull(map['mosh_enabled']),
      moshServerCommand: _stringOrNull(map['mosh_server_command']),
      telnetEnabled: _boolOrNull(map['telnet_enabled']),
      telnetIdentityId: _intOrNull(map['telnet_identity_id']),
      telnetIdentityUuid: _stringOrNull(map['telnet_identity_uuid']),
      telnetUsername: _stringOrNull(map['telnet_username']),
      telnetPassword: _stringOrNull(map['telnet_password']),
      telnetPort: _intOrNull(map['telnet_port']),
      telnetThemeId: _normalizeThemeId(_stringOrNull(map['telnet_theme_id'])),
      environmentVariables: _environmentVariablesOrEmpty(
        map['environment_variables'],
      ),
      encoding: _stringOrNull(map['encoding']),
      telnetEncoding: _stringOrNull(map['telnet_encoding']),
      keyId: _intOrNull(map['key_id']),
      keyUuid: _stringOrNull(map['key_uuid']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      deletedAt: _dateTimeOrNull(map['deleted_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

class KeyEntry {
  const KeyEntry({
    this.id,
    this.uuid,
    required this.name,
    this.privateKey,
    this.publicKey,
    this.certificate,
    this.createdAt,
    this.updatedAt,
    this.deletedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  });

  final int? id;
  final String? uuid;
  final String name;
  final String? privateKey;
  final String? publicKey;
  final String? certificate;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final DateTime? deletedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'name': name,
    'private_key': privateKey,
    'public_key': publicKey,
    'certificate': certificate,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'deleted_at': _dateTimeToJson(deletedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  static KeyEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return KeyEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      name: _string(map['name']),
      privateKey: _stringOrNull(map['private_key']),
      publicKey: _stringOrNull(map['public_key']),
      certificate: _stringOrNull(map['certificate']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      deletedAt: _dateTimeOrNull(map['deleted_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

class IdentityEntry {
  const IdentityEntry({
    this.id,
    this.uuid,
    required this.name,
    this.username,
    this.password,
    this.keyId,
    this.keyUuid,
    this.createdAt,
    this.updatedAt,
    this.deletedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  });

  final int? id;
  final String? uuid;
  final String name;
  final String? username;
  final String? password;
  final int? keyId;
  final String? keyUuid;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final DateTime? deletedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'name': name,
    'username': username,
    'password': password,
    'key_id': keyId,
    'key_uuid': keyUuid,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'deleted_at': _dateTimeToJson(deletedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  static IdentityEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return IdentityEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      name: _string(map['name']),
      username: _stringOrNull(map['username']),
      password: _stringOrNull(map['password']),
      keyId: _intOrNull(map['key_id']),
      keyUuid: _stringOrNull(map['key_uuid']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      deletedAt: _dateTimeOrNull(map['deleted_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

class TagEntry {
  const TagEntry({
    this.id,
    this.uuid,
    required this.name,
    this.createdAt,
    this.updatedAt,
    this.deletedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  });

  final int? id;
  final String? uuid;
  final String name;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final DateTime? deletedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'name': name,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'deleted_at': _dateTimeToJson(deletedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  static TagEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return TagEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      name: _string(map['name']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      deletedAt: _dateTimeOrNull(map['deleted_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

class HostEntry {
  const HostEntry({
    this.id,
    this.uuid,
    required this.name,
    this.groupId,
    this.groupUuid,
    this.identityId,
    this.identityUuid,
    this.proxyId,
    this.proxyUuid,
    this.host,
    this.port,
    this.username,
    this.password,
    this.themeId,
    this.startupSnippetId,
    this.startupSnippetUuid,
    bool? sshEnabled,
    bool? moshEnabled,
    String? moshServerCommand,
    bool? telnetEnabled,
    this.telnetIdentityId,
    this.telnetIdentityUuid,
    this.telnetUsername,
    this.telnetPassword,
    this.telnetPort,
    this.telnetThemeId,
    this.environmentVariables = const [],
    String? encoding,
    String? telnetEncoding,
    required this.type,
    this.keyId,
    this.keyUuid,
    this.shellPath,
    this.workDir,
    this.os,
    this.distro,
    this.tagUuids = const [],
    this.createdAt,
    this.updatedAt,
    this.deletedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  }) : sshEnabledOverride = sshEnabled,
       moshEnabledOverride = moshEnabled,
       moshServerCommandOverride = moshServerCommand,
       telnetEnabledOverride = telnetEnabled,
       encodingOverride = encoding,
       telnetEncodingOverride = telnetEncoding;

  final int? id;
  final String? uuid;
  final String name;
  final int? groupId;
  final String? groupUuid;
  final int? identityId;
  final String? identityUuid;
  final int? proxyId;
  final String? proxyUuid;
  final String? host;
  final int? port;
  final String? username;
  final String? password;
  final String? themeId;
  final int? startupSnippetId;
  final String? startupSnippetUuid;
  final bool? sshEnabledOverride;
  final bool? moshEnabledOverride;
  final String? moshServerCommandOverride;
  final bool? telnetEnabledOverride;
  final int? telnetIdentityId;
  final String? telnetIdentityUuid;
  final String? telnetUsername;
  final String? telnetPassword;
  final int? telnetPort;
  final String? telnetThemeId;
  final List<HostEnvironmentVariable> environmentVariables;
  final String? encodingOverride;
  final String? telnetEncodingOverride;
  final NautermHostType type;
  final int? keyId;
  final String? keyUuid;
  final String? shellPath;
  final String? workDir;
  final String? os;
  final String? distro;
  final List<String> tagUuids;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final DateTime? deletedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  bool get sshEnabled => sshEnabledOverride ?? false;
  bool get moshEnabled => moshEnabledOverride ?? false;
  String get moshServerCommand =>
      moshServerCommandOverride?.trim().isNotEmpty == true
      ? moshServerCommandOverride!.trim()
      : defaultMoshServerCommand;
  bool get telnetEnabled => telnetEnabledOverride ?? false;
  String get encoding => encodingOverride?.trim().isNotEmpty == true
      ? encodingOverride!.trim()
      : defaultHostEncoding;
  String get telnetEncoding => telnetEncodingOverride?.trim().isNotEmpty == true
      ? telnetEncodingOverride!.trim()
      : defaultHostEncoding;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'name': name,
    'group_id': groupId,
    'group_uuid': groupUuid,
    'identity_id': identityId,
    'identity_uuid': identityUuid,
    'proxy_id': proxyId,
    'proxy_uuid': proxyUuid,
    'host': host,
    'port': port,
    'username': username,
    'password': password,
    'theme_id': _normalizeThemeId(themeId),
    'startup_snippet_id': startupSnippetId,
    'startup_snippet_uuid': startupSnippetUuid,
    'ssh_enabled': sshEnabledOverride,
    'mosh_enabled': moshEnabledOverride,
    'mosh_server_command': moshServerCommandOverride,
    'telnet_enabled': telnetEnabledOverride,
    'telnet_identity_id': telnetIdentityId,
    'telnet_identity_uuid': telnetIdentityUuid,
    'telnet_username': telnetUsername,
    'telnet_password': telnetPassword,
    'telnet_port': telnetPort,
    'telnet_theme_id': _normalizeThemeId(telnetThemeId),
    'environment_variables': [
      for (final variable in environmentVariables) variable.toJson(),
    ],
    'encoding': encodingOverride,
    'telnet_encoding': telnetEncodingOverride,
    'type': type.storageValue,
    'key_id': keyId,
    'key_uuid': keyUuid,
    'shell_path': shellPath,
    'work_dir': workDir,
    'os': os,
    'distro': distro,
    'tag_uuids': tagUuids,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'deleted_at': _dateTimeToJson(deletedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  HostEntry withPlatform({required String os, String? distro}) {
    return HostEntry(
      id: id,
      uuid: uuid,
      name: name,
      groupId: groupId,
      groupUuid: groupUuid,
      identityId: identityId,
      identityUuid: identityUuid,
      proxyId: proxyId,
      proxyUuid: proxyUuid,
      host: host,
      port: port,
      username: username,
      password: password,
      themeId: themeId,
      startupSnippetId: startupSnippetId,
      startupSnippetUuid: startupSnippetUuid,
      sshEnabled: sshEnabledOverride,
      moshEnabled: moshEnabledOverride,
      moshServerCommand: moshServerCommandOverride,
      telnetEnabled: telnetEnabledOverride,
      telnetIdentityId: telnetIdentityId,
      telnetIdentityUuid: telnetIdentityUuid,
      telnetUsername: telnetUsername,
      telnetPassword: telnetPassword,
      telnetPort: telnetPort,
      telnetThemeId: telnetThemeId,
      environmentVariables: environmentVariables,
      encoding: encodingOverride,
      telnetEncoding: telnetEncodingOverride,
      type: type,
      keyId: keyId,
      keyUuid: keyUuid,
      shellPath: shellPath,
      workDir: workDir,
      os: os.trim(),
      distro: distro?.trim(),
      tagUuids: tagUuids,
      createdAt: createdAt,
      updatedAt: updatedAt,
      deletedAt: deletedAt,
      version: version,
      createdDeviceId: createdDeviceId,
      updatedDeviceId: updatedDeviceId,
    );
  }

  static HostEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return HostEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      name: _string(map['name']),
      groupId: _intOrNull(map['group_id']),
      groupUuid: _stringOrNull(map['group_uuid']),
      identityId: _intOrNull(map['identity_id']),
      identityUuid: _stringOrNull(map['identity_uuid']),
      proxyId: _intOrNull(map['proxy_id']),
      proxyUuid: _stringOrNull(map['proxy_uuid']),
      host: _stringOrNull(map['host']),
      port: _intOrNull(map['port']),
      username: _stringOrNull(map['username']),
      password: _stringOrNull(map['password']),
      themeId: _normalizeThemeId(_stringOrNull(map['theme_id'])),
      startupSnippetId: _intOrNull(map['startup_snippet_id']),
      startupSnippetUuid: _stringOrNull(map['startup_snippet_uuid']),
      sshEnabled: _boolOrNull(map['ssh_enabled']),
      moshEnabled: _boolOrNull(map['mosh_enabled']),
      moshServerCommand: _stringOrNull(map['mosh_server_command']),
      telnetEnabled: _boolOrNull(map['telnet_enabled']),
      telnetIdentityId: _intOrNull(map['telnet_identity_id']),
      telnetIdentityUuid: _stringOrNull(map['telnet_identity_uuid']),
      telnetUsername: _stringOrNull(map['telnet_username']),
      telnetPassword: _stringOrNull(map['telnet_password']),
      telnetPort: _intOrNull(map['telnet_port']),
      telnetThemeId: _normalizeThemeId(_stringOrNull(map['telnet_theme_id'])),
      environmentVariables: _environmentVariablesOrEmpty(
        map['environment_variables'],
      ),
      encoding: _stringOrNull(map['encoding']),
      telnetEncoding: _stringOrNull(map['telnet_encoding']),
      type: NautermHostType.fromJson(map['type']),
      keyId: _intOrNull(map['key_id']),
      keyUuid: _stringOrNull(map['key_uuid']),
      shellPath: _stringOrNull(map['shell_path']),
      workDir: _stringOrNull(map['work_dir']),
      os: _stringOrNull(map['os']),
      distro: _stringOrNull(map['distro']),
      tagUuids: _stringListOrEmpty(map['tag_uuids']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      deletedAt: _dateTimeOrNull(map['deleted_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

class PortForwardEntry {
  const PortForwardEntry({
    this.id,
    this.uuid,
    required this.name,
    required this.type,
    required this.bindAddress,
    required this.bindPort,
    required this.destinationHost,
    required this.destinationPort,
    required this.connectionId,
    this.hostUuid,
    this.createdAt,
    this.updatedAt,
    this.deletedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  });

  final int? id;
  final String? uuid;
  final String name;
  final String type;
  final String bindAddress;
  final int bindPort;
  final String destinationHost;
  final int destinationPort;
  final int connectionId;
  final String? hostUuid;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final DateTime? deletedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'name': name,
    'type': type,
    'bind_address': bindAddress,
    'bind_port': bindPort,
    'destination_host': destinationHost,
    'destination_port': destinationPort,
    'connection_id': connectionId,
    'host_uuid': hostUuid,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'deleted_at': _dateTimeToJson(deletedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  static PortForwardEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return PortForwardEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      name: _string(map['name']),
      type: _string(map['type']),
      bindAddress: _string(map['bind_address']),
      bindPort: _int(map['bind_port']),
      destinationHost: _string(map['destination_host']),
      destinationPort: _int(map['destination_port']),
      connectionId: _int(map['connection_id']),
      hostUuid: _stringOrNull(map['host_uuid']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      deletedAt: _dateTimeOrNull(map['deleted_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

class ProxyEntry {
  const ProxyEntry({
    this.id,
    this.uuid,
    required this.name,
    required this.type,
    required this.host,
    required this.port,
    this.identityId,
    this.identityUuid,
    this.username,
    this.password,
    this.createdAt,
    this.updatedAt,
    this.deletedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  });

  final int? id;
  final String? uuid;
  final String name;
  final String type;
  final String host;
  final int port;
  final int? identityId;
  final String? identityUuid;
  final String? username;
  final String? password;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final DateTime? deletedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'name': name,
    'type': type,
    'host': host,
    'port': port,
    'identity_id': identityId,
    'identity_uuid': identityUuid,
    'username': username,
    'password': password,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'deleted_at': _dateTimeToJson(deletedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  static ProxyEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return ProxyEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      name: _string(map['name']),
      type: _string(map['type']),
      host: _string(map['host']),
      port: _int(map['port']),
      identityId: _intOrNull(map['identity_id']),
      identityUuid: _stringOrNull(map['identity_uuid']),
      username: _stringOrNull(map['username']),
      password: _stringOrNull(map['password']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      deletedAt: _dateTimeOrNull(map['deleted_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

class AiProviderEntry {
  const AiProviderEntry({
    this.id,
    this.uuid,
    required this.name,
    required this.protocol,
    required this.baseUrl,
    required this.model,
    required this.apiKey,
    this.config = const <String, Object?>{},
    this.active = true,
    this.createdAt,
    this.updatedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  });

  final int? id;
  final String? uuid;
  final String name;
  final String protocol;
  final String baseUrl;
  final String model;
  final String apiKey;
  final Map<String, Object?> config;
  final bool active;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  int? get maxTokens {
    final value = config['max_tokens'];
    final parsed = value is num ? value.toInt() : null;
    return parsed != null && parsed > 0 ? parsed : null;
  }

  double? get temperature => _finiteDouble(config['temperature']);

  List<String> get models => model.isEmpty
      ? []
      : model
            .split(',')
            .map((entry) => entry.trim())
            .where((entry) => entry.isNotEmpty)
            .toList(growable: false);

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'name': name,
    'protocol': protocol,
    'base_url': baseUrl,
    'model': model,
    'api_key': apiKey,
    'config': config,
    'active': active,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  static AiProviderEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return AiProviderEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      name: _string(map['name']),
      protocol: _string(map['protocol']),
      baseUrl: _string(map['base_url']),
      model: _string(map['model']),
      apiKey: _string(map['api_key']),
      config: map['config'] == null
          ? const <String, Object?>{}
          : Map<String, Object?>.unmodifiable(_jsonMap(map['config'])),
      active: _boolOrFalse(map['active']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

double? _finiteDouble(Object? value) {
  final parsed = value is num ? value.toDouble() : null;
  return parsed != null && parsed.isFinite ? parsed : null;
}

class SftpFavoritePathEntry {
  const SftpFavoritePathEntry({
    this.id,
    this.uuid,
    required this.scope,
    this.hostId,
    this.hostUuid,
    required this.path,
    this.createdAt,
    this.updatedAt,
    this.deletedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  });

  final int? id;
  final String? uuid;
  final SftpFavoriteScope scope;
  final int? hostId;
  final String? hostUuid;
  final String path;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final DateTime? deletedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'scope': scope.storageValue,
    'host_id': hostId,
    'host_uuid': hostUuid,
    'path': path,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'deleted_at': _dateTimeToJson(deletedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  static SftpFavoritePathEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return SftpFavoritePathEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      scope: SftpFavoriteScope.fromJson(map['scope']),
      hostId: _intOrNull(map['host_id']),
      hostUuid: _stringOrNull(map['host_uuid']),
      path: _string(map['path']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      deletedAt: _dateTimeOrNull(map['deleted_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

class SnippetPackageEntry {
  const SnippetPackageEntry({
    this.id,
    this.uuid,
    required this.name,
    this.createdAt,
    this.updatedAt,
    this.deletedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  });

  final int? id;
  final String? uuid;
  final String name;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final DateTime? deletedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'name': name,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'deleted_at': _dateTimeToJson(deletedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  static SnippetPackageEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return SnippetPackageEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      name: _string(map['name']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      deletedAt: _dateTimeOrNull(map['deleted_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

class SnippetEntry {
  const SnippetEntry({
    this.id,
    this.uuid,
    this.packageId,
    this.packageUuid,
    this.scope = SnippetScope.global,
    required this.description,
    required this.script,
    this.targetGroupIds = const [],
    this.targetHostIds = const [],
    this.createdAt,
    this.updatedAt,
    this.deletedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  });

  final int? id;
  final String? uuid;
  final int? packageId;
  final String? packageUuid;
  final SnippetScope scope;
  final String description;
  final String script;
  final List<int> targetGroupIds;
  final List<int> targetHostIds;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final DateTime? deletedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'package_id': packageId,
    'package_uuid': packageUuid,
    'scope': scope.storageValue,
    'description': description,
    'script': script,
    'target_group_ids': targetGroupIds,
    'target_host_ids': targetHostIds,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'deleted_at': _dateTimeToJson(deletedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  static SnippetEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return SnippetEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      packageId: _intOrNull(map['package_id']),
      packageUuid: _stringOrNull(map['package_uuid']),
      scope: SnippetScope.fromJson(map['scope']),
      description: _string(map['description']),
      script: _string(map['script']),
      targetGroupIds: _intListOrEmpty(map['target_group_ids']),
      targetHostIds: _intListOrEmpty(map['target_host_ids']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      deletedAt: _dateTimeOrNull(map['deleted_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

class ShellHistoryEntry {
  const ShellHistoryEntry({
    this.id,
    this.sourceId,
    required this.command,
    this.sessionId,
    this.title,
    this.hostId,
    this.host,
    this.port,
    this.username,
    this.shellPath,
    this.cwd,
    this.createdAt,
  });

  final int? id;
  final String? sourceId;
  final String command;
  final String? sessionId;
  final String? title;
  final int? hostId;
  final String? host;
  final int? port;
  final String? username;
  final String? shellPath;
  final String? cwd;
  final DateTime? createdAt;

  ShellHistoryEntry copyWith({
    int? id,
    String? sourceId,
    String? command,
    String? sessionId,
    String? title,
    int? hostId,
    String? host,
    int? port,
    String? username,
    String? shellPath,
    String? cwd,
    DateTime? createdAt,
  }) {
    return ShellHistoryEntry(
      id: id ?? this.id,
      sourceId: sourceId ?? this.sourceId,
      command: command ?? this.command,
      sessionId: sessionId ?? this.sessionId,
      title: title ?? this.title,
      hostId: hostId ?? this.hostId,
      host: host ?? this.host,
      port: port ?? this.port,
      username: username ?? this.username,
      shellPath: shellPath ?? this.shellPath,
      cwd: cwd ?? this.cwd,
      createdAt: createdAt ?? this.createdAt,
    );
  }

  Map<String, Object?> toJson() => {
    'id': id,
    'source_id': sourceId,
    'command': command,
    'session_id': sessionId,
    'title': title,
    'host_id': hostId,
    'host': host,
    'port': port,
    'username': username,
    'shell_path': shellPath,
    'cwd': cwd,
    'created_at': _dateTimeToJson(createdAt),
  };

  static ShellHistoryEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return ShellHistoryEntry(
      id: _intOrNull(map['id']),
      sourceId: _stringOrNull(map['source_id']),
      command: _string(map['command']),
      sessionId: _stringOrNull(map['session_id']),
      title: _stringOrNull(map['title']),
      hostId: _intOrNull(map['host_id']),
      host: _stringOrNull(map['host']),
      port: _intOrNull(map['port']),
      username: _stringOrNull(map['username']),
      shellPath: _stringOrNull(map['shell_path']),
      cwd: _stringOrNull(map['cwd']),
      createdAt: _dateTimeOrNull(map['created_at']),
    );
  }
}

class TerminalLogEntry {
  const TerminalLogEntry({
    required this.id,
    required this.title,
    this.themeId,
    this.hostId,
    this.hostUuid,
    this.host,
    this.port,
    this.username,
    this.shellPath,
    this.workDir,
    this.cwd,
    required this.captureFile,
    this.captureBytes = 0,
    this.captureSha256,
    this.columns,
    this.rows,
    required this.startedAt,
    this.endedAt,
  });

  final String id;
  final String title;
  final String? themeId;
  final int? hostId;
  final String? hostUuid;
  final String? host;
  final int? port;
  final String? username;
  final String? shellPath;
  final String? workDir;
  final String? cwd;
  final String captureFile;
  final int captureBytes;
  final String? captureSha256;
  final int? columns;
  final int? rows;
  final DateTime startedAt;
  final DateTime? endedAt;

  TerminalLogEntry copyWith({
    String? id,
    String? title,
    String? themeId,
    int? hostId,
    String? hostUuid,
    String? host,
    int? port,
    String? username,
    String? shellPath,
    String? workDir,
    String? cwd,
    String? captureFile,
    int? captureBytes,
    String? captureSha256,
    int? columns,
    int? rows,
    DateTime? startedAt,
    DateTime? endedAt,
  }) {
    return TerminalLogEntry(
      id: id ?? this.id,
      title: title ?? this.title,
      themeId: themeId ?? this.themeId,
      hostId: hostId ?? this.hostId,
      hostUuid: hostUuid ?? this.hostUuid,
      host: host ?? this.host,
      port: port ?? this.port,
      username: username ?? this.username,
      shellPath: shellPath ?? this.shellPath,
      workDir: workDir ?? this.workDir,
      cwd: cwd ?? this.cwd,
      captureFile: captureFile ?? this.captureFile,
      captureBytes: captureBytes ?? this.captureBytes,
      captureSha256: captureSha256 ?? this.captureSha256,
      columns: columns ?? this.columns,
      rows: rows ?? this.rows,
      startedAt: startedAt ?? this.startedAt,
      endedAt: endedAt ?? this.endedAt,
    );
  }

  Map<String, Object?> toJson() => {
    'id': id,
    'title': title,
    'theme_id': _normalizeThemeId(themeId),
    'host_id': hostId,
    'host_uuid': hostUuid,
    'host': host,
    'port': port,
    'username': username,
    'shell_path': shellPath,
    'work_dir': workDir,
    'cwd': cwd,
    'capture_file': captureFile,
    'capture_bytes': captureBytes,
    'capture_sha256': captureSha256,
    'columns': columns,
    'rows': rows,
    'started_at': _dateTimeToJson(startedAt),
    'ended_at': _dateTimeToJson(endedAt),
  };

  static TerminalLogEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return TerminalLogEntry(
      id: _string(map['id']),
      title: _string(map['title']),
      themeId: _normalizeThemeId(_stringOrNull(map['theme_id'])),
      hostId: _intOrNull(map['host_id']),
      hostUuid: _stringOrNull(map['host_uuid']),
      host: _stringOrNull(map['host']),
      port: _intOrNull(map['port']),
      username: _stringOrNull(map['username']),
      shellPath: _stringOrNull(map['shell_path']),
      workDir: _stringOrNull(map['work_dir']),
      cwd: _stringOrNull(map['cwd']),
      captureFile: _string(map['capture_file']),
      captureBytes: _intOrNull(map['capture_bytes']) ?? 0,
      captureSha256: _stringOrNull(map['capture_sha256']),
      columns: _intOrNull(map['columns']),
      rows: _intOrNull(map['rows']),
      startedAt: _dateTimeOrNull(map['started_at']) ?? DateTime.now().toUtc(),
      endedAt: _dateTimeOrNull(map['ended_at']),
    );
  }
}

class TerminalLogEvent {
  const TerminalLogEvent({
    this.id,
    this.logId,
    this.logUuid,
    required this.timestamp,
    required this.type,
    required this.message,
    this.connectionKind,
    this.data,
  });

  final int? id;
  final String? logId;
  final String? logUuid;
  final DateTime timestamp;
  final String type;
  final String message;
  final String? connectionKind;
  final String? data;

  Map<String, Object?> toJson() => {
    'id': id,
    'log_id': logId,
    'log_uuid': logUuid,
    'timestamp': _dateTimeToJson(timestamp),
    'type': type,
    'message': message,
    'connection_kind': connectionKind,
    'data': data,
  };

  static TerminalLogEvent fromJson(Object? json) {
    final map = _jsonMap(json);
    return TerminalLogEvent(
      id: _intOrNull(map['id']),
      logId: _stringOrNull(map['log_id']),
      logUuid: _stringOrNull(map['log_uuid']),
      timestamp: _dateTimeOrNull(map['timestamp']) ?? DateTime.now().toUtc(),
      type: _string(map['type']),
      message: _string(map['message']),
      connectionKind: _stringOrNull(map['connection_kind']),
      data: _stringOrNull(map['data']),
    );
  }
}

class AiConversationEntry {
  const AiConversationEntry({
    this.id,
    this.uuid,
    required this.title,
    this.preview,
    required this.scope,
    this.hostUuid,
    this.providerUuid,
    this.model = '',
    this.messages = const [],
    this.commandBlocks = const [],
    this.createdAt,
    this.updatedAt,
    this.version,
    this.createdDeviceId,
    this.updatedDeviceId,
  });

  final int? id;
  final String? uuid;
  final String title;
  final String? preview;
  final String scope;
  final String? hostUuid;
  final String? providerUuid;
  final String model;
  final List<AiMessageEntry> messages;
  final List<AiCommandBlockEntry> commandBlocks;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final int? version;
  final String? createdDeviceId;
  final String? updatedDeviceId;

  List<String> get models => model.isEmpty
      ? []
      : model
            .split(',')
            .map((entry) => entry.trim())
            .where((entry) => entry.isNotEmpty)
            .toList(growable: false);

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'title': title,
    'preview': preview,
    'scope': scope,
    'host_uuid': hostUuid,
    'provider_uuid': providerUuid,
    'model': model,
    'messages': [for (final message in messages) message.toJson()],
    'command_blocks': [for (final block in commandBlocks) block.toJson()],
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'version': version,
    'created_device_id': createdDeviceId,
    'updated_device_id': updatedDeviceId,
  };

  static AiConversationEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return AiConversationEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      title: _string(map['title']),
      preview: _stringOrNull(map['preview']),
      scope: _string(map['scope']),
      hostUuid: _stringOrNull(map['host_uuid']),
      providerUuid: _stringOrNull(map['provider_uuid']),
      model: _string(map['model'] ?? ''),
      messages: _jsonList(map['messages'])
          .map(AiMessageEntry.fromJson)
          .toList(growable: false),
      commandBlocks: _jsonList(map['command_blocks'])
          .map(AiCommandBlockEntry.fromJson)
          .toList(growable: false),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      version: _intOrNull(map['version']),
      createdDeviceId: _stringOrNull(map['created_device_id']),
      updatedDeviceId: _stringOrNull(map['updated_device_id']),
    );
  }
}

class AiMessageEntry {
  const AiMessageEntry({
    this.id,
    this.uuid,
    required this.role,
    required this.content,
    this.context = '',
    required this.sequence,
    this.toolCalls = const [],
    this.toolResult,
    this.attachments = const [],
    this.createdAt,
    this.updatedAt,
    this.version,
  });

  final int? id;
  final String? uuid;
  final String role;
  final String content;
  final String context;
  final int sequence;
  final List<Map<String, Object?>> toolCalls;
  final Map<String, Object?>? toolResult;
  final List<Map<String, Object?>> attachments;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final int? version;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'role': role,
    'content': content,
    'context': context,
    'sequence': sequence,
    'tool_calls': toolCalls,
    'tool_result': toolResult,
    'attachments': attachments,
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'version': version,
  };

  static AiMessageEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return AiMessageEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      role: _string(map['role']),
      content: _string(map['content']),
      context: _stringOrNull(map['context']) ?? '',
      sequence: _int(map['sequence']),
      toolCalls: _jsonList(map['tool_calls'])
          .map(_jsonMap)
          .toList(growable: false),
      toolResult: map['tool_result'] == null
          ? null
          : _jsonMap(map['tool_result']),
      attachments: _jsonList(map['attachments'])
          .map(_jsonMap)
          .toList(growable: false),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      version: _intOrNull(map['version']),
    );
  }
}

class AiCommandBlockEntry {
  const AiCommandBlockEntry({
    this.id,
    this.uuid,
    required this.toolCallId,
    required this.command,
    required this.explanation,
    required this.status,
    required this.sequence,
    this.output,
    this.exitCode,
    this.error,
    this.startedAt,
    this.finishedAt,
    this.createdAt,
    this.updatedAt,
    this.version,
  });

  final int? id;
  final String? uuid;
  final String toolCallId;
  final String command;
  final String explanation;
  final String status;
  final int sequence;
  final String? output;
  final int? exitCode;
  final String? error;
  final DateTime? startedAt;
  final DateTime? finishedAt;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final int? version;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'tool_call_id': toolCallId,
    'command': command,
    'explanation': explanation,
    'status': status,
    'sequence': sequence,
    'output': output,
    'exit_code': exitCode,
    'error': error,
    'started_at': _dateTimeToJson(startedAt),
    'finished_at': _dateTimeToJson(finishedAt),
    'created_at': _dateTimeToJson(createdAt),
    'updated_at': _dateTimeToJson(updatedAt),
    'version': version,
  };

  static AiCommandBlockEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return AiCommandBlockEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      toolCallId: _string(map['tool_call_id']),
      command: _string(map['command']),
      explanation: _string(map['explanation']),
      status: _string(map['status']),
      sequence: _int(map['sequence']),
      output: _stringOrNull(map['output']),
      exitCode: _intOrNull(map['exit_code']),
      error: _stringOrNull(map['error']),
      startedAt: _dateTimeOrNull(map['started_at']),
      finishedAt: _dateTimeOrNull(map['finished_at']),
      createdAt: _dateTimeOrNull(map['created_at']),
      updatedAt: _dateTimeOrNull(map['updated_at']),
      version: _intOrNull(map['version']),
    );
  }
}

class SftpTaskHistoryEntry {
  const SftpTaskHistoryEntry({
    this.id,
    this.uuid,
    this.hostUuid,
    required this.type,
    this.host = '',
    this.username = '',
    this.port = 22,
    required this.status,
    required this.displayName,
    required this.sourcePath,
    required this.targetPath,
    required this.createdAt,
    required this.finishedAt,
    required this.bytes,
    required this.totalBytes,
    required this.itemKind,
    this.error,
  });

  final int? id;
  final String? uuid;
  final String? hostUuid;
  final String type;
  final String host;
  final String username;
  final int port;
  final String status;
  final String displayName;
  final String sourcePath;
  final String targetPath;
  final DateTime createdAt;
  final DateTime finishedAt;
  final int bytes;
  final int totalBytes;
  final String itemKind;
  final String? error;

  Map<String, Object?> toJson() => {
    'id': id,
    'uuid': uuid,
    'host_uuid': hostUuid,
    'type': type,
    'host': host,
    'username': username,
    'port': port,
    'status': status,
    'display_name': displayName,
    'source_path': sourcePath,
    'target_path': targetPath,
    'created_at': createdAt.toUtc().millisecondsSinceEpoch,
    'finished_at': finishedAt.toUtc().millisecondsSinceEpoch,
    'bytes': bytes,
    'total_bytes': totalBytes,
    'item_kind': itemKind,
    'error': error,
  };

  static SftpTaskHistoryEntry fromJson(Object? json) {
    final map = _jsonMap(json);
    return SftpTaskHistoryEntry(
      id: _intOrNull(map['id']),
      uuid: _stringOrNull(map['uuid']),
      hostUuid: _stringOrNull(map['host_uuid']),
      type: _string(map['type']),
      host: _string(map['host']),
      username: _string(map['username']),
      port: _int(map['port']),
      status: _string(map['status']),
      displayName: _string(map['display_name']),
      sourcePath: _string(map['source_path']),
      targetPath: _string(map['target_path']),
      createdAt: _dateTimeOrNull(map['created_at'])!,
      finishedAt: _dateTimeOrNull(map['finished_at'])!,
      bytes: _int(map['bytes']),
      totalBytes: _int(map['total_bytes']),
      itemKind: _string(map['item_kind']),
      error: _stringOrNull(map['error']),
    );
  }
}

class NautermDataException implements Exception {
  const NautermDataException(this.message);

  final String message;

  @override
  String toString() => message;
}

class NautermDataStore {
  NautermDataStore._(this._bindings, this._handle);

  factory NautermDataStore.openDefault() {
    final operation = NautermLog.begin('database', 'Open default database');
    try {
      final bindings = _NautermDataBindings.open();
      final handle = bindings.openDefault();
      if (handle == nullptr) {
        throw const NautermDataException('Unable to open default database.');
      }
      operation.succeed();
      return NautermDataStore._(bindings, handle);
    } on Object catch (error, stackTrace) {
      operation.fail(error, stackTrace: stackTrace);
      rethrow;
    }
  }

  factory NautermDataStore.openPath(String path) {
    final operation = NautermLog.begin('database', 'Open database');
    try {
      final bindings = _NautermDataBindings.open();
      final pathPointer = path.toNativeUtf8();
      try {
        final handle = bindings.openPath(pathPointer);
        if (handle == nullptr) {
          throw NautermDataException('Unable to open database: $path');
        }
        operation.succeed();
        return NautermDataStore._(bindings, handle);
      } finally {
        malloc.free(pathPointer);
      }
    } on Object catch (error, stackTrace) {
      operation.fail(error, stackTrace: stackTrace);
      rethrow;
    }
  }

  static String defaultPath() {
    final bindings = _NautermDataBindings.open();
    final pathPointer = bindings.defaultPath();
    if (pathPointer == nullptr) {
      throw const NautermDataException(
        'Unable to resolve default database path.',
      );
    }
    try {
      return pathPointer.toDartString();
    } finally {
      bindings.freeString(pathPointer);
    }
  }

  final _NautermDataBindings _bindings;
  Pointer<Void> _handle;

  int get schemaVersion => _callInt('schema_version');
  String get deviceId => _callString('device_id');

  /// Reads a key from the `app_metadata` table, or `null` when absent.
  String? getAppMetadata(String key) {
    final data = _call('get_app_meta', {'key': key});
    return data == null ? null : _string(data);
  }

  /// Inserts or updates a key in the `app_metadata` table.
  void setAppMetadata(String key, String value) {
    _call('set_app_meta', {'key': key, 'value': value});
  }

  // ---- Encryption / master-key management -------------------------------------------------

  /// Whether the vault has been initialised and whether a Master Key is configured.
  ({bool initialised, bool hasMasterKey}) encryptionStatus() {
    final data = _call('encryption_status') as Map<String, dynamic>;
    return (
      initialised: data['initialised'] as bool? ?? false,
      hasMasterKey: data['has_master_key'] as bool? ?? false,
    );
  }

  /// Force-initialise the vault (generates the DEK and stores it sealed with the Device Key).
  /// Idempotent — safe to call at every startup.
  ({bool initialised, bool hasMasterKey}) initEncryption() {
    final operation = NautermLog.begin('keyring', 'Initialize encryption');
    try {
      final data = _call('init_encryption') as Map<String, dynamic>;
      final result = (
        initialised: data['initialised'] as bool? ?? false,
        hasMasterKey: data['has_master_key'] as bool? ?? false,
      );
      operation.succeed(
        fields: {
          'initialized': result.initialised,
          'has_master_key': result.hasMasterKey,
        },
      );
      return result;
    } on Object catch (error, stackTrace) {
      operation.fail(error, stackTrace: stackTrace);
      rethrow;
    }
  }

  /// Set the Master Key for the first time. Requires that the Device Key binding is present.
  void setMasterKey(String masterKey) {
    _call('set_master_key', {'master_key': masterKey});
    NautermLog.info('keyring', 'Master key configured.');
  }

  /// Rotate the Master Key. This re-seals the DEK only — records are not re-encrypted.
  void changeMasterKey({
    required String currentMasterKey,
    required String newMasterKey,
  }) {
    _call('change_master_key', {
      'current_master_key': currentMasterKey,
      'new_master_key': newMasterKey,
    });
    NautermLog.info('keyring', 'Master key changed.');
  }

  /// Remove the Master Key (fall back to Device-Key-only unlock).
  void removeMasterKey(String currentMasterKey) {
    _call('remove_master_key', {'current_master_key': currentMasterKey});
    NautermLog.info('keyring', 'Master key removed.');
  }

  /// Returns true if the given Master Key unlocks the vault.
  bool verifyMasterKey(String masterKey) {
    final data = _call('verify_master_key', {
      'master_key': masterKey,
    }) as Map<String, dynamic>;
    return data['ok'] as bool? ?? false;
  }

  // ---- GitHub sync ------------------------------------------------------------------------

  void githubSaveToken(String token) {
    _call('github_save_token', {'token': token});
  }

  bool githubHasToken() {
    final data = _call('github_load_token') as Map<String, dynamic>;
    return data['has_token'] as bool? ?? false;
  }

  String? githubReadToken() {
    final data = _call('github_read_token') as Map<String, dynamic>;
    return data['token'] as String?;
  }

  void githubDeleteToken() {
    _call('github_delete_token');
  }

  void githubGistSaveToken(String token) {
    _call('github_gist_save_token', {'token': token});
  }

  bool githubGistHasToken() {
    final data = _call('github_gist_load_token') as Map<String, dynamic>;
    return data['has_token'] as bool? ?? false;
  }

  void githubGistDeleteToken() {
    _call('github_gist_delete_token');
  }

  void githubGistSaveConfig({
    String gistId = '',
    String filename = 'nauterm-sync.enc',
  }) {
    _call('github_gist_save_config', {
      'config': {'gist_id': gistId, 'filename': filename},
    });
  }

  Map<String, dynamic> githubGistLoadConfig() {
    return Map<String, dynamic>.from(_call('github_gist_load_config') as Map);
  }

  void githubGistDeleteConfig() {
    _call('github_gist_delete_config');
  }

  Map<String, dynamic> githubGistSync({
    String? masterKey,
    String strategy = 'smart_merge',
    int backupCount = 10,
  }) {
    return Map<String, dynamic>.from(
      _call('github_gist_sync', {
        'master_key': masterKey,
        'strategy': strategy,
        'backup_count': backupCount,
      }) as Map,
    );
  }

  Map<String, dynamic> githubGistChangeMasterKey({
    required String currentMasterKey,
    required String newMasterKey,
  }) {
    return Map<String, dynamic>.from(
      _call('github_gist_change_master_key', {
        'current_master_key': currentMasterKey,
        'new_master_key': newMasterKey,
      }) as Map,
    );
  }

  List<Map<String, dynamic>> githubGistListHistory({int limit = 20}) {
    return (_call('github_gist_list_history', {'limit': limit}) as List)
        .map((item) => Map<String, dynamic>.from(item as Map))
        .toList();
  }

  Map<String, dynamic> githubGistRestoreVersion(
    String version, {
    int backupCount = 10,
  }) {
    return Map<String, dynamic>.from(
      _call('github_gist_restore_version', {
        'version': version,
        'backup_count': backupCount,
      }) as Map,
    );
  }

  bool hasLocalSyncKey() {
    final data = _call('sync_key_status') as Map<String, dynamic>;
    return data['has_local_sync_key'] as bool? ?? false;
  }

  void forgetSyncKey() {
    _call('forget_sync_key');
  }

  void githubSaveConfig({
    required String repositoryUrl,
    String branch = 'main',
    String path = 'nauterm-sync.enc',
  }) {
    _call('github_save_config', {
      'config': {
        'repository_url': repositoryUrl,
        'branch': branch,
        'path': path,
      },
    });
  }

  Map<String, dynamic>? githubLoadConfig() {
    final data = _call('github_load_config');
    if (data == null) return null;
    return Map<String, dynamic>.from(data as Map);
  }

  void githubDeleteConfig() {
    _call('github_delete_config');
  }

  /// Pull, merge, and push the encrypted sync file. The temporary encrypted blob only lives
  /// inside the Rust side and is deleted before this call returns.
  Map<String, dynamic> githubSync({
    String? masterKey,
    String strategy = 'smart_merge',
    int backupCount = 10,
  }) {
    return Map<String, dynamic>.from(
      _call('github_sync', {
        'master_key': masterKey,
        'strategy': strategy,
        'backup_count': backupCount,
      }) as Map,
    );
  }

  Map<String, dynamic> githubChangeMasterKey({
    required String currentMasterKey,
    required String newMasterKey,
  }) {
    return Map<String, dynamic>.from(
      _call('github_change_master_key', {
        'current_master_key': currentMasterKey,
        'new_master_key': newMasterKey,
      }) as Map,
    );
  }

  List<Map<String, dynamic>> githubListHistory({int limit = 20}) {
    return (_call('github_list_history', {'limit': limit}) as List)
        .map((item) => Map<String, dynamic>.from(item as Map))
        .toList();
  }

  Map<String, dynamic> githubRestoreRevision(
    String commitSha, {
    int backupCount = 10,
  }) {
    return Map<String, dynamic>.from(
      _call('github_restore_revision', {
        'commit_sha': commitSha,
        'backup_count': backupCount,
      }) as Map,
    );
  }

  // ---- S3-compatible sync ---------------------------------------------------------------

  void s3SaveCredentials({
    required String accessKeyId,
    required String secretAccessKey,
  }) {
    _call('s3_save_credentials', {
      'access_key_id': accessKeyId,
      'secret_access_key': secretAccessKey,
    });
  }

  bool s3HasCredentials() {
    final data = _call('s3_load_credentials') as Map<String, dynamic>;
    return data['has_credentials'] as bool? ?? false;
  }

  Map<String, String>? s3ReadCredentials() {
    final data = _call('s3_read_credentials');
    return data == null ? null : Map<String, String>.from(data as Map);
  }

  void s3DeleteCredentials() {
    _call('s3_delete_credentials');
  }

  void s3SaveConfig({
    required String endpoint,
    required String region,
    required String bucket,
    required String prefix,
    required String filename,
  }) {
    _call('s3_save_config', {
      'config': {
        'endpoint': endpoint,
        'region': region,
        'bucket': bucket,
        'prefix': prefix,
        'filename': filename,
      },
    });
  }

  Map<String, dynamic>? s3LoadConfig() {
    final data = _call('s3_load_config');
    return data == null ? null : Map<String, dynamic>.from(data as Map);
  }

  void s3DeleteConfig() {
    _call('s3_delete_config');
  }

  Map<String, dynamic> s3Sync({
    String? masterKey,
    String strategy = 'smart_merge',
    int backupCount = 10,
  }) {
    return Map<String, dynamic>.from(
      _call('s3_sync', {
        'master_key': masterKey,
        'strategy': strategy,
        'backup_count': backupCount,
      }) as Map,
    );
  }

  List<Map<String, dynamic>> s3ListHistory({int limit = 20}) {
    return (_call('s3_list_history', {'limit': limit}) as List)
        .map((item) => Map<String, dynamic>.from(item as Map))
        .toList(growable: false);
  }

  Map<String, dynamic> s3RestoreVersion(
    String versionId, {
    int backupCount = 10,
  }) {
    return Map<String, dynamic>.from(
      _call('s3_restore_version', {
        'version_id': versionId,
        'backup_count': backupCount,
      }) as Map,
    );
  }

  Map<String, dynamic> s3ChangeMasterKey({
    required String currentMasterKey,
    required String newMasterKey,
  }) {
    return Map<String, dynamic>.from(
      _call('s3_change_master_key', {
        'current_master_key': currentMasterKey,
        'new_master_key': newMasterKey,
      }) as Map,
    );
  }

  // ---- Cloud object sync ---------------------------------------------------------------

  List<Map<String, dynamic>> cloudListProviders() {
    return (_call('cloud_list_providers') as List)
        .map((item) => Map<String, dynamic>.from(item as Map))
        .toList(growable: false);
  }

  Map<String, String>? cloudLoadCredentials(String providerId) {
    final data = _call('cloud_load_credentials', {'provider_id': providerId});
    if (data == null) return null;
    final envelope = Map<String, dynamic>.from(data as Map);
    return Map<String, String>.from(envelope['values'] as Map);
  }

  Map<String, dynamic> syncPreferences() {
    return Map<String, dynamic>.from(_call('sync_preferences') as Map);
  }

  Map<String, dynamic> refreshRemoteSyncStatus() {
    return Map<String, dynamic>.from(
      _call('refresh_remote_sync_status') as Map,
    );
  }

  Map<String, dynamic> saveSyncPreferences({
    required String? activeProviderId,
  }) {
    return Map<String, dynamic>.from(
      _call('save_sync_preferences', {
        'preferences': {'active_provider_id': activeProviderId},
      }) as Map,
    );
  }

  Map<String, dynamic> cloudSaveProvider({
    required Map<String, dynamic> provider,
    Map<String, String>? credentials,
  }) {
    return Map<String, dynamic>.from(
      _call('cloud_save_provider', {
        'provider': provider,
        'credentials': credentials == null
            ? null
            : <String, dynamic>{'values': credentials},
      }) as Map,
    );
  }

  bool cloudDeleteProvider(String providerId) {
    final result = _call('cloud_delete_provider', {'provider_id': providerId});
    return (result as Map<String, dynamic>)['deleted'] as bool? ?? false;
  }

  Map<String, dynamic> cloudSync({
    required String providerId,
    String? masterKey,
    String strategy = 'smart_merge',
    int backupCount = 10,
  }) {
    return Map<String, dynamic>.from(
      _call('cloud_sync', {
        'provider_id': providerId,
        'master_key': masterKey,
        'strategy': strategy,
        'backup_count': backupCount,
      }) as Map,
    );
  }

  List<Map<String, dynamic>> cloudListHistory({
    required String providerId,
    int limit = 20,
  }) {
    return (_call('cloud_list_history', {
          'provider_id': providerId,
          'limit': limit,
        }) as List)
        .map((item) => Map<String, dynamic>.from(item as Map))
        .toList(growable: false);
  }

  Map<String, dynamic> cloudRestoreVersion({
    required String providerId,
    required String versionId,
    int backupCount = 10,
  }) {
    return Map<String, dynamic>.from(
      _call('cloud_restore_version', {
        'provider_id': providerId,
        'version_id': versionId,
        'backup_count': backupCount,
      }) as Map,
    );
  }

  List<Map<String, dynamic>> localSyncBackups() {
    return (_call('local_sync_backup_list') as List)
        .map((item) => Map<String, dynamic>.from(item as Map))
        .toList(growable: false);
  }

  Map<String, dynamic> restoreLocalSyncBackup(
    String backupId, {
    int backupCount = 10,
  }) {
    return Map<String, dynamic>.from(
      _call('local_sync_backup_restore', {
        'backup_id': backupId,
        'backup_count': backupCount,
      }) as Map,
    );
  }

  Map<String, dynamic> cloudChangeMasterKey({
    required String providerId,
    required String currentMasterKey,
    required String newMasterKey,
  }) {
    return Map<String, dynamic>.from(
      _call('cloud_change_master_key', {
        'provider_id': providerId,
        'current_master_key': currentMasterKey,
        'new_master_key': newMasterKey,
      }) as Map,
    );
  }

  void dispose() {
    if (_handle == nullptr) {
      return;
    }
    _bindings.destroy(_handle);
    _handle = nullptr;
  }

  int saveGroup(HostGroup group) {
    return _callInt('save_group', {'group': group.toJson()});
  }

  HostGroup? getGroup(int id) {
    final data = _call('get_group', {'id': id});
    return data == null ? null : HostGroup.fromJson(data);
  }

  List<HostGroup> listGroups() {
    return _callList('list_groups').map(HostGroup.fromJson).toList();
  }

  int deleteGroup(int id) {
    return _callInt('delete_group', {'id': id});
  }

  int saveKey(KeyEntry key) {
    return _callInt('save_key', {'key': key.toJson()});
  }

  KeyEntry? getKey(int id) {
    final data = _call('get_key', {'id': id});
    return data == null ? null : KeyEntry.fromJson(data);
  }

  List<KeyEntry> listKeys() {
    return _callList('list_keys').map(KeyEntry.fromJson).toList();
  }

  int deleteKey(int id) {
    return _callInt('delete_key', {'id': id});
  }

  int saveIdentity(IdentityEntry identity) {
    return _callInt('save_identity', {'identity': identity.toJson()});
  }

  IdentityEntry? getIdentity(int id) {
    final data = _call('get_identity', {'id': id});
    return data == null ? null : IdentityEntry.fromJson(data);
  }

  List<IdentityEntry> listIdentities() {
    return _callList('list_identities').map(IdentityEntry.fromJson).toList();
  }

  int deleteIdentity(int id) {
    return _callInt('delete_identity', {'id': id});
  }

  int saveTag(TagEntry tag) {
    return _callInt('save_tag', {'tag': tag.toJson()});
  }

  TagEntry? getTag(int id) {
    final data = _call('get_tag', {'id': id});
    return data == null ? null : TagEntry.fromJson(data);
  }

  List<TagEntry> listTags() {
    return _callList('list_tags').map(TagEntry.fromJson).toList();
  }

  int deleteTag(int id) {
    return _callInt('delete_tag', {'id': id});
  }

  int saveHost(HostEntry host) {
    return _callInt('save_host', {'host': host.toJson()});
  }

  HostEntry? getHost(int id) {
    final data = _call('get_host', {'id': id});
    return data == null ? null : HostEntry.fromJson(data);
  }

  List<HostEntry> listHosts({int? groupId}) {
    return _callList('list_hosts', {
      'group_id': groupId,
    }).map(HostEntry.fromJson).toList();
  }

  int deleteHost(int id) {
    return _callInt('delete_host', {'id': id});
  }

  int savePortForward(PortForwardEntry portForward) {
    return _callInt('save_port_forward', {
      'port_forward': portForward.toJson(),
    });
  }

  PortForwardEntry? getPortForward(int id) {
    final data = _call('get_port_forward', {'id': id});
    return data == null ? null : PortForwardEntry.fromJson(data);
  }

  List<PortForwardEntry> listPortForwards({int? connectionId}) {
    return _callList('list_port_forwards', {
      'connection_id': connectionId,
    }).map(PortForwardEntry.fromJson).toList();
  }

  int deletePortForward(int id) {
    return _callInt('delete_port_forward', {'id': id});
  }

  int saveProxy(ProxyEntry proxy) {
    return _callInt('save_proxy', {'proxy': proxy.toJson()});
  }

  ProxyEntry? getProxy(int id) {
    final data = _call('get_proxy', {'id': id});
    return data == null ? null : ProxyEntry.fromJson(data);
  }

  List<ProxyEntry> listProxies() {
    return _callList('list_proxies').map(ProxyEntry.fromJson).toList();
  }

  int deleteProxy(int id) {
    return _callInt('delete_proxy', {'id': id});
  }

  AiProviderEntry saveAiProvider(AiProviderEntry provider) {
    return AiProviderEntry.fromJson(
      _call('save_ai_provider', {'provider': provider.toJson()}),
    );
  }

  AiProviderEntry? getActiveAiProvider() {
    final data = _call('get_active_ai_provider');
    return data == null ? null : AiProviderEntry.fromJson(data);
  }

  List<AiProviderEntry> listAiProviders() {
    return _callList('list_ai_providers')
        .map(AiProviderEntry.fromJson)
        .toList(growable: false);
  }

  int deleteAiProvider(int id) {
    return _callInt('delete_ai_provider', {'id': id});
  }

  int saveSftpFavoritePath(SftpFavoritePathEntry favorite) {
    return _callInt('save_sftp_favorite_path', {'favorite': favorite.toJson()});
  }

  List<SftpFavoritePathEntry> listSftpFavoritePaths({
    required SftpFavoriteScope scope,
    int? hostId,
  }) {
    return _callList('list_sftp_favorite_paths', {
      'scope': scope.storageValue,
      'host_id': hostId,
    }).map(SftpFavoritePathEntry.fromJson).toList();
  }

  int deleteSftpFavoritePathByTarget({
    required SftpFavoriteScope scope,
    int? hostId,
    required String path,
  }) {
    return _callInt('delete_sftp_favorite_path_by_target', {
      'scope': scope.storageValue,
      'host_id': hostId,
      'path': path,
    });
  }

  int saveSnippetPackage(SnippetPackageEntry package) {
    return _callInt('save_snippet_package', {'package': package.toJson()});
  }

  List<SnippetPackageEntry> listSnippetPackages() {
    return _callList('list_snippet_packages')
        .map(SnippetPackageEntry.fromJson)
        .toList();
  }

  int deleteSnippetPackage(int id) {
    return _callInt('delete_snippet_package', {'id': id});
  }

  int saveSnippet(SnippetEntry snippet) {
    return _callInt('save_snippet', {'snippet': snippet.toJson()});
  }

  SnippetEntry? getSnippet(int id) {
    final data = _call('get_snippet', {'id': id});
    return data == null ? null : SnippetEntry.fromJson(data);
  }

  List<SnippetEntry> listSnippets({int? packageId}) {
    return _callList('list_snippets', {
      'package_id': packageId,
    }).map(SnippetEntry.fromJson).toList();
  }

  int deleteSnippet(int id) {
    return _callInt('delete_snippet', {'id': id});
  }

  int saveSftpTaskHistory(SftpTaskHistoryEntry task) {
    return _callInt('save_sftp_task_history', {
      'task': task.toJson(),
      'cutoff': DateTime.now()
          .subtract(const Duration(days: 30))
          .toUtc()
          .millisecondsSinceEpoch,
    });
  }

  List<SftpTaskHistoryEntry> listSftpTaskHistory({required DateTime cutoff}) {
    return _callList('list_sftp_task_history', {
      'cutoff': cutoff.toUtc().millisecondsSinceEpoch,
    }).map(SftpTaskHistoryEntry.fromJson).toList(growable: false);
  }

  int deleteSftpTaskHistory(int id) {
    return _callInt('delete_sftp_task_history', {'id': id});
  }

  int clearSftpTaskHistory() {
    return _callInt('clear_sftp_task_history');
  }

  String saveTerminalLog(
    TerminalLogEntry log, {
    List<TerminalLogEvent> events = const [],
  }) {
    return _callString('save_terminal_log', {
      'log': log.toJson(),
      'events': [for (final event in events) event.toJson()],
    });
  }

  List<TerminalLogEntry> listTerminalLogs({int limit = 80, int offset = 0}) {
    return _callList('list_terminal_logs', {
      'limit': limit,
      'offset': offset,
    }).map(TerminalLogEntry.fromJson).toList();
  }

  List<TerminalLogEvent> listTerminalLogEvents(String logId) {
    return _callList('list_terminal_log_events', {
      'log_id': logId,
    }).map(TerminalLogEvent.fromJson).toList();
  }

  String? deleteTerminalLog(String logId) {
    final value = _call('delete_terminal_log', {'log_id': logId});
    return value as String?;
  }

  List<String> clearTerminalLogs() {
    return _callList('clear_terminal_logs').cast<String>();
  }

  List<String> listTerminalCaptureFiles() {
    return _callList('list_terminal_capture_files').cast<String>();
  }

  List<TerminalLogEntry> listIncompleteTerminalCaptures() {
    return _callList('list_incomplete_terminal_captures')
        .map(TerminalLogEntry.fromJson)
        .toList();
  }

  void clearMissingTerminalCapture(String logId) {
    _call('clear_missing_terminal_capture', {'log_id': logId});
  }

  void finalizeRecoveredTerminalCapture({
    required String logId,
    required int captureBytes,
    required String captureSha256,
    required DateTime endedAt,
  }) {
    _call('finalize_recovered_terminal_capture', {
      'log_id': logId,
      'capture_bytes': captureBytes,
      'capture_sha256': captureSha256,
      'ended_at': _dateTimeToJson(endedAt),
    });
  }

  AiConversationEntry saveAiConversation(AiConversationEntry conversation) {
    return AiConversationEntry.fromJson(
      _call('save_ai_conversation', {'conversation': conversation.toJson()}),
    );
  }

  AiConversationEntry? getAiConversation(String uuid) {
    final value = _call('get_ai_conversation', {'uuid': uuid});
    return value == null ? null : AiConversationEntry.fromJson(value);
  }

  List<AiConversationEntry> listAiConversations({
    String? scope,
    String? hostUuid,
    int limit = 100,
  }) {
    return _callList('list_ai_conversations', {
      'scope': scope,
      'host_uuid': hostUuid,
      'limit': limit,
    }).map(AiConversationEntry.fromJson).toList(growable: false);
  }

  int deleteAiConversation(String uuid) {
    return _callInt('delete_ai_conversation', {'uuid': uuid});
  }

  Object? _call(String operation, [Map<String, Object?> payload = const {}]) {
    if (_handle == nullptr) {
      throw const NautermDataException('Database is closed.');
    }

    final requestPointer = jsonEncode({'op': operation, ...payload})
        .toNativeUtf8();
    Pointer<Utf8> responsePointer = nullptr;

    try {
      responsePointer = _bindings.call(_handle, requestPointer);
      if (responsePointer == nullptr) {
        throw const NautermDataException('Database call returned null.');
      }

      final response = jsonDecode(responsePointer.toDartString());
      final responseMap = _jsonMap(response);
      if (responseMap['ok'] != true) {
        throw NautermDataException(
          _stringOrNull(responseMap['error']) ?? 'Database call failed.',
        );
      }
      return responseMap['data'];
    } on Object catch (error, stackTrace) {
      NautermLog.error(
        'database',
        'Database operation failed.',
        error: error,
        stackTrace: stackTrace,
        fields: {'operation': operation},
      );
      rethrow;
    } finally {
      malloc.free(requestPointer);
      if (responsePointer != nullptr) {
        _bindings.freeString(responsePointer);
      }
    }
  }

  int _callInt(String operation, [Map<String, Object?> payload = const {}]) {
    return _int(_call(operation, payload));
  }

  String _callString(
    String operation, [
    Map<String, Object?> payload = const {},
  ]) {
    return _string(_call(operation, payload));
  }

  List<Object?> _callList(
    String operation, [
    Map<String, Object?> payload = const {},
  ]) {
    return _jsonList(_call(operation, payload));
  }
}

typedef _OpenDefaultNative = Pointer<Void> Function();
typedef _OpenDefaultDart = Pointer<Void> Function();

typedef _OpenPathNative = Pointer<Void> Function(Pointer<Utf8> path);
typedef _OpenPathDart = Pointer<Void> Function(Pointer<Utf8> path);

typedef _DestroyNative = Void Function(Pointer<Void> handle);
typedef _DestroyDart = void Function(Pointer<Void> handle);

typedef _CallNative = Pointer<Utf8> Function(
  Pointer<Void> handle,
  Pointer<Utf8> request,
);
typedef _CallDart = Pointer<Utf8> Function(
  Pointer<Void> handle,
  Pointer<Utf8> request,
);

typedef _DefaultPathNative = Pointer<Utf8> Function();
typedef _DefaultPathDart = Pointer<Utf8> Function();

typedef _FreeStringNative = Void Function(Pointer<Utf8> value);
typedef _FreeStringDart = void Function(Pointer<Utf8> value);

class _NautermDataBindings {
  _NautermDataBindings(this.library)
    : openDefault = library
          .lookupFunction<_OpenDefaultNative, _OpenDefaultDart>(
            'nauterm_database_open_default',
          ),
      openPath = library.lookupFunction<_OpenPathNative, _OpenPathDart>(
        'nauterm_database_open_path',
      ),
      destroy = library.lookupFunction<_DestroyNative, _DestroyDart>(
        'nauterm_database_destroy',
      ),
      call = library.lookupFunction<_CallNative, _CallDart>(
        'nauterm_database_call',
      ),
      defaultPath = library
          .lookupFunction<_DefaultPathNative, _DefaultPathDart>(
            'nauterm_database_default_path',
          ),
      freeString = library.lookupFunction<_FreeStringNative, _FreeStringDart>(
        'nauterm_string_free',
      );

  factory _NautermDataBindings.open() {
    final errors = <String>[];
    final candidates = _libraryCandidates();
    for (var index = 0; index < candidates.length; index += 1) {
      final candidate = candidates[index];
      try {
        final bindings = _NautermDataBindings(DynamicLibrary.open(candidate));
        if (!_loadLogged) {
          _loadLogged = true;
          NautermLog.info(
            'native',
            'Database native library loaded.',
            fields: {'candidate_index': index},
          );
        }
        return bindings;
      } on Object catch (error) {
        errors.add('$candidate: $error');
      }
    }

    NautermLog.error(
      'native',
      'Database native library could not be loaded.',
      fields: {'attempt_count': candidates.length},
    );

    throw NautermDataException(
      'Unable to load nauterm_ffi native library.\n${errors.join('\n')}',
    );
  }

  final DynamicLibrary library;
  static bool _loadLogged = false;
  final _OpenDefaultDart openDefault;
  final _OpenPathDart openPath;
  final _DestroyDart destroy;
  final _CallDart call;
  final _DefaultPathDart defaultPath;
  final _FreeStringDart freeString;
}

Map<String, Object?> _jsonMap(Object? value) {
  if (value is Map<String, Object?>) {
    return value;
  }
  if (value is Map) {
    return value.cast<String, Object?>();
  }
  throw FormatException('Expected JSON object, got $value');
}

List<Object?> _jsonList(Object? value) {
  if (value is List<Object?>) {
    return value;
  }
  if (value is List) {
    return value.cast<Object?>();
  }
  throw FormatException('Expected JSON array, got $value');
}

List<int> _intListOrEmpty(Object? value) {
  if (value == null) {
    return const [];
  }
  return _jsonList(value).map(_int).toList(growable: false);
}

List<String> _stringListOrEmpty(Object? value) {
  if (value == null) {
    return const [];
  }
  return _jsonList(value)
      .whereType<String>()
      .map((entry) => entry.trim())
      .where((entry) => entry.isNotEmpty)
      .toList(growable: false);
}

List<HostEnvironmentVariable> _environmentVariablesOrEmpty(Object? value) {
  if (value == null) {
    return const [];
  }
  return _jsonList(value)
      .map(HostEnvironmentVariable.fromJson)
      .where((entry) => entry.variable.trim().isNotEmpty)
      .toList(growable: false);
}

int _int(Object? value) {
  if (value is int) {
    return value;
  }
  throw FormatException('Expected int, got $value');
}

int? _intOrNull(Object? value) {
  if (value == null) {
    return null;
  }
  return _int(value);
}

bool _boolOrFalse(Object? value) {
  if (value == null) {
    return false;
  }
  if (value is bool) {
    return value;
  }
  if (value is int) {
    return value != 0;
  }
  throw FormatException('Expected bool, got $value');
}

bool? _boolOrNull(Object? value) {
  if (value == null) return null;
  return _boolOrFalse(value);
}

String? _normalizeThemeId(String? value) {
  final trimmed = value?.trim();
  if (trimmed == null || trimmed.isEmpty) {
    return null;
  }
  final name = trimmed.split(RegExp(r'[/\\]')).last;
  return name.endsWith('.toml') ? name.substring(0, name.length - 5) : name;
}

String _string(Object? value) {
  if (value is String) {
    return value;
  }
  throw FormatException('Expected string, got $value');
}

String? _stringOrNull(Object? value) {
  if (value == null) {
    return null;
  }
  return _string(value);
}

DateTime? _dateTimeOrNull(Object? value) {
  if (value is num) {
    return DateTime.fromMillisecondsSinceEpoch(value.toInt(), isUtc: true);
  }
  final text = _stringOrNull(value)?.trim();
  if (text == null || text.isEmpty) {
    return null;
  }
  final normalized = text.contains('T') ? text : text.replaceFirst(' ', 'T');
  final withZone =
      normalized.endsWith('Z') ||
          RegExp(r'[+-]\d\d:?\d\d$').hasMatch(normalized)
      ? normalized
      : '${normalized}Z';
  return DateTime.tryParse(withZone)?.toUtc();
}

int? _dateTimeToJson(DateTime? value) => value?.toUtc().millisecondsSinceEpoch;

List<String> _libraryCandidates() {
  final name = _libraryName();
  final separator = Platform.pathSeparator;
  final root = Directory.current.path;
  final executableDirectory = File(Platform.resolvedExecutable).parent.path;
  final candidates = <String>[
    [root, 'native', 'nauterm_ffi', 'target', 'debug', name].join(separator),
    [root, 'native', 'nauterm_ffi', 'target', 'release', name].join(separator),
    '$executableDirectory$separator$name',
    [executableDirectory, 'lib', name].join(separator),
    if (Platform.isMacOS)
      [executableDirectory, '..', 'Frameworks', name].join(separator),
    name,
  ];

  return candidates.toSet().toList(growable: false);
}

String _libraryName() {
  if (Platform.isMacOS) {
    return 'libnauterm_ffi.dylib';
  }
  if (Platform.isWindows) {
    return 'nauterm_ffi.dll';
  }
  return 'libnauterm_ffi.so';
}
