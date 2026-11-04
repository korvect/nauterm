//! Exercise the application's actual PTY worker/startup path, in addition to
//! the independent ConPTY protocol fixture.
use super::*;
use std::time::{Duration, Instant};

fn await_output(pty: &mut LocalPty, bytes: &mut Vec<u8>, marker: &[u8]) {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut queries = bytes.windows(4).filter(|part| *part == b"\x1b[6n").count();
    loop {
        let pump = pty.drain_output();
        bytes.extend(pump.output);
        let total = bytes.windows(4).filter(|part| *part == b"\x1b[6n").count();
        for _ in queries..total {
            pty.queue_input(b"\x1b[1;1R");
        }
        queries = total;
        if bytes.windows(marker.len()).any(|part| part == marker) {
            return;
        }
        assert!(
            !pump.exited,
            "Shell exited before marker: {}",
            String::from_utf8_lossy(bytes)
        );
        assert!(
            Instant::now() < deadline,
            "Missing marker {:?}: {}",
            marker,
            String::from_utf8_lossy(bytes)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn cmd_native_worker_reports_commands_and_exits() {
    exercise(std::env::var("COMSPEC").expect("Windows CMD"), false);
}

#[test]
fn git_bash_native_worker_tracks_unicode_directory_resize_and_exit() {
    let candidates = [
        std::env::var_os("ProgramFiles").map(|root| PathBuf::from(root).join("Git/bin/bash.exe")),
        std::env::var_os("LOCALAPPDATA")
            .map(|root| PathBuf::from(root).join("Programs/Git/bin/bash.exe")),
    ];
    let bash = candidates
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
        .or_else(|| resolve_shell_program("bash.exe").map(PathBuf::from))
        .expect("Install Git for Windows for this opt-in integration test");
    exercise(bash.to_string_lossy().into_owned(), true);
}

fn exercise(shell: String, bash: bool) {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("workspace 用户 & more");
    std::fs::create_dir(&directory).unwrap();
    let vars = [
        ("HOME", root.path().to_string_lossy().into_owned()),
        (
            "HISTFILE",
            root.path()
                .join("fixture-history")
                .to_string_lossy()
                .into_owned(),
        ),
        (
            "NAUTERM_SHELL_INTEGRATION_TOKEN",
            "native-worker-fixture".into(),
        ),
    ];
    let options = TerminalOptions {
        shell_path: Some(shell),
        working_directory: Some(directory),
        environment: vars
            .into_iter()
            .map(
                |(variable, value)| crate::terminal::TerminalEnvironmentVariable {
                    variable: variable.into(),
                    value,
                },
            )
            .collect(),
        ..TerminalOptions::default()
    };
    let mut pty = LocalPty::spawn(
        TerminalGeometry {
            columns: 100,
            rows: 30,
        },
        &options,
    )
    .unwrap();
    let mut output = vec![];
    await_output(&mut pty, &mut output, b"]133;B;aid=");
    assert!(String::from_utf8_lossy(&output).contains("]7;file://"));
    pty.queue_input(if bash {
        b"printf 'NATIVE_WORKER_OK\\n'\r"
    } else {
        b"echo NATIVE_WORKER_OK\r"
    });
    let mut command = vec![];
    await_output(&mut pty, &mut command, b"]4545;CommandExited;");
    assert!(command
        .windows(b"NATIVE_WORKER_OK".len())
        .any(|part| part == b"NATIVE_WORKER_OK"));
    assert!(command
        .windows(b"]4545;CommandStarted;".len())
        .any(|part| part == b"]4545;CommandStarted;"));
    pty.resize(TerminalGeometry {
        columns: 86,
        rows: 22,
    });
    pty.queue_input(b"echo NATIVE_RESIZED\r");
    await_output(&mut pty, &mut vec![], b"]4545;CommandExited;");
    pty.queue_input(b"exit\r");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if pty.drain_output().exited {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "LocalPty worker did not report shell exit"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
