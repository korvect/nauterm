part of 'terminal_ffi.dart';

final class _NativeTerminalCell extends Struct {
  @Uint32()
  external int textOffset;

  @Uint16()
  external int textLength;

  @Uint16()
  external int flags;

  @Uint32()
  external int foreground;

  @Uint32()
  external int background;

  @Uint32()
  external int hyperlinkOffset;

  @Uint32()
  external int hyperlinkLength;
}

final class _NativeTerminalGraphicImage extends Struct {
  @Uint32()
  external int id;

  @Uint64()
  external int generation;

  @Uint32()
  external int width;

  @Uint32()
  external int height;

  @IntPtr()
  external int dataOffset;

  @IntPtr()
  external int dataLength;
}

final class _NativeTerminalGraphicPlacement extends Struct {
  @Uint32()
  external int imageId;

  @Uint32()
  external int placementId;

  @Int32()
  external int zIndex;

  @Int32()
  external int viewportColumn;

  @Int32()
  external int viewportRow;

  @Uint32()
  external int columns;

  @Uint32()
  external int rows;

  @Uint32()
  external int sourceX;

  @Uint32()
  external int sourceY;

  @Uint32()
  external int sourceWidth;

  @Uint32()
  external int sourceHeight;
}

final class _NativeTerminalSnapshot extends Struct {
  @Uint32()
  external int emulatorBackend;

  @Uint32()
  external int columns;

  @Uint32()
  external int rows;

  @Uint32()
  external int historyLines;

  @Uint32()
  external int displayOffset;

  @IntPtr()
  external int titleLength;

  external Pointer<Uint8> title;

  @IntPtr()
  external int clipboardLength;

  external Pointer<Uint8> clipboard;

  @Uint64()
  external int bellCount;

  @Uint32()
  external int cursorColumn;

  @Uint32()
  external int cursorRow;

  @Uint32()
  external int cursorVisible;

  @Uint32()
  external int cursorShape;

  @Uint32()
  external int cursorColor;

  @Uint32()
  external int cursorBlinking;

  @Uint32()
  external int keyboardMode;

  @Uint32()
  external int inputEchoEnabled;

  @Uint32()
  external int alternateScreen;

  @IntPtr()
  external int cellsLength;

  external Pointer<_NativeTerminalCell> cells;

  @IntPtr()
  external int textLength;

  external Pointer<Uint8> text;

  @IntPtr()
  external int hyperlinkTextLength;

  external Pointer<Uint8> hyperlinkText;

  @IntPtr()
  external int graphicImagesLength;

  external Pointer<_NativeTerminalGraphicImage> graphicImages;

  @IntPtr()
  external int graphicPlacementsLength;

  external Pointer<_NativeTerminalGraphicPlacement> graphicPlacements;

  @IntPtr()
  external int graphicDataLength;

  external Pointer<Uint8> graphicData;
}

typedef _CreateLocalSessionConfiguredNative =
    Uint64 Function(
      Uint32 columns,
      Uint32 rows,
      Uint32 emulatorBackend,
      Uint32 scrollbackLines,
      Pointer<Utf8> terminalType,
      Uint32 colorTerm,
      Uint32 osc52Mode,
      Uint32 cursorShape,
      Bool cursorBlink,
      Uint32 defaultForeground,
      Uint32 defaultBackground,
      Uint32 defaultCursor,
      Pointer<Utf8> shellPath,
      Pointer<Utf8> workingDirectory,
      Pointer<Utf8> environment,
    );
typedef _CreateLocalSessionConfiguredDart =
    int Function(
      int columns,
      int rows,
      int emulatorBackend,
      int scrollbackLines,
      Pointer<Utf8> terminalType,
      int colorTerm,
      int osc52Mode,
      int cursorShape,
      bool cursorBlink,
      int defaultForeground,
      int defaultBackground,
      int defaultCursor,
      Pointer<Utf8> shellPath,
      Pointer<Utf8> workingDirectory,
      Pointer<Utf8> environment,
    );

typedef _CreateCommandSessionConfiguredNative =
    Uint64 Function(
      Uint32 columns,
      Uint32 rows,
      Uint32 emulatorBackend,
      Uint32 scrollbackLines,
      Pointer<Utf8> terminalType,
      Uint32 colorTerm,
      Uint32 osc52Mode,
      Uint32 cursorShape,
      Bool cursorBlink,
      Uint32 defaultForeground,
      Uint32 defaultBackground,
      Uint32 defaultCursor,
      Pointer<Utf8> program,
      Pointer<Utf8> args,
      Pointer<Utf8> workingDirectory,
      Pointer<Utf8> environment,
    );
typedef _CreateCommandSessionConfiguredDart =
    int Function(
      int columns,
      int rows,
      int emulatorBackend,
      int scrollbackLines,
      Pointer<Utf8> terminalType,
      int colorTerm,
      int osc52Mode,
      int cursorShape,
      bool cursorBlink,
      int defaultForeground,
      int defaultBackground,
      int defaultCursor,
      Pointer<Utf8> program,
      Pointer<Utf8> args,
      Pointer<Utf8> workingDirectory,
      Pointer<Utf8> environment,
    );

typedef _CreateSshSessionConfiguredNative =
    Uint64 Function(
      Uint32 columns,
      Uint32 rows,
      Uint32 emulatorBackend,
      Uint32 scrollbackLines,
      Pointer<Utf8> terminalType,
      Uint32 colorTerm,
      Uint32 osc52Mode,
      Uint32 cursorShape,
      Bool cursorBlink,
      Uint32 defaultForeground,
      Uint32 defaultBackground,
      Uint32 defaultCursor,
      Uint32 sshKeepaliveIntervalSeconds,
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Uint32 hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      Pointer<Utf8> environment,
      Pointer<Utf8> encoding,
    );
typedef _CreateSshSessionConfiguredDart =
    int Function(
      int columns,
      int rows,
      int emulatorBackend,
      int scrollbackLines,
      Pointer<Utf8> terminalType,
      int colorTerm,
      int osc52Mode,
      int cursorShape,
      bool cursorBlink,
      int defaultForeground,
      int defaultBackground,
      int defaultCursor,
      int sshKeepaliveIntervalSeconds,
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      int hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      Pointer<Utf8> environment,
      Pointer<Utf8> encoding,
    );

typedef _CreateMoshSessionConfiguredNative =
    Uint64 Function(
      Uint32 columns,
      Uint32 rows,
      Uint32 emulatorBackend,
      Uint32 scrollbackLines,
      Pointer<Utf8> terminalType,
      Uint32 colorTerm,
      Uint32 osc52Mode,
      Uint32 cursorShape,
      Bool cursorBlink,
      Uint32 defaultForeground,
      Uint32 defaultBackground,
      Uint32 defaultCursor,
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Uint32 hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      Pointer<Utf8> environment,
      Pointer<Utf8> serverCommand,
    );
typedef _CreateMoshSessionConfiguredDart =
    int Function(
      int columns,
      int rows,
      int emulatorBackend,
      int scrollbackLines,
      Pointer<Utf8> terminalType,
      int colorTerm,
      int osc52Mode,
      int cursorShape,
      bool cursorBlink,
      int defaultForeground,
      int defaultBackground,
      int defaultCursor,
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      int hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      Pointer<Utf8> environment,
      Pointer<Utf8> serverCommand,
    );

typedef _ReconnectSshSessionNative =
    Bool Function(
      Uint64 sessionId,
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Uint32 hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      Uint32 sshKeepaliveIntervalSeconds,
      Pointer<Utf8> encoding,
    );
typedef _ReconnectSshSessionDart =
    bool Function(
      int sessionId,
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      int hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      int sshKeepaliveIntervalSeconds,
      Pointer<Utf8> encoding,
    );

typedef _ReconnectMoshSessionNative =
    Bool Function(
      Uint64 sessionId,
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Uint32 hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      Pointer<Utf8> serverCommand,
    );
typedef _ReconnectMoshSessionDart =
    bool Function(
      int sessionId,
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      int hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      Pointer<Utf8> serverCommand,
    );

typedef _CreateTelnetSessionConfiguredNative =
    Uint64 Function(
      Uint32 columns,
      Uint32 rows,
      Uint32 emulatorBackend,
      Uint32 scrollbackLines,
      Pointer<Utf8> terminalType,
      Uint32 colorTerm,
      Uint32 osc52Mode,
      Uint32 cursorShape,
      Bool cursorBlink,
      Uint32 defaultForeground,
      Uint32 defaultBackground,
      Uint32 defaultCursor,
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> encoding,
      Pointer<Utf8> environment,
    );
typedef _CreateTelnetSessionConfiguredDart =
    int Function(
      int columns,
      int rows,
      int emulatorBackend,
      int scrollbackLines,
      Pointer<Utf8> terminalType,
      int colorTerm,
      int osc52Mode,
      int cursorShape,
      bool cursorBlink,
      int defaultForeground,
      int defaultBackground,
      int defaultCursor,
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> encoding,
      Pointer<Utf8> environment,
    );

typedef _CreateSerialSessionConfiguredNative =
    Uint64 Function(
      Uint32 columns,
      Uint32 rows,
      Uint32 emulatorBackend,
      Uint32 scrollbackLines,
      Pointer<Utf8> terminalType,
      Uint32 colorTerm,
      Uint32 osc52Mode,
      Uint32 cursorShape,
      Bool cursorBlink,
      Uint32 defaultForeground,
      Uint32 defaultBackground,
      Uint32 defaultCursor,
      Pointer<Utf8> serialPort,
      Uint32 baudRate,
      Uint32 dataBits,
      Uint32 parity,
      Uint32 stopBits,
      Uint32 flowControl,
    );
typedef _CreateSerialSessionConfiguredDart =
    int Function(
      int columns,
      int rows,
      int emulatorBackend,
      int scrollbackLines,
      Pointer<Utf8> terminalType,
      int colorTerm,
      int osc52Mode,
      int cursorShape,
      bool cursorBlink,
      int defaultForeground,
      int defaultBackground,
      int defaultCursor,
      Pointer<Utf8> serialPort,
      int baudRate,
      int dataBits,
      int parity,
      int stopBits,
      int flowControl,
    );

typedef _CreateTerminalConfiguredNative =
    Pointer<Void> Function(
      Uint32 columns,
      Uint32 rows,
      Uint32 emulatorBackend,
      Uint32 scrollbackLines,
      Pointer<Utf8> terminalType,
      Uint32 colorTerm,
      Uint32 osc52Mode,
      Uint32 cursorShape,
      Bool cursorBlink,
      Uint32 defaultForeground,
      Uint32 defaultBackground,
      Uint32 defaultCursor,
    );
typedef _CreateTerminalConfiguredDart =
    Pointer<Void> Function(
      int columns,
      int rows,
      int emulatorBackend,
      int scrollbackLines,
      Pointer<Utf8> terminalType,
      int colorTerm,
      int osc52Mode,
      int cursorShape,
      bool cursorBlink,
      int defaultForeground,
      int defaultBackground,
      int defaultCursor,
    );

typedef _DestroyTerminalNative = Void Function(Pointer<Void> handle);
typedef _DestroyTerminalDart = void Function(Pointer<Void> handle);

typedef _CloseSessionNative = Bool Function(Uint64 sessionId);
typedef _CloseSessionDart = bool Function(int sessionId);

typedef _RuntimeLifecycleNative = Void Function();
typedef _RuntimeLifecycleDart = void Function();

typedef _CapturePrepareDirectoryNative = Bool Function(Pointer<Utf8>);
typedef _CapturePrepareDirectoryDart = bool Function(Pointer<Utf8>);
typedef _CaptureOpenNative = Uint64 Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _CaptureOpenDart = int Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _CaptureOpenBoolNative = Bool Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _CaptureOpenBoolDart = bool Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _CaptureRecoverNative =
    Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _CaptureRecoverDart =
    Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _CaptureAppendNative = Bool Function(Uint64, Pointer<Uint8>, IntPtr);
typedef _CaptureAppendDart = bool Function(int, Pointer<Uint8>, int);
typedef _CaptureHandleBoolNative = Bool Function(Uint64);
typedef _CaptureHandleBoolDart = bool Function(int);
typedef _CaptureHandleJsonNative = Pointer<Utf8> Function(Uint64);
typedef _CaptureHandleJsonDart = Pointer<Utf8> Function(int);
typedef _CaptureReaderNextNative = Pointer<Utf8> Function(Uint64);
typedef _CaptureReaderNextDart = Pointer<Utf8> Function(int);
typedef _CaptureReaderCloseNative = Void Function(Uint64);
typedef _CaptureReaderCloseDart = void Function(int);

typedef _ResizeSessionNative =
    Bool Function(
      Uint64 sessionId,
      Uint32 columns,
      Uint32 rows,
      Uint32 cellWidth,
      Uint32 cellHeight,
    );
typedef _ResizeSessionDart =
    bool Function(
      int sessionId,
      int columns,
      int rows,
      int cellWidth,
      int cellHeight,
    );

typedef _NotifyNetworkChangedNative = Bool Function(Uint64 sessionId);
typedef _NotifyNetworkChangedDart = bool Function(int sessionId);

typedef _ExitAlternateScreenNative = Bool Function(Uint64 sessionId);
typedef _ExitAlternateScreenDart = bool Function(int sessionId);

typedef _ScrollLinesNative = Bool Function(Uint64 sessionId, Int32 lines);
typedef _ScrollLinesDart = bool Function(int sessionId, int lines);

typedef _ScrollPageNative = Bool Function(Uint64 sessionId);
typedef _ScrollPageDart = bool Function(int sessionId);

typedef _SearchSessionNative =
    Pointer<Utf8> Function(
      Uint64 sessionId,
      Pointer<Utf8> query,
      Uint32 direction,
      Uint32 originRow,
      Uint32 originColumn,
    );
typedef _SearchSessionDart =
    Pointer<Utf8> Function(
      int sessionId,
      Pointer<Utf8> query,
      int direction,
      int originRow,
      int originColumn,
    );

typedef _SelectionTextSessionNative =
    Pointer<Utf8> Function(Uint64 sessionId, Int64 start, Int64 end);
typedef _SelectionTextSessionDart =
    Pointer<Utf8> Function(int sessionId, int start, int end);

typedef _TerminalSelectionTextNative =
    Pointer<Utf8> Function(Pointer<Void> handle, Int64 start, Int64 end);
typedef _TerminalSelectionTextDart =
    Pointer<Utf8> Function(Pointer<Void> handle, int start, int end);

typedef _CommandBlockSessionNative =
    Pointer<Utf8> Function(Uint64 sessionId, Int64 offset);
typedef _CommandBlockSessionDart =
    Pointer<Utf8> Function(int sessionId, int offset);

typedef _TerminalCommandBlockNative =
    Pointer<Utf8> Function(Pointer<Void> handle, Int64 offset);
typedef _TerminalCommandBlockDart =
    Pointer<Utf8> Function(Pointer<Void> handle, int offset);

typedef _WakeupNative = Void Function(Pointer<Void> userData);

typedef _SetWakeupNative =
    Bool Function(
      Uint64 sessionId,
      Pointer<NativeFunction<_WakeupNative>> callback,
      Pointer<Void> userData,
    );
typedef _SetWakeupDart =
    bool Function(
      int sessionId,
      Pointer<NativeFunction<_WakeupNative>> callback,
      Pointer<Void> userData,
    );

typedef _PollSessionNative = Bool Function(Uint64 sessionId);
typedef _PollSessionDart = bool Function(int sessionId);

typedef _IsExitedSessionNative = Bool Function(Uint64 sessionId);
typedef _IsExitedSessionDart = bool Function(int sessionId);

typedef _WriteCodepointNative =
    Bool Function(Uint64 sessionId, Uint32 codepoint);
typedef _WriteCodepointDart = bool Function(int sessionId, int codepoint);

typedef _WriteSessionBytesNative =
    Bool Function(Uint64 sessionId, Pointer<Uint8> bytes, IntPtr len);
typedef _WriteSessionBytesDart =
    bool Function(int sessionId, Pointer<Uint8> bytes, int len);

typedef _SendInputCodepointNative =
    Bool Function(Uint64 sessionId, Uint32 codepoint);
typedef _SendInputCodepointDart = bool Function(int sessionId, int codepoint);

typedef _SendInputBytesStatusNative =
    Uint32 Function(Uint64 sessionId, Pointer<Uint8> bytes, IntPtr len);
typedef _SendInputBytesStatusDart =
    int Function(int sessionId, Pointer<Uint8> bytes, int len);

typedef _SnapshotNative =
    Pointer<_NativeTerminalSnapshot> Function(Uint64 sessionId);
typedef _SnapshotDart =
    Pointer<_NativeTerminalSnapshot> Function(int sessionId);

typedef _BellCountNative = Uint64 Function(Uint64 sessionId);
typedef _BellCountDart = int Function(int sessionId);

typedef _ClipboardNative = Pointer<Utf8> Function(Uint64 sessionId);
typedef _ClipboardDart = Pointer<Utf8> Function(int sessionId);

typedef _TerminalResizeNative =
    Void Function(
      Pointer<Void> handle,
      Uint32 columns,
      Uint32 rows,
      Uint32 cellWidth,
      Uint32 cellHeight,
    );
typedef _TerminalResizeDart =
    void Function(
      Pointer<Void> handle,
      int columns,
      int rows,
      int cellWidth,
      int cellHeight,
    );

typedef _TerminalScrollLinesNative =
    Bool Function(Pointer<Void> handle, Int32 lines);
typedef _TerminalScrollLinesDart =
    bool Function(Pointer<Void> handle, int lines);

typedef _TerminalScrollPageNative = Bool Function(Pointer<Void> handle);
typedef _TerminalScrollPageDart = bool Function(Pointer<Void> handle);

typedef _TerminalSearchNative =
    Pointer<Utf8> Function(
      Pointer<Void> handle,
      Pointer<Utf8> query,
      Uint32 direction,
      Uint32 originRow,
      Uint32 originColumn,
    );
typedef _TerminalSearchDart =
    Pointer<Utf8> Function(
      Pointer<Void> handle,
      Pointer<Utf8> query,
      int direction,
      int originRow,
      int originColumn,
    );

typedef _TerminalWriteCodepointNative =
    Bool Function(Pointer<Void> handle, Uint32 codepoint);
typedef _TerminalWriteCodepointDart =
    bool Function(Pointer<Void> handle, int codepoint);

typedef _TerminalWriteBytesNative =
    Bool Function(Pointer<Void> handle, Pointer<Uint8> bytes, IntPtr len);
typedef _TerminalWriteBytesDart =
    bool Function(Pointer<Void> handle, Pointer<Uint8> bytes, int len);

typedef _TerminalSnapshotNative =
    Pointer<_NativeTerminalSnapshot> Function(Pointer<Void> handle);
typedef _TerminalSnapshotDart =
    Pointer<_NativeTerminalSnapshot> Function(Pointer<Void> handle);

typedef _TerminalPlainTextNative = Pointer<Utf8> Function(Pointer<Void> handle);
typedef _TerminalPlainTextDart = Pointer<Utf8> Function(Pointer<Void> handle);

typedef _FreeSnapshotNative =
    Void Function(Pointer<_NativeTerminalSnapshot> snapshot);
typedef _FreeSnapshotDart =
    void Function(Pointer<_NativeTerminalSnapshot> snapshot);

typedef _DrainConnectionEventsNative = Pointer<Utf8> Function(Uint64 sessionId);
typedef _DrainConnectionEventsDart = Pointer<Utf8> Function(int sessionId);

typedef _DrainOutputCaptureNative = Pointer<Utf8> Function(Uint64 sessionId);
typedef _DrainOutputCaptureDart = Pointer<Utf8> Function(int sessionId);
typedef _ReadSessionShellHistoryNative =
    Pointer<Utf8> Function(Uint64 sessionId);
typedef _ReadSessionShellHistoryDart = Pointer<Utf8> Function(int sessionId);

typedef _SuppressOutputUntilNative =
    Bool Function(Uint64 sessionId, Pointer<Uint8> marker, IntPtr len);
typedef _SuppressOutputUntilDart =
    bool Function(int sessionId, Pointer<Uint8> marker, int len);

typedef _CancelOutputSuppressionNative = Bool Function(Uint64 sessionId);
typedef _CancelOutputSuppressionDart = bool Function(int sessionId);

typedef _ListSerialPortsNative = Pointer<Utf8> Function();
typedef _ListSerialPortsDart = Pointer<Utf8> Function();

typedef _StartPortForwardNative =
    Pointer<Utf8> Function(
      Uint64 id,
      Pointer<Utf8> forwardType,
      Pointer<Utf8> sshHost,
      Uint16 sshPort,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Pointer<Utf8> bindAddress,
      Uint16 bindPort,
      Pointer<Utf8> destinationHost,
      Uint16 destinationPort,
      Pointer<Utf8> proxyJson,
    );
typedef _StartPortForwardDart =
    Pointer<Utf8> Function(
      int id,
      Pointer<Utf8> forwardType,
      Pointer<Utf8> sshHost,
      int sshPort,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Pointer<Utf8> bindAddress,
      int bindPort,
      Pointer<Utf8> destinationHost,
      int destinationPort,
      Pointer<Utf8> proxyJson,
    );

typedef _StopPortForwardNative = Bool Function(Uint64 id);
typedef _StopPortForwardDart = bool Function(int id);

typedef _StopAllPortForwardsNative = Uint32 Function();
typedef _StopAllPortForwardsDart = int Function();

typedef _PortForwardStatusNative = Pointer<Utf8> Function(Uint64 id);
typedef _PortForwardStatusDart = Pointer<Utf8> Function(int id);

typedef _SshListDirectoriesNative =
    Pointer<Utf8> Function(
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Pointer<Utf8> directory,
    );
typedef _SshListDirectoriesDart =
    Pointer<Utf8> Function(
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Pointer<Utf8> directory,
    );

typedef _SshListDirectoryEntriesNative =
    Pointer<Utf8> Function(
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Pointer<Utf8> directory,
    );
typedef _SshListDirectoryEntriesDart =
    Pointer<Utf8> Function(
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Pointer<Utf8> directory,
    );

typedef _SshDetectHostOsNative =
    Pointer<Utf8> Function(
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Uint32 hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
    );
typedef _SshDetectHostOsDart =
    Pointer<Utf8> Function(
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      int hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
    );

typedef _SshCollectSystemInfoNative =
    Pointer<Utf8> Function(
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Uint32 hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
    );
typedef _SshCollectSystemInfoDart =
    Pointer<Utf8> Function(
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      int hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
    );

typedef _SshExportPublicKeyNative =
    Pointer<Utf8> Function(
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Uint32 hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      Pointer<Utf8> publicKey,
      Pointer<Utf8> location,
      Pointer<Utf8> filename,
      Pointer<Utf8> script,
    );
typedef _SshExportPublicKeyDart =
    Pointer<Utf8> Function(
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      int hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      Pointer<Utf8> publicKey,
      Pointer<Utf8> location,
      Pointer<Utf8> filename,
      Pointer<Utf8> script,
    );

typedef _SftpListDirectoryEntriesNative =
    Pointer<Utf8> Function(
      Uint64 requestId,
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Pointer<Utf8> directory,
      Uint32 hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
    );
typedef _SftpListDirectoryEntriesDart =
    Pointer<Utf8> Function(
      int requestId,
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Pointer<Utf8> directory,
      int hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
    );

typedef _SftpExecuteTaskNative =
    Pointer<Utf8> Function(
      Uint64 taskId,
      Pointer<Utf8> host,
      Uint16 port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Pointer<Utf8> operationJson,
      Uint32 hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      Pointer<NativeFunction<_SftpTaskProgressCallbackNative>> progressCallback,
      Pointer<Void> progressUserData,
    );
typedef _SftpExecuteTaskDart =
    Pointer<Utf8> Function(
      int taskId,
      Pointer<Utf8> host,
      int port,
      Pointer<Utf8> username,
      Pointer<Utf8> password,
      Pointer<Utf8> privateKey,
      Pointer<Utf8> passphrase,
      Pointer<Utf8> knownHostsPath,
      Pointer<Utf8> operationJson,
      int hostKeyTrustMode,
      Pointer<Utf8> proxyJson,
      Pointer<NativeFunction<_SftpTaskProgressCallbackNative>> progressCallback,
      Pointer<Void> progressUserData,
    );

typedef _SftpTaskProgressCallbackNative =
    Void Function(
      Pointer<Void> userData,
      Uint64 transferredBytes,
      Uint64 totalBytes,
      Pointer<Utf8> currentPath,
    );
typedef _SftpCancelTaskNative = Bool Function(Uint64 taskId);
typedef _SftpCancelTaskDart = bool Function(int taskId);
typedef _SftpCloseSudoSessionNative = Bool Function(Pointer<Utf8> sessionId);
typedef _SftpCloseSudoSessionDart = bool Function(Pointer<Utf8> sessionId);

typedef _FreeStringNative = Void Function(Pointer<Utf8> text);
typedef _FreeStringDart = void Function(Pointer<Utf8> text);
