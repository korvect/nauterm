part of 'terminal_ffi.dart';

class _TerminalBindings {
  static const _expectedAbiVersion = 5;

  _TerminalBindings(this.library)
    : createLocalSessionConfigured = library
          .lookupFunction<
            _CreateLocalSessionConfiguredNative,
            _CreateLocalSessionConfiguredDart
          >('nauterm_session_create_local_configured'),
      createTerminalConfigured = library
          .lookupFunction<
            _CreateTerminalConfiguredNative,
            _CreateTerminalConfiguredDart
          >('nauterm_terminal_create_configured'),
      createCommandSessionConfigured = library
          .lookupFunction<
            _CreateCommandSessionConfiguredNative,
            _CreateCommandSessionConfiguredDart
          >('nauterm_session_create_command_configured'),
      createSshSessionConfigured = library
          .lookupFunction<
            _CreateSshSessionConfiguredNative,
            _CreateSshSessionConfiguredDart
          >('nauterm_session_create_ssh_configured'),
      createMoshSessionConfigured = library
          .lookupFunction<
            _CreateMoshSessionConfiguredNative,
            _CreateMoshSessionConfiguredDart
          >('nauterm_session_create_mosh_configured'),
      reconnectSshSession = library
          .lookupFunction<_ReconnectSshSessionNative, _ReconnectSshSessionDart>(
            'nauterm_session_reconnect_ssh',
          ),
      reconnectMoshSession = library
          .lookupFunction<
            _ReconnectMoshSessionNative,
            _ReconnectMoshSessionDart
          >('nauterm_session_reconnect_mosh'),
      createTelnetSessionConfigured = library
          .lookupFunction<
            _CreateTelnetSessionConfiguredNative,
            _CreateTelnetSessionConfiguredDart
          >('nauterm_session_create_telnet_configured'),
      createSerialSessionConfigured = library
          .lookupFunction<
            _CreateSerialSessionConfiguredNative,
            _CreateSerialSessionConfiguredDart
          >('nauterm_session_create_serial_configured'),
      close = library.lookupFunction<_CloseSessionNative, _CloseSessionDart>(
        'nauterm_session_close',
      ),
      initializeRuntime = library
          .lookupFunction<_RuntimeLifecycleNative, _RuntimeLifecycleDart>(
            'nauterm_runtime_initialize',
          ),
      prepareRuntimeShutdown = library
          .lookupFunction<_RuntimeLifecycleNative, _RuntimeLifecycleDart>(
            'nauterm_runtime_prepare_shutdown',
          ),
      writeShellIntegrationResources = library
          .lookupFunction<
            _WriteShellIntegrationResourcesNative,
            _WriteShellIntegrationResourcesDart
          >('nauterm_shell_integration_write_resources'),
      capturePrepareDirectory = library
          .lookupFunction<
            _CapturePrepareDirectoryNative,
            _CapturePrepareDirectoryDart
          >('nauterm_capture_prepare_directory'),
      captureWriterOpen = library
          .lookupFunction<_CaptureOpenNative, _CaptureOpenDart>(
            'nauterm_capture_writer_open',
          ),
      captureWriterAppend = library
          .lookupFunction<_CaptureAppendNative, _CaptureAppendDart>(
            'nauterm_capture_writer_append',
          ),
      captureWriterFlush = library
          .lookupFunction<_CaptureHandleBoolNative, _CaptureHandleBoolDart>(
            'nauterm_capture_writer_flush',
          ),
      captureWriterCheckpoint = library
          .lookupFunction<_CaptureHandleJsonNative, _CaptureHandleJsonDart>(
            'nauterm_capture_writer_checkpoint',
          ),
      captureWriterFinalize = library
          .lookupFunction<_CaptureHandleJsonNative, _CaptureHandleJsonDart>(
            'nauterm_capture_writer_finalize',
          ),
      captureWriterClose = library
          .lookupFunction<_CaptureHandleBoolNative, _CaptureHandleBoolDart>(
            'nauterm_capture_writer_close',
          ),
      captureWriterAbort = library
          .lookupFunction<_CaptureHandleBoolNative, _CaptureHandleBoolDart>(
            'nauterm_capture_writer_abort',
          ),
      captureReaderOpen = library
          .lookupFunction<_CaptureOpenNative, _CaptureOpenDart>(
            'nauterm_capture_reader_open',
          ),
      captureVerifyComplete = library
          .lookupFunction<_CaptureOpenBoolNative, _CaptureOpenBoolDart>(
            'nauterm_capture_verify_complete',
          ),
      captureRecover = library
          .lookupFunction<_CaptureRecoverNative, _CaptureRecoverDart>(
            'nauterm_capture_recover',
          ),
      captureReaderNext = library
          .lookupFunction<_CaptureReaderNextNative, _CaptureReaderNextDart>(
            'nauterm_capture_reader_next',
          ),
      captureReaderClose = library
          .lookupFunction<_CaptureReaderCloseNative, _CaptureReaderCloseDart>(
            'nauterm_capture_reader_close',
          ),
      destroyTerminal = library
          .lookupFunction<_DestroyTerminalNative, _DestroyTerminalDart>(
            'nauterm_terminal_destroy',
          ),
      resize = library.lookupFunction<_ResizeSessionNative, _ResizeSessionDart>(
        'nauterm_session_resize',
      ),
      notifyNetworkChanged = library
          .lookupFunction<
            _NotifyNetworkChangedNative,
            _NotifyNetworkChangedDart
          >('nauterm_session_notify_network_changed'),
      exitAlternateScreen = library
          .lookupFunction<_ExitAlternateScreenNative, _ExitAlternateScreenDart>(
            'nauterm_session_exit_alternate_screen',
          ),
      bellCount = library.lookupFunction<_BellCountNative, _BellCountDart>(
        'nauterm_session_bell_count',
      ),
      clipboard = library.lookupFunction<_ClipboardNative, _ClipboardDart>(
        'nauterm_session_clipboard',
      ),
      terminalResize = library
          .lookupFunction<_TerminalResizeNative, _TerminalResizeDart>(
            'nauterm_terminal_resize',
          ),
      scrollLines = library
          .lookupFunction<_ScrollLinesNative, _ScrollLinesDart>(
            'nauterm_session_scroll_lines',
          ),
      terminalScrollLines = library
          .lookupFunction<_TerminalScrollLinesNative, _TerminalScrollLinesDart>(
            'nauterm_terminal_scroll_lines',
          ),
      scrollPageUp = library.lookupFunction<_ScrollPageNative, _ScrollPageDart>(
        'nauterm_session_scroll_page_up',
      ),
      terminalScrollPageUp = library
          .lookupFunction<_TerminalScrollPageNative, _TerminalScrollPageDart>(
            'nauterm_terminal_scroll_page_up',
          ),
      scrollPageDown = library
          .lookupFunction<_ScrollPageNative, _ScrollPageDart>(
            'nauterm_session_scroll_page_down',
          ),
      terminalScrollPageDown = library
          .lookupFunction<_TerminalScrollPageNative, _TerminalScrollPageDart>(
            'nauterm_terminal_scroll_page_down',
          ),
      scrollToBottom = library
          .lookupFunction<_ScrollPageNative, _ScrollPageDart>(
            'nauterm_session_scroll_to_bottom',
          ),
      search = library.lookupFunction<_SearchSessionNative, _SearchSessionDart>(
        'nauterm_session_search',
      ),
      terminalSearch = library
          .lookupFunction<_TerminalSearchNative, _TerminalSearchDart>(
            'nauterm_terminal_search',
          ),
      selectionText = library
          .lookupFunction<
            _SelectionTextSessionNative,
            _SelectionTextSessionDart
          >('nauterm_session_selection_text'),
      terminalSelectionText = library
          .lookupFunction<
            _TerminalSelectionTextNative,
            _TerminalSelectionTextDart
          >('nauterm_terminal_selection_text'),
      commandBlockAt = library
          .lookupFunction<_CommandBlockSessionNative, _CommandBlockSessionDart>(
            'nauterm_session_command_block_at',
          ),
      terminalCommandBlockAt = library
          .lookupFunction<
            _TerminalCommandBlockNative,
            _TerminalCommandBlockDart
          >('nauterm_terminal_command_block_at'),
      promptClickMove = library
          .lookupFunction<
            _PromptClickMoveSessionNative,
            _PromptClickMoveSessionDart
          >('nauterm_session_prompt_click_move'),
      terminalPromptClickMove = library
          .lookupFunction<
            _TerminalPromptClickMoveNative,
            _TerminalPromptClickMoveDart
          >('nauterm_terminal_prompt_click_move'),
      terminalPlainText = library
          .lookupFunction<_TerminalPlainTextNative, _TerminalPlainTextDart>(
            'nauterm_terminal_plain_text',
          ),
      setWakeupCallback = library
          .lookupFunction<_SetWakeupNative, _SetWakeupDart>(
            'nauterm_session_set_wakeup_callback',
          ),
      poll = library.lookupFunction<_PollSessionNative, _PollSessionDart>(
        'nauterm_session_poll',
      ),
      isExited = library
          .lookupFunction<_IsExitedSessionNative, _IsExitedSessionDart>(
            'nauterm_session_is_exited',
          ),
      writeCodepoint = library
          .lookupFunction<_WriteCodepointNative, _WriteCodepointDart>(
            'nauterm_session_write_codepoint',
          ),
      writeSessionBytes = library
          .lookupFunction<_WriteSessionBytesNative, _WriteSessionBytesDart>(
            'nauterm_session_write_bytes',
          ),
      terminalWriteCodepoint = library
          .lookupFunction<
            _TerminalWriteCodepointNative,
            _TerminalWriteCodepointDart
          >('nauterm_terminal_write_codepoint'),
      terminalWriteBytes = library
          .lookupFunction<_TerminalWriteBytesNative, _TerminalWriteBytesDart>(
            'nauterm_terminal_write_bytes',
          ),
      sendInputCodepoint = library
          .lookupFunction<_SendInputCodepointNative, _SendInputCodepointDart>(
            'nauterm_session_send_input_codepoint',
          ),
      sendInputBytesStatus = library
          .lookupFunction<
            _SendInputBytesStatusNative,
            _SendInputBytesStatusDart
          >('nauterm_session_send_input_bytes_status'),
      snapshot = library.lookupFunction<_SnapshotNative, _SnapshotDart>(
        'nauterm_session_snapshot',
      ),
      terminalSnapshot = library
          .lookupFunction<_TerminalSnapshotNative, _TerminalSnapshotDart>(
            'nauterm_terminal_snapshot',
          ),
      freeSnapshot = library
          .lookupFunction<_FreeSnapshotNative, _FreeSnapshotDart>(
            'nauterm_terminal_free_snapshot',
          ),
      drainConnectionEvents = library
          .lookupFunction<
            _DrainConnectionEventsNative,
            _DrainConnectionEventsDart
          >('nauterm_session_drain_events'),
      drainOutputCapture = library
          .lookupFunction<_DrainOutputCaptureNative, _DrainOutputCaptureDart>(
            'nauterm_session_drain_output_capture',
          ),
      readSessionShellHistory = library
          .lookupFunction<
            _ReadSessionShellHistoryNative,
            _ReadSessionShellHistoryDart
          >('nauterm_session_read_shell_history'),
      suppressOutputUntil = library
          .lookupFunction<_SuppressOutputUntilNative, _SuppressOutputUntilDart>(
            'nauterm_session_suppress_output_until',
          ),
      cancelOutputSuppression = library
          .lookupFunction<
            _CancelOutputSuppressionNative,
            _CancelOutputSuppressionDart
          >('nauterm_session_cancel_output_suppression'),
      listSerialPorts = library
          .lookupFunction<_ListSerialPortsNative, _ListSerialPortsDart>(
            'nauterm_serial_list_ports',
          ),
      fido2ListDevices = library
          .lookupFunction<_Fido2ListDevicesNative, _Fido2ListDevicesDart>(
            'nauterm_fido2_list_devices',
          ),
      fido2VerifyPin = library
          .lookupFunction<_Fido2VerifyPinNative, _Fido2VerifyPinDart>(
            'nauterm_fido2_verify_pin',
          ),
      fido2Generate = library
          .lookupFunction<_Fido2GenerateNative, _Fido2GenerateDart>(
            'nauterm_fido2_generate',
          ),
      startPortForward = library
          .lookupFunction<_StartPortForwardNative, _StartPortForwardDart>(
            'nauterm_port_forward_start',
          ),
      stopPortForward = library
          .lookupFunction<_StopPortForwardNative, _StopPortForwardDart>(
            'nauterm_port_forward_stop',
          ),
      stopAllPortForwards = library
          .lookupFunction<_StopAllPortForwardsNative, _StopAllPortForwardsDart>(
            'nauterm_port_forward_stop_all',
          ),
      portForwardStatus = library
          .lookupFunction<_PortForwardStatusNative, _PortForwardStatusDart>(
            'nauterm_port_forward_status',
          ),
      sshListDirectories = library
          .lookupFunction<_SshListDirectoriesNative, _SshListDirectoriesDart>(
            'nauterm_ssh_list_directories',
          ),
      sshListDirectoryEntries = library
          .lookupFunction<
            _SshListDirectoryEntriesNative,
            _SshListDirectoryEntriesDart
          >('nauterm_ssh_list_directory_entries'),
      sftpListDirectoryEntries = library
          .lookupFunction<
            _SftpListDirectoryEntriesNative,
            _SftpListDirectoryEntriesDart
          >('nauterm_sftp_list_directory_entries'),
      sftpExecuteTask = library
          .lookupFunction<_SftpExecuteTaskNative, _SftpExecuteTaskDart>(
            'nauterm_sftp_execute_task',
          ),
      sftpCancelTask = library
          .lookupFunction<_SftpCancelTaskNative, _SftpCancelTaskDart>(
            'nauterm_sftp_cancel_task',
          ),
      sftpCloseSudoSession = library
          .lookupFunction<
            _SftpCloseSudoSessionNative,
            _SftpCloseSudoSessionDart
          >('nauterm_sftp_close_sudo_session'),
      freeString = library.lookupFunction<_FreeStringNative, _FreeStringDart>(
        'nauterm_string_free',
      );

  factory _TerminalBindings.open() {
    final errors = <String>[];
    final candidates = _libraryCandidates();
    for (var index = 0; index < candidates.length; index += 1) {
      final candidate = candidates[index];
      try {
        final library = DynamicLibrary.open(candidate);
        final abiVersion = library
            .lookupFunction<Uint32 Function(), int Function()>(
              'nauterm_ffi_abi_version',
            )();
        if (abiVersion != _expectedAbiVersion) {
          throw StateError(
            'nauterm_ffi ABI version mismatch: expected '
            '$_expectedAbiVersion, got $abiVersion',
          );
        }
        final bindings = _TerminalBindings(library);
        if (!_loadLogged) {
          _loadLogged = true;
          NautermLog.info(
            'native',
            'Terminal native library loaded.',
            fields: {'candidate_index': index, 'abi_version': abiVersion},
          );
        }
        return bindings;
      } on Object catch (error) {
        errors.add('$candidate: $error');
      }
    }

    NautermLog.error(
      'native',
      'Terminal native library could not be loaded.',
      fields: {'attempt_count': candidates.length},
    );

    throw FfiTerminalLoadException(
      'Unable to load nauterm_ffi native library.\n${errors.join('\n')}',
    );
  }

  final DynamicLibrary library;
  static bool _loadLogged = false;
  final _CreateLocalSessionConfiguredDart createLocalSessionConfigured;
  final _CreateCommandSessionConfiguredDart createCommandSessionConfigured;
  final _CreateTerminalConfiguredDart createTerminalConfigured;
  final _CreateSshSessionConfiguredDart createSshSessionConfigured;
  final _CreateMoshSessionConfiguredDart createMoshSessionConfigured;
  final _ReconnectSshSessionDart reconnectSshSession;
  final _ReconnectMoshSessionDart reconnectMoshSession;
  final _CreateTelnetSessionConfiguredDart createTelnetSessionConfigured;
  final _CreateSerialSessionConfiguredDart createSerialSessionConfigured;
  final _CloseSessionDart close;
  final _RuntimeLifecycleDart initializeRuntime;
  final _RuntimeLifecycleDart prepareRuntimeShutdown;
  final _WriteShellIntegrationResourcesDart writeShellIntegrationResources;
  final _CapturePrepareDirectoryDart capturePrepareDirectory;
  final _CaptureOpenDart captureWriterOpen;
  final _CaptureAppendDart captureWriterAppend;
  final _CaptureHandleBoolDart captureWriterFlush;
  final _CaptureHandleJsonDart captureWriterCheckpoint;
  final _CaptureHandleJsonDart captureWriterFinalize;
  final _CaptureHandleBoolDart captureWriterClose;
  final _CaptureHandleBoolDart captureWriterAbort;
  final _CaptureOpenDart captureReaderOpen;
  final _CaptureOpenBoolDart captureVerifyComplete;
  final _CaptureRecoverDart captureRecover;
  final _CaptureReaderNextDart captureReaderNext;
  final _CaptureReaderCloseDart captureReaderClose;
  final _DestroyTerminalDart destroyTerminal;
  final _ResizeSessionDart resize;
  final _NotifyNetworkChangedDart notifyNetworkChanged;
  final _ExitAlternateScreenDart exitAlternateScreen;
  final _BellCountDart bellCount;
  final _ClipboardDart clipboard;
  final _TerminalResizeDart terminalResize;
  final _ScrollLinesDart scrollLines;
  final _TerminalScrollLinesDart terminalScrollLines;
  final _ScrollPageDart scrollPageUp;
  final _TerminalScrollPageDart terminalScrollPageUp;
  final _ScrollPageDart scrollPageDown;
  final _TerminalScrollPageDart terminalScrollPageDown;
  final _ScrollPageDart scrollToBottom;
  final _SearchSessionDart search;
  final _TerminalSearchDart terminalSearch;
  final _SelectionTextSessionDart selectionText;
  final _TerminalSelectionTextDart terminalSelectionText;
  final _CommandBlockSessionDart commandBlockAt;
  final _TerminalCommandBlockDart terminalCommandBlockAt;
  final _PromptClickMoveSessionDart promptClickMove;
  final _TerminalPromptClickMoveDart terminalPromptClickMove;
  final _TerminalPlainTextDart terminalPlainText;
  final _SetWakeupDart setWakeupCallback;
  final _PollSessionDart poll;
  final _IsExitedSessionDart isExited;
  final _WriteCodepointDart writeCodepoint;
  final _WriteSessionBytesDart writeSessionBytes;
  final _TerminalWriteCodepointDart terminalWriteCodepoint;
  final _TerminalWriteBytesDart terminalWriteBytes;
  final _SendInputCodepointDart sendInputCodepoint;
  final _SendInputBytesStatusDart sendInputBytesStatus;
  final _SnapshotDart snapshot;
  final _TerminalSnapshotDart terminalSnapshot;
  final _FreeSnapshotDart freeSnapshot;
  final _DrainConnectionEventsDart drainConnectionEvents;
  final _DrainOutputCaptureDart drainOutputCapture;
  final _ReadSessionShellHistoryDart readSessionShellHistory;
  final _SuppressOutputUntilDart suppressOutputUntil;
  final _CancelOutputSuppressionDart cancelOutputSuppression;
  final _ListSerialPortsDart listSerialPorts;
  final _Fido2ListDevicesDart fido2ListDevices;
  final _Fido2VerifyPinDart fido2VerifyPin;
  final _Fido2GenerateDart fido2Generate;
  final _StartPortForwardDart startPortForward;
  final _StopPortForwardDart stopPortForward;
  final _StopAllPortForwardsDart stopAllPortForwards;
  final _PortForwardStatusDart portForwardStatus;
  final _SshListDirectoriesDart sshListDirectories;
  final _SshListDirectoryEntriesDart sshListDirectoryEntries;
  final _SftpListDirectoryEntriesDart sftpListDirectoryEntries;
  final _SftpExecuteTaskDart sftpExecuteTask;
  final _SftpCancelTaskDart sftpCancelTask;
  final _SftpCloseSudoSessionDart sftpCloseSudoSession;
  final _FreeStringDart freeString;
}

void _withNativeBytes(
  Uint8List bytes,
  void Function(Pointer<Uint8> pointer, int length) work,
) {
  final pointer = malloc<Uint8>(bytes.length);
  try {
    pointer.asTypedList(bytes.length).setAll(0, bytes);
    work(pointer, bytes.length);
  } finally {
    malloc.free(pointer);
  }
}

T _withNativeBytesResult<T>(
  Uint8List bytes,
  T Function(Pointer<Uint8> pointer, int length) work,
) {
  final pointer = malloc<Uint8>(bytes.length);
  try {
    pointer.asTypedList(bytes.length).setAll(0, bytes);
    return work(pointer, bytes.length);
  } finally {
    malloc.free(pointer);
  }
}

TerminalSearchResult _searchResultFromNative(
  _TerminalBindings bindings,
  Pointer<Utf8> resultPointer,
) {
  if (resultPointer == nullptr) {
    return const TerminalSearchResult.notFound();
  }

  try {
    final decoded = jsonDecode(resultPointer.toDartString());
    if (decoded is! Map<Object?, Object?>) {
      return const TerminalSearchResult.notFound();
    }
    final result = decoded.cast<String, Object?>();
    final error = result['error'] as String?;
    if (result['found'] != true) {
      return TerminalSearchResult.notFound(error: error);
    }

    final columns = result['columns'] as int? ?? 0;
    final rows = result['rows'] as int? ?? 0;
    if (columns <= 0 || rows <= 0) {
      return TerminalSearchResult.notFound(error: error);
    }

    final startRow = (result['start_row'] as int? ?? 0)
        .clamp(0, rows - 1)
        .toInt();
    final startColumn = (result['start_column'] as int? ?? 0)
        .clamp(0, columns - 1)
        .toInt();
    final endRow = (result['end_row'] as int? ?? startRow)
        .clamp(0, rows - 1)
        .toInt();
    final endColumn = (result['end_column'] as int? ?? startColumn + 1)
        .clamp(0, columns)
        .toInt();
    final start = startRow * columns + startColumn;
    final end = endRow * columns + endColumn;
    if (start >= end) {
      return TerminalSearchResult.notFound(error: error);
    }

    return TerminalSearchResult(
      selection: TerminalSelection(start: start, end: end),
      error: error,
    );
  } on Object catch (error) {
    return TerminalSearchResult.notFound(error: error.toString());
  } finally {
    bindings.freeString(resultPointer);
  }
}

int _searchDirectionToNative(TerminalSearchDirection direction) {
  return switch (direction) {
    TerminalSearchDirection.next => 0,
    TerminalSearchDirection.previous => 1,
  };
}

extension on TerminalSearchResult {
  TerminalSearchResult relativeToSnapshot(TerminalSnapshot snapshot) {
    final currentSelection = selection;
    if (currentSelection == null || snapshot.displayOffset == 0) {
      return this;
    }
    return TerminalSearchResult(
      selection: currentSelection.shift(
        -snapshot.displayOffset * snapshot.columns,
      ),
      error: error,
    );
  }
}

TerminalCommandBlock? _commandBlockFromNative(
  _TerminalBindings bindings,
  Pointer<Utf8> pointer,
) {
  if (pointer == nullptr) {
    return null;
  }
  try {
    final decoded = jsonDecode(pointer.toDartString());
    if (decoded is! Map<Object?, Object?>) {
      return null;
    }
    final start = decoded['start'];
    final end = decoded['end'];
    if (start is! int || end is! int || start >= end) {
      return null;
    }
    final workingDirectory = decoded['working_directory'];
    final id = decoded['id'];
    final inputStart = decoded['input_start'];
    final command = decoded['command'];
    final exitCode = decoded['exit_code'];
    final completed = decoded['completed'];
    final shellIntegrated = decoded['shell_integrated'];
    return TerminalCommandBlock(
      selection: TerminalSelection(start: start, end: end),
      id: id is int && id > 0 ? id : null,
      inputStart: inputStart is int ? inputStart : null,
      workingDirectory:
          workingDirectory is String && workingDirectory.isNotEmpty
          ? workingDirectory
          : null,
      command: command is String && command.isNotEmpty ? command : null,
      exitCode: exitCode is int ? exitCode : null,
      completed: completed == true,
      shellIntegrated: shellIntegrated == true,
    );
  } on Object {
    return null;
  } finally {
    bindings.freeString(pointer);
  }
}

TerminalPromptClickMove? _promptClickMoveFromNative(
  _TerminalBindings bindings,
  Pointer<Utf8> pointer,
) {
  if (pointer == nullptr) return null;
  try {
    final decoded = jsonDecode(pointer.toDartString());
    if (decoded is! Map<Object?, Object?>) return null;
    final left = decoded['left'];
    final right = decoded['right'];
    if (left is! int || right is! int || left < 0 || right < 0) return null;
    return TerminalPromptClickMove(left: left, right: right);
  } on Object {
    return null;
  } finally {
    bindings.freeString(pointer);
  }
}

TerminalSnapshot _snapshotFromNative(_NativeTerminalSnapshot native) {
  final titleBytes = native.titleLength == 0
      ? Uint8List(0)
      : native.title.asTypedList(native.titleLength);
  final title = titleBytes.isEmpty
      ? ''
      : utf8.decode(titleBytes, allowMalformed: true);
  final clipboardBytes = native.clipboardLength == 0
      ? Uint8List(0)
      : native.clipboard.asTypedList(native.clipboardLength);
  final clipboard = clipboardBytes.isEmpty
      ? ''
      : utf8.decode(clipboardBytes, allowMalformed: true);
  final textBytes = native.textLength == 0
      ? Uint8List(0)
      : native.text.asTypedList(native.textLength);
  final hyperlinkBytes = native.hyperlinkTextLength == 0
      ? Uint8List(0)
      : native.hyperlinkText.asTypedList(native.hyperlinkTextLength);
  final cells = List<TerminalCell>.generate(native.cellsLength, (index) {
    final cell = (native.cells + index).ref;
    final start = cell.textOffset;
    final end = start + cell.textLength;
    final hyperlinkStart = cell.hyperlinkOffset;
    final hyperlinkEnd = hyperlinkStart + cell.hyperlinkLength;
    final text = start >= 0 && end <= textBytes.length && end >= start
        ? utf8.decode(Uint8List.sublistView(textBytes, start, end))
        : ' ';
    final hyperlink =
        hyperlinkStart >= 0 &&
            hyperlinkEnd <= hyperlinkBytes.length &&
            hyperlinkEnd >= hyperlinkStart
        ? utf8.decode(
            Uint8List.sublistView(hyperlinkBytes, hyperlinkStart, hyperlinkEnd),
            allowMalformed: true,
          )
        : '';

    return TerminalCell(
      text: text.isEmpty ? ' ' : text,
      foreground: _colorFromRgb(cell.foreground),
      background: _colorFromRgb(cell.background),
      flags: cell.flags,
      hyperlink: hyperlink,
    );
  }, growable: false);
  final graphics = _graphicsFromNative(native);

  return TerminalSnapshot(
    emulatorBackend: _emulatorBackendFromNative(native.emulatorBackend),
    graphicImages: graphics.$1,
    graphicPlacements: graphics.$2,
    columns: native.columns,
    rows: native.rows,
    historyLines: native.historyLines,
    displayOffset: native.displayOffset,
    title: title,
    clipboardText: clipboard,
    bellCount: native.bellCount,
    cells: cells,
    cursor: TerminalCursor(
      column: native.cursorColumn,
      row: native.cursorRow,
      visible: native.cursorVisible != 0,
      shape: _cursorShapeFromNative(native.cursorShape),
      color: _colorFromRgb(native.cursorColor),
      blinking: native.cursorBlinking != 0,
    ),
    keyboardMode: _keyboardModeFromNative(native.keyboardMode),
    inputEchoEnabled: native.inputEchoEnabled != 0,
    alternateScreen: native.alternateScreen != 0,
  );
}

(List<TerminalGraphicImage>, List<TerminalGraphicPlacement>)
_graphicsFromNative(_NativeTerminalSnapshot native) {
  final data = native.graphicDataLength == 0
      ? Uint8List(0)
      : native.graphicData.asTypedList(native.graphicDataLength);
  final images = List<TerminalGraphicImage>.generate(
    native.graphicImagesLength,
    (index) {
      final image = (native.graphicImages + index).ref;
      final start = image.dataOffset;
      final end = start + image.dataLength;
      final rgba = start >= 0 && end >= start && end <= data.length
          ? Uint8List.fromList(Uint8List.sublistView(data, start, end))
          : Uint8List(0);
      return TerminalGraphicImage(
        id: image.id,
        generation: image.generation,
        width: image.width,
        height: image.height,
        rgba: rgba,
      );
    },
    growable: false,
  );
  final placements = List<TerminalGraphicPlacement>.generate(
    native.graphicPlacementsLength,
    (index) {
      final placement = (native.graphicPlacements + index).ref;
      return TerminalGraphicPlacement(
        imageId: placement.imageId,
        placementId: placement.placementId,
        zIndex: placement.zIndex,
        viewportColumn: placement.viewportColumn,
        viewportRow: placement.viewportRow,
        columns: placement.columns,
        rows: placement.rows,
        sourceX: placement.sourceX,
        sourceY: placement.sourceY,
        sourceWidth: placement.sourceWidth,
        sourceHeight: placement.sourceHeight,
      );
    },
    growable: false,
  );
  return (images, placements);
}

TerminalEmulatorBackend _emulatorBackendFromNative(int value) =>
    value >= 0 && value < TerminalEmulatorBackend.values.length
    ? TerminalEmulatorBackend.values[value]
    : TerminalEmulatorBackend.alacritty;

Uint8List _hexDecode(String value) {
  final length = value.length;
  if (length.isOdd) {
    return Uint8List(0);
  }
  final bytes = Uint8List(length ~/ 2);
  for (var i = 0; i < length; i += 2) {
    final high = _hexValue(value.codeUnitAt(i));
    final low = _hexValue(value.codeUnitAt(i + 1));
    if (high < 0 || low < 0) {
      return Uint8List(0);
    }
    bytes[i ~/ 2] = (high << 4) | low;
  }
  return bytes;
}

int _hexValue(int codeUnit) {
  if (codeUnit >= 0x30 && codeUnit <= 0x39) {
    return codeUnit - 0x30;
  }
  if (codeUnit >= 0x61 && codeUnit <= 0x66) {
    return codeUnit - 0x61 + 10;
  }
  if (codeUnit >= 0x41 && codeUnit <= 0x46) {
    return codeUnit - 0x41 + 10;
  }
  return -1;
}

Color _colorFromRgb(int rgb) {
  return Color(0xff000000 | (rgb & 0x00ffffff));
}

int _colorToNativeRgb(Color color) {
  return color.toARGB32() & 0x00ffffff;
}

TerminalCursorShape _cursorShapeFromNative(int shape) {
  return switch (shape) {
    1 => TerminalCursorShape.underline,
    2 => TerminalCursorShape.beam,
    3 => TerminalCursorShape.hollowBlock,
    _ => TerminalCursorShape.block,
  };
}

TerminalKeyboardMode _keyboardModeFromNative(int mode) {
  return TerminalKeyboardMode(
    applicationCursor: mode & 0x01 != 0,
    applicationKeypad: mode & 0x02 != 0,
    bracketedPaste: mode & 0x04 != 0,
    focusEvents: mode & 0x08 != 0,
    mouseReportClick: mode & 0x10 != 0,
    mouseDrag: mode & 0x20 != 0,
    mouseMotion: mode & 0x40 != 0,
    sgrMouse: mode & 0x80 != 0,
  );
}

int _cursorShapeToNative(TerminalCursorShape shape) {
  return switch (shape) {
    TerminalCursorShape.block => 0,
    TerminalCursorShape.underline => 1,
    TerminalCursorShape.beam => 2,
    TerminalCursorShape.hollowBlock => 3,
  };
}

int _colorTermToNative(TerminalColorTerm colorTerm) {
  return switch (colorTerm) {
    TerminalColorTerm.none => 0,
    TerminalColorTerm.truecolor => 1,
  };
}

int _osc52ModeToNative(TerminalOsc52Mode mode) {
  return mode == TerminalOsc52Mode.copyAndPaste ? 1 : 0;
}

int _serialParityToNative(SerialParity parity) {
  return switch (parity) {
    SerialParity.none => 0,
    SerialParity.even => 1,
    SerialParity.odd => 2,
  };
}

int _serialFlowControlToNative(SerialFlowControl flowControl) {
  return switch (flowControl) {
    SerialFlowControl.none => 0,
    SerialFlowControl.software => 1,
    SerialFlowControl.hardware => 2,
  };
}

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
