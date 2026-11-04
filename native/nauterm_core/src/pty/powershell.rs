use super::*;
use base64::Engine;

pub(crate) const SCRIPT: &str = include_str!("powershell.ps1");

pub(super) fn configure(
    options: &mut Options,
    shell_path: &str,
) -> io::Result<Option<ShellIntegrationFiles>> {
    let Some(program) = resolve_shell_program(shell_path) else {
        return Ok(None);
    };
    let files = ShellIntegrationFiles {
        root: create_shell_integration_root()?,
    };
    let script = write_shell_integration_file(&files.root, "nauterm.ps1", SCRIPT)?;
    let path = script.to_str().ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidData,
            "PowerShell integration requires a Unicode script path",
        )
    })?;
    // Supply our trusted startup code as a command, preserving the user's
    // normal profile loading and execution-policy configuration. UTF-16LE is
    // required by both Windows PowerShell 5.1 and PowerShell 7 EncodedCommand.
    let bootstrap = format!(
        ". ([scriptblock]::Create([IO.File]::ReadAllText('{}')))",
        path.replace('\'', "''")
    );
    let encoded = encoded_command(&bootstrap);
    options.shell = Some(Shell::new(
        program,
        vec![
            "-NoLogo".into(),
            "-NoExit".into(),
            "-EncodedCommand".into(),
            encoded,
        ],
    ));
    Ok(Some(files))
}

pub(crate) fn encoded_command(command: &str) -> String {
    let bytes: Vec<_> = command.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_preserves_profile_loading_and_quotes_unicode_script_paths() {
        let root = tempfile::tempdir().unwrap();
        let shell = root.path().join("powershell.exe");
        std::fs::write(&shell, []).unwrap();
        let mut options = Options::default();
        let files = configure(&mut options, shell.to_str().unwrap())
            .unwrap()
            .unwrap();
        let expected = Shell::new(
            shell.to_string_lossy().into(),
            vec![
                "-NoLogo".into(),
                "-NoExit".into(),
                "-EncodedCommand".into(),
                encoded_command(&format!(
                    ". ([scriptblock]::Create([IO.File]::ReadAllText('{}')))",
                    files
                        .root
                        .join("nauterm.ps1")
                        .to_str()
                        .unwrap()
                        .replace('\'', "''")
                )),
            ],
        );
        assert_eq!(options.shell, Some(expected));
        assert_eq!(
            std::fs::read_to_string(files.root.join("nauterm.ps1")).unwrap(),
            SCRIPT
        );
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded_command("用户's $path"))
            .unwrap();
        let utf16: Vec<_> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        assert_eq!(String::from_utf16(&utf16).unwrap(), "用户's $path");
        let path = files.root.clone();
        drop(files);
        assert!(!path.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_powershell_reports_command_boundaries_directory_and_ai_completion() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("workspace 用户's #100%");
        std::fs::create_dir(&directory).unwrap();
        let script = format!(
            "$env:NAUTERM_SHELL_INTEGRATION_TOKEN='fixture-token'; function global:prompt {{ 'fixture> ' }}\n{}\n{}",
            SCRIPT, include_str!("powershell_test.ps1")
        );
        let output = std::process::Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-EncodedCommand",
                &encoded_command(&script),
            ])
            .current_dir(directory)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("NAUTERM_POWERSHELL_PROTOCOL_OK"));
    }
}
