# Native CMD integration

This first-party crate contains the prompt/output protocol and state model for
a Nauterm-owned Windows console interceptor. It does **not** yet contain an
installed Windows hook or enable CMD tracking in the client. Clink is not a
runtime dependency and no Clink source or binary is included.

The intended host creates only its own CMD child inside Nauterm's ConPTY. The
interceptor recognizes a marked main prompt before treating console reads as
commands, preserving input inside `set /p`, `pause` and interactive programs.
It obtains the exit status using a separate internal probe after execution,
rather than appending shell operators to user input. Probe interception must
preserve a user-defined `ERRORLEVEL` environment variable and must reject
unknown output rather than report success. Prompt writes and command input
must retain their original Unicode/code-page data.

The RS client already provides a per-session integration token to the shell's
environment. Reuse it for AI line-ready/completion acknowledgements; avoid
introducing synthetic setup commands into CMD history. Cooked-input cancellation
must be verified using `CONSOLE_READCONSOLE_CONTROL` before binding an AI key.

Implemented and model-tested:

- Byte/UTF-16 prompt-marker recognition across arbitrary write boundaries.
- Bounded exact prompt capture/replay at console-read boundaries; incomplete
  markers and overflow do not authorize command capture, and redraws preserve
  previously captured output.
- Literal/base64 command transport and validated file-URI directory events.
- Strict, session-specific signed exit-code probes, including Ctrl+C status.
- Command/AI state transitions and cancellation before versus after submission.

Still required before declaring CMD integration supported:

- Windows child host, console API interception and exact prompt replay.
- Preserve profiles, environment, command-line editing and console modes;
  distinguish continuation input from program input without capturing passwords.
- Wire the host into local PTY startup and the AI runner, package its binaries.
- Real ConPTY verification on Windows x64 and arm64, including paths with spaces,
  Unicode, `%`, `!` and `&`, Ctrl+C, multiline commands, resize and process exit.

Run the model checks with `cargo test -p nauterm_cmd_integration --locked`.
These tests do not establish Windows console behavior.
The six-platform GPUI CI matrix and the Windows local validation helper also
run these protocol checks.
