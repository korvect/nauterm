enum SerialParity { none, even, odd }

enum SerialFlowControl { none, software, hardware }

class SerialConnectionConfig {
  const SerialConnectionConfig({
    required this.serialPort,
    required this.baudRate,
    this.dataBits = 8,
    this.parity = SerialParity.none,
    this.stopBits = 1,
    this.flowControl = SerialFlowControl.none,
  });

  factory SerialConnectionConfig.fromHostConfig({
    required String serialPort,
    required int baudRate,
    required Map<String, Object?> config,
  }) {
    final options = config['serial'];
    if (options != null && options is! Map) {
      throw const FormatException('Serial config must be an object.');
    }
    final json = options is Map ? options : const <String, Object?>{};
    final dataBits = json['data_bits'] ?? 8;
    final stopBits = json['stop_bits'] ?? 1;
    final parity = json['parity'] ?? 'none';
    final flowControl = json['flow_control'] ?? 'none';
    if (serialPort.trim().isEmpty ||
        baudRate <= 0 ||
        baudRate > 0xffffffff ||
        dataBits is! int ||
        dataBits < 5 ||
        dataBits > 8 ||
        stopBits is! int ||
        (stopBits != 1 && stopBits != 2)) {
      throw const FormatException('Invalid serial connection settings.');
    }
    return SerialConnectionConfig(
      serialPort: serialPort,
      baudRate: baudRate,
      dataBits: dataBits,
      stopBits: stopBits,
      parity: SerialParity.values.firstWhere(
        (value) => value.name == parity,
        orElse: () => throw const FormatException('Invalid serial parity.'),
      ),
      flowControl: SerialFlowControl.values.firstWhere(
        (value) => value.name == flowControl,
        orElse: () =>
            throw const FormatException('Invalid serial flow control.'),
      ),
    );
  }

  Map<String, Object?> mergeHostConfig(Map<String, Object?> config) => {
    ...config,
    'serial': {
      if (config['serial'] case final Map options)
        ...Map<String, Object?>.from(options),
      'data_bits': dataBits,
      'parity': parity.name,
      'stop_bits': stopBits,
      'flow_control': flowControl.name,
    },
  };

  final String serialPort;
  final int baudRate;
  final int dataBits;
  final SerialParity parity;
  final int stopBits;
  final SerialFlowControl flowControl;

  String get framingLabel {
    final parityLabel = switch (parity) {
      SerialParity.none => 'N',
      SerialParity.even => 'E',
      SerialParity.odd => 'O',
    };
    return '$dataBits$parityLabel$stopBits';
  }

  String get flowControlLabel => switch (flowControl) {
    SerialFlowControl.none => 'none',
    SerialFlowControl.software => 'software',
    SerialFlowControl.hardware => 'hardware',
  };

  String get summary => '$baudRate baud, $framingLabel, flow $flowControlLabel';
}
