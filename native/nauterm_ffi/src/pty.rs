use std::ffi::c_void;
#[cfg(unix)]
use std::ffi::{CStr, CString};
use std::io::{self, ErrorKind, Read, Write};
use std::num::NonZeroUsize;
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use alacritty_terminal::event::{OnResize, WindowSize};
use alacritty_terminal::tty::{self, EventedPty, EventedReadWrite, Options, Pty, Shell};
use polling::{Event as PollingEvent, Events, PollMode, Poller};

use crate::output_queue::{append_output, clear_output, drain_output_chunk};
use crate::terminal::{TerminalCommand, TerminalGeometry, TerminalOptions};

const CELL_WIDTH_PIXELS: u16 = 8;
const CELL_HEIGHT_PIXELS: u16 = 16;
const PTY_READ_BUFFER_SIZE: usize = 64 * 1024;
const PTY_MAX_READ_BURST_BYTES: usize = 16 * 1024;
#[cfg(windows)]
const PTY_READ_WRITE_TOKEN: usize = 2;
#[cfg(not(windows))]
const PTY_READ_WRITE_TOKEN: usize = 0;
const PTY_CHILD_EVENT_TOKEN: usize = 1;

pub struct LocalPty {
    commands: PtyCommandSender,
    exited: Arc<AtomicBool>,
    input_visible: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    wakeup: WakeupSlot,
    worker: Option<JoinHandle<()>>,
    _shell_integration: Option<ShellIntegrationFiles>,
}

pub struct PtyPump {
    pub output: Vec<u8>,
    pub exited: bool,
    pub has_more: bool,
}

#[derive(Clone, Copy)]
pub struct WakeupCallback {
    callback: extern "C" fn(*mut c_void),
    user_data: usize,
}

type WakeupSlot = Arc<Mutex<Option<WakeupCallback>>>;
static WAKEUP_CALLBACKS_ENABLED: AtomicBool = AtomicBool::new(true);
static SHELL_INTEGRATION_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

enum PtyCommand {
    Input(Vec<u8>),
    Resize(TerminalGeometry),
    Shutdown,
}

struct PtyCommandSender {
    poller: Arc<Poller>,
    sender: Sender<PtyCommand>,
}

impl LocalPty {
    pub fn spawn(
        geometry: TerminalGeometry,
        terminal_options: &TerminalOptions,
    ) -> io::Result<Self> {
        let mut options = Options::default();
        if let Some(command) = &terminal_options.command {
            options.shell = shell_for_command_session(command);
        } else if let Some(shell_path) = &terminal_options.shell_path {
            options.shell = shell_for_local_session(shell_path);
        } else {
            options.shell = default_local_shell();
        }
        options.working_directory =
            resolve_working_directory(terminal_options.working_directory.as_deref());
        options.env.insert(
            "TERM".to_owned(),
            terminal_options.terminal_type.term().to_owned(),
        );
        options
            .env
            .insert("NAUTERM_SESSION".to_owned(), "1".to_owned());
        if let Some(color_term) = terminal_options.color_term.env_value() {
            options
                .env
                .insert("COLORTERM".to_owned(), color_term.to_owned());
        }
        for entry in &terminal_options.environment {
            let variable = entry.variable.trim();
            if variable.is_empty() || variable.contains('=') {
                continue;
            }
            options.env.insert(variable.to_owned(), entry.value.clone());
        }
        let shell_integration = if terminal_options.command.is_none() {
            configure_shell_integration(&mut options, terminal_options.shell_path.as_deref())?
        } else {
            None
        };
        if shell_integration.is_none() {
            configure_history_filtering(&mut options, terminal_options.shell_path.as_deref());
        }

        let pty = tty::new(&options, window_size(geometry), 0)?;
        let poller = Arc::new(Poller::new()?);
        let (sender, receiver) = mpsc::channel();
        let exited = Arc::new(AtomicBool::new(false));
        let input_visible = Arc::new(AtomicBool::new(true));
        let output = Arc::new(Mutex::new(Vec::new()));
        let wakeup = Arc::new(Mutex::new(None));

        let worker = spawn_pty_worker(
            pty,
            poller.clone(),
            receiver,
            exited.clone(),
            input_visible.clone(),
            output.clone(),
            wakeup.clone(),
        )?;

        Ok(Self {
            commands: PtyCommandSender { poller, sender },
            exited,
            input_visible,
            output,
            wakeup,
            worker: Some(worker),
            _shell_integration: shell_integration,
        })
    }

    pub fn resize(&mut self, geometry: TerminalGeometry) {
        self.commands.send(PtyCommand::Resize(geometry));
    }

    pub fn queue_input(&mut self, bytes: &[u8]) {
        self.commands.send(PtyCommand::Input(bytes.to_vec()));
    }

    pub fn set_wakeup_callback(&mut self, callback: Option<WakeupCallback>) {
        if let Ok(mut wakeup) = self.wakeup.lock() {
            *wakeup = callback;
        }
        notify_wakeup(&self.wakeup);
    }

    pub fn input_visible(&self) -> bool {
        self.input_visible.load(Ordering::Acquire)
    }

    pub fn drain_output(&mut self) -> PtyPump {
        let (output, has_more) = drain_output_chunk(&self.output);
        if has_more {
            notify_wakeup(&self.wakeup);
        }
        let exited = self.exited.load(Ordering::Acquire);

        PtyPump {
            output,
            exited,
            has_more,
        }
    }

    pub fn clear_pending_output(&mut self) {
        clear_output(&self.output);
    }
}

fn shell_for_command_session(command: &TerminalCommand) -> Option<Shell> {
    let program = command.program.trim();
    if program.is_empty() {
        return None;
    }
    Some(Shell::new(program.to_owned(), command.args.clone()))
}

#[cfg(target_os = "macos")]
fn shell_for_local_session(shell_path: &str) -> Option<Shell> {
    let shell_program = resolve_shell_program(shell_path)?;
    let Some(user) = std::env::var("USER").ok().filter(|user| !user.is_empty()) else {
        return Some(Shell::new(shell_program, Vec::new()));
    };
    let home = std::env::var("HOME").unwrap_or_default();
    let has_home_hushlogin = !home.is_empty() && Path::new(&home).join(".hushlogin").exists();
    let flags = if has_home_hushlogin { "-qflp" } else { "-flp" };
    let mut args = vec![flags.to_owned(), user, shell_program.clone()];
    args.extend(macos_login_shell_args(&shell_program));

    Some(Shell::new("/usr/bin/login".to_owned(), args))
}

#[cfg(target_os = "macos")]
fn macos_login_shell_args(shell_program: &str) -> Vec<String> {
    match shell_name(shell_program) {
        Some("zsh") => vec!["-l".to_owned()],
        Some("fish") => vec!["--login".to_owned()],
        Some("bash" | "dash" | "sh" | "ksh" | "ksh93" | "mksh" | "csh" | "tcsh" | "ash") => {
            vec!["-l".to_owned()]
        }
        Some("pwsh" | "powershell") => vec!["-Login".to_owned()],
        _ => Vec::new(),
    }
}

#[cfg(not(target_os = "macos"))]
fn shell_for_local_session(shell_path: &str) -> Option<Shell> {
    resolve_shell_program(shell_path).map(|program| Shell::new(program, Vec::new()))
}

fn configure_history_filtering(options: &mut Options, shell_path: Option<&str>) {
    let Some(shell_path) = shell_path else {
        return;
    };
    if let Some("bash") = shell_name(shell_path) {
        let history_control = options
            .env
            .get("HISTCONTROL")
            .map(String::as_str)
            .unwrap_or_default();
        if !history_control
            .split(':')
            .any(|value| matches!(value.trim(), "ignorespace" | "ignoreboth"))
        {
            let value = if history_control.is_empty() {
                "ignorespace".to_owned()
            } else {
                format!("ignorespace:{history_control}")
            };
            options.env.insert("HISTCONTROL".to_owned(), value);
        }
    }
}

struct ShellIntegrationFiles {
    root: PathBuf,
}

impl Drop for ShellIntegrationFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn configure_shell_integration(
    options: &mut Options,
    shell_path: Option<&str>,
) -> io::Result<Option<ShellIntegrationFiles>> {
    let Some(shell_path) = shell_path else {
        return Ok(None);
    };
    match shell_name(shell_path) {
        Some("zsh") => configure_zsh_integration(options).map(Some),
        Some("bash") => configure_bash_integration(options, shell_path),
        Some("fish") => configure_fish_integration(options).map(Some),
        _ => Ok(None),
    }
}

fn create_shell_integration_root() -> io::Result<PathBuf> {
    let root = loop {
        let id = SHELL_INTEGRATION_ID.fetch_add(1, Ordering::Relaxed);
        let candidate = std::env::temp_dir().join(format!(
            "nauterm-shell-integration-{}-{id}",
            std::process::id()
        ));
        match std::fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        {
            let _ = std::fs::remove_dir_all(&root);
            return Err(error);
        }
    }
    Ok(root)
}

fn write_shell_integration_file(root: &Path, relative: &str, content: &str) -> io::Result<PathBuf> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    Ok(path)
}

fn configure_zsh_integration(options: &mut Options) -> io::Result<ShellIntegrationFiles> {
    let root = create_shell_integration_root()?;
    if let Err(error) = write_shell_integration_file(&root, ".zshenv", ZSH_INTEGRATION) {
        let _ = std::fs::remove_dir_all(&root);
        return Err(error);
    }

    if let Some(original) = options
        .env
        .get("ZDOTDIR")
        .cloned()
        .or_else(|| std::env::var("ZDOTDIR").ok())
    {
        options
            .env
            .insert("NAUTERM_ZSH_ZDOTDIR".to_owned(), original);
        options
            .env
            .insert("NAUTERM_ZSH_ZDOTDIR_SET".to_owned(), "1".to_owned());
    }
    options
        .env
        .insert("ZDOTDIR".to_owned(), root.to_string_lossy().into_owned());
    options.env.insert(
        "NAUTERM_SHELL_INTEGRATION_INJECT".to_owned(),
        "1".to_owned(),
    );
    Ok(ShellIntegrationFiles { root })
}

fn configure_bash_integration(
    options: &mut Options,
    shell_path: &str,
) -> io::Result<Option<ShellIntegrationFiles>> {
    let Some(shell) = bash_integration_shell(shell_path) else {
        return Ok(None);
    };
    let root = create_shell_integration_root()?;
    let script = match write_shell_integration_file(&root, "nauterm.bash", BASH_INTEGRATION) {
        Ok(script) => script,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&root);
            return Err(error);
        }
    };

    if let Some(original) = options
        .env
        .get("ENV")
        .cloned()
        .or_else(|| std::env::var("ENV").ok())
    {
        options.env.insert("NAUTERM_BASH_ENV".to_owned(), original);
        options
            .env
            .insert("NAUTERM_BASH_ENV_SET".to_owned(), "1".to_owned());
    }
    if !options.env.contains_key("HISTFILE") && std::env::var_os("HISTFILE").is_none() {
        if let Some(home) = user_home_dir() {
            options.env.insert(
                "HISTFILE".to_owned(),
                Path::new(&home)
                    .join(".bash_history")
                    .to_string_lossy()
                    .into_owned(),
            );
            options
                .env
                .insert("NAUTERM_BASH_UNEXPORT_HISTFILE".to_owned(), "1".to_owned());
        }
    }
    options
        .env
        .insert("ENV".to_owned(), script.to_string_lossy().into_owned());
    options.env.insert(
        "NAUTERM_SHELL_INTEGRATION_INJECT".to_owned(),
        "1".to_owned(),
    );
    options.shell = Some(shell);
    Ok(Some(ShellIntegrationFiles { root }))
}

#[cfg(target_os = "macos")]
fn bash_integration_shell(shell_path: &str) -> Option<Shell> {
    let shell_program = resolve_shell_program(shell_path)?;
    if shell_program == "/bin/bash" {
        return None;
    }
    let Some(user) = std::env::var("USER").ok().filter(|user| !user.is_empty()) else {
        return Some(Shell::new(shell_program, vec!["--posix".to_owned()]));
    };
    let home = std::env::var("HOME").unwrap_or_default();
    let has_home_hushlogin = !home.is_empty() && Path::new(&home).join(".hushlogin").exists();
    let flags = if has_home_hushlogin { "-qflp" } else { "-flp" };
    Some(Shell::new(
        "/usr/bin/login".to_owned(),
        vec![
            flags.to_owned(),
            user,
            shell_program,
            "-l".to_owned(),
            "--posix".to_owned(),
        ],
    ))
}

#[cfg(not(target_os = "macos"))]
fn bash_integration_shell(shell_path: &str) -> Option<Shell> {
    resolve_shell_program(shell_path).map(|program| Shell::new(program, vec!["--posix".to_owned()]))
}

fn configure_fish_integration(options: &mut Options) -> io::Result<ShellIntegrationFiles> {
    let root = create_shell_integration_root()?;
    if let Err(error) = write_shell_integration_file(
        &root,
        "fish/vendor_conf.d/nauterm-shell-integration.fish",
        FISH_INTEGRATION,
    ) {
        let _ = std::fs::remove_dir_all(&root);
        return Err(error);
    }

    let integration_dir = root.to_string_lossy().into_owned();
    let original = options
        .env
        .get("XDG_DATA_DIRS")
        .cloned()
        .or_else(|| std::env::var("XDG_DATA_DIRS").ok())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    options.env.insert(
        "XDG_DATA_DIRS".to_owned(),
        format!("{integration_dir}:{original}"),
    );
    options
        .env
        .insert("NAUTERM_FISH_XDG_DIR".to_owned(), integration_dir);
    options.env.insert(
        "NAUTERM_SHELL_INTEGRATION_INJECT".to_owned(),
        "1".to_owned(),
    );
    Ok(ShellIntegrationFiles { root })
}

pub(crate) const ZSH_INTEGRATION: &str = r#"if [[ -n ${NAUTERM_SHELL_INTEGRATION_INJECT+x} ]]; then
if [[ -n ${NAUTERM_ZSH_ZDOTDIR_SET+x} ]]; then
  builtin export ZDOTDIR="$NAUTERM_ZSH_ZDOTDIR"
  builtin unset NAUTERM_ZSH_ZDOTDIR NAUTERM_ZSH_ZDOTDIR_SET
else
  builtin unset ZDOTDIR
fi
{
  builtin typeset __nauterm_zshenv=${ZDOTDIR-$HOME}/.zshenv
  [[ ! -r "$__nauterm_zshenv" ]] || builtin source -- "$__nauterm_zshenv"
} always {
  builtin unset __nauterm_zshenv
}
  builtin unset NAUTERM_SHELL_INTEGRATION_INJECT
fi
if [[ -o interactive && -z ${__nauterm_shell_integration+x} ]]; then
  builtin typeset -gi __nauterm_shell_integration=1
  builtin typeset -gi __nauterm_command_active=0
  builtin typeset -gi __nauterm_ai_armed=0
  builtin typeset -g __nauterm_ai_token="${NAUTERM_SHELL_INTEGRATION_TOKEN:-$$}"
  builtin autoload -Uz add-zsh-hook add-zle-hook-widget
  __nauterm_command_preexec() {
    builtin local __nauterm_command_encoded
    __nauterm_command_encoded=$(builtin print -rn -- "$1" | command base64 2>/dev/null | command tr -d '\r\n')
    builtin print -rn -- $'\e]4545;CommandStarted;'$$';'${__nauterm_command_encoded}$'\a\e]133;C;aid='$$$'\a'
    __nauterm_command_active=1
  }
  __nauterm_prompt_precmd() {
    builtin local -i __nauterm_status=$?
    if (( __nauterm_command_active )); then
      builtin print -rn -- $'\e]4545;CommandExited;'$$';'${__nauterm_status}$'\a\e]133;D;'${__nauterm_status}$';aid='$$$'\a'
      __nauterm_command_active=0
    fi
    if (( __nauterm_ai_armed )); then
      builtin print -rn -- $'\e]777;nauterm-command-end='${__nauterm_ai_token}$';'${__nauterm_status}$'\a'
      __nauterm_ai_armed=0
    fi
    builtin print -rn -- $'\e]7;file://localhost'${PWD}$'\a\e]133;A;cl=line;aid='$$$'\a'
  }
  __nauterm_line_init() {
    builtin print -rn -- $'\e]133;B;aid='$$$'\a'
  }
  __nauterm_ai_park_line() {
    if [[ -n "$BUFFER" ]]; then
      zle -I
      builtin print -r -- ''
      BUFFER=''
      CURSOR=0
    fi
    __nauterm_ai_armed=1
    builtin print -rn -- $'\e]777;nauterm-line-ready='${__nauterm_ai_token}$'\a'
  }
  __nauterm_deferred_init() {
    add-zsh-hook -d precmd __nauterm_deferred_init
    add-zsh-hook precmd __nauterm_prompt_precmd
    add-zsh-hook preexec __nauterm_command_preexec
    add-zle-hook-widget line-init __nauterm_line_init
    zle -N __nauterm_ai_park_line
    bindkey '^X^]' __nauterm_ai_park_line
    __nauterm_prompt_precmd
  }
  add-zsh-hook precmd __nauterm_deferred_init
fi
"#;

pub(crate) const BASH_INTEGRATION: &str = r#"if [[ $- != *i* ]]; then
  return
fi

__nauterm_bootstrap_token="${NAUTERM_SHELL_INTEGRATION_TOKEN:-$BASHPID}"
if [[ -n ${NAUTERM_SHELL_INTEGRATION_INJECT+x} ]]; then
if [[ -n ${NAUTERM_BASH_ENV_SET+x} ]]; then
  export ENV="$NAUTERM_BASH_ENV"
else
  unset ENV
fi
unset NAUTERM_BASH_ENV NAUTERM_BASH_ENV_SET
set +o posix
shopt -u inherit_errexit 2>/dev/null
if [[ -n ${NAUTERM_BASH_UNEXPORT_HISTFILE+x} ]]; then
  export -n HISTFILE
  unset NAUTERM_BASH_UNEXPORT_HISTFILE
fi

if shopt -q login_shell; then
  [[ -r /etc/profile ]] && source /etc/profile
  for __nauterm_profile in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
    if [[ -r "$__nauterm_profile" ]]; then
      source "$__nauterm_profile"
      break
    fi
  done
else
  for __nauterm_profile in /etc/bash.bashrc /etc/bash/bashrc /etc/bashrc; do
    if [[ -r "$__nauterm_profile" ]]; then
      source "$__nauterm_profile"
      break
    fi
  done
  [[ -r "$HOME/.bashrc" ]] && source "$HOME/.bashrc"
fi
unset __nauterm_profile
unset NAUTERM_SHELL_INTEGRATION_INJECT
fi

if [[ -z ${__nauterm_shell_integration+x} ]]; then
  __nauterm_shell_integration=1
  __nauterm_ai_token="$__nauterm_bootstrap_token"
  __nauterm_last_histcmd=${HISTCMD:-0}
  unset __nauterm_bootstrap_token

  __nauterm_ai_precmd() {
    local __nauterm_status=$? __nauterm_command __nauterm_encoded
    if [[ ${HISTCMD:-0} -gt ${__nauterm_last_histcmd:-0} ]]; then
      __nauterm_command=$(builtin fc -ln -1 2>/dev/null)
      if [[ -n "$__nauterm_command" ]]; then
        __nauterm_encoded=$(printf '%s' "$__nauterm_command" | command base64 2>/dev/null | command tr -d '\r\n')
        printf '\033]4545;CommandStarted;%s;%s\007\033]133;C;aid=%s\007' "$BASHPID" "$__nauterm_encoded" "$BASHPID"
        printf '\033]4545;CommandExited;%s;%s\007\033]133;D;%s;aid=%s\007' "$BASHPID" "$__nauterm_status" "$__nauterm_status" "$BASHPID"
      fi
      __nauterm_last_histcmd=$HISTCMD
    fi
    printf '\033]777;nauterm-command-end=%s;%s\033\\' "$__nauterm_ai_token" "$__nauterm_status"
    printf '\033]7;file://localhost%s\007\033]133;A;cl=line;aid=%s\007' "$PWD" "$BASHPID"
    return "$__nauterm_status"
  }
  bind '"\C-x\C-]":"\C-a\C-k"'

  if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == "declare -a"* ]]; then
    __nauterm_prompt_commands=("${PROMPT_COMMAND[@]}")
    PROMPT_COMMAND=(__nauterm_ai_precmd "${__nauterm_prompt_commands[@]}")
    unset __nauterm_prompt_commands
  else
    __nauterm_prompt_command=${PROMPT_COMMAND:-}
    PROMPT_COMMAND="__nauterm_ai_precmd${__nauterm_prompt_command:+;$__nauterm_prompt_command}"
    unset __nauterm_prompt_command
  fi
  [[ "$PS1" == *'133;B;aid='* ]] || PS1="${PS1}"'\[\e]133;B;aid='"$BASHPID"'\a\]'
  [[ "$PS2" == *'133;B;aid='* ]] || PS2="${PS2}"'\[\e]133;B;aid='"$BASHPID"'\a\]'
fi
"#;

pub(crate) const FISH_INTEGRATION: &str = r#"if set -q NAUTERM_SHELL_INTEGRATION_INJECT
if set -q NAUTERM_FISH_XDG_DIR
    set -l __nauterm_xdg_dirs
    if set -q XDG_DATA_DIRS
        for __nauterm_xdg_dir in (string split : -- "$XDG_DATA_DIRS")
            if test "$__nauterm_xdg_dir" != "$NAUTERM_FISH_XDG_DIR"
                set -a __nauterm_xdg_dirs "$__nauterm_xdg_dir"
            end
        end
    end
    if test (count $__nauterm_xdg_dirs) -gt 0
        set -gx XDG_DATA_DIRS (string join : -- $__nauterm_xdg_dirs)
    else
        set -e XDG_DATA_DIRS
    end
    set -e NAUTERM_FISH_XDG_DIR
end
set -e NAUTERM_SHELL_INTEGRATION_INJECT
end

status --is-interactive; or return
set -q __nauterm_shell_integration; and return
set -g __nauterm_shell_integration 1
set -g __nauterm_ai_armed 0
set -g __nauterm_ai_token $fish_pid
if set -q NAUTERM_SHELL_INTEGRATION_TOKEN
    set -g __nauterm_ai_token "$NAUTERM_SHELL_INTEGRATION_TOKEN"
end

function __nauterm_setup --on-event fish_prompt
    functions -e __nauterm_setup

    function __nauterm_prompt_start --on-event fish_prompt
        printf '\033]7;file://localhost%s\007\033]133;A;cl=line;aid=%s\007' $PWD $fish_pid
    end
    function __nauterm_command_preexec --on-event fish_preexec
        set -l __nauterm_encoded (printf '%s' "$argv[1]" | command base64 2>/dev/null | string collect | string replace -a \n '')
        printf '\033]4545;CommandStarted;%s;%s\007\033]133;C;aid=%s\007' $fish_pid $__nauterm_encoded $fish_pid
    end
    function __nauterm_command_postexec --on-event fish_postexec
        set -l __nauterm_status $status
        printf '\033]4545;CommandExited;%s;%s\007\033]133;D;%s;aid=%s\007' $fish_pid $__nauterm_status $__nauterm_status $fish_pid
        if test $__nauterm_ai_armed -eq 1
            printf '\033]777;nauterm-command-end=%s;%s\007' $__nauterm_ai_token $__nauterm_status
            set -g __nauterm_ai_armed 0
        end
    end
    function __nauterm_ai_park_line
        if test -n (commandline)
            echo
            commandline -r ''
            commandline -f repaint
        end
        set -g __nauterm_ai_armed 1
        printf '\033]777;nauterm-line-ready=%s\007' $__nauterm_ai_token
    end

    functions -e __nauterm_original_fish_prompt 2>/dev/null
    functions -c fish_prompt __nauterm_original_fish_prompt
    function fish_prompt
        __nauterm_original_fish_prompt
        printf '\033]133;B;aid=%s\007' $fish_pid
    end
    bind \cx\c] __nauterm_ai_park_line
    __nauterm_prompt_start
end
"#;

fn shell_name(shell_path: &str) -> Option<&str> {
    Path::new(shell_path).file_name()?.to_str()
}

#[cfg(windows)]
fn default_local_shell() -> Option<Shell> {
    for shell in ["pwsh.exe", "powershell.exe", "cmd.exe"] {
        if let Some(program) = resolve_shell_program(shell) {
            return Some(Shell::new(program, Vec::new()));
        }
    }
    None
}

#[cfg(not(windows))]
fn default_local_shell() -> Option<Shell> {
    None
}

fn resolve_shell_program(shell_path: &str) -> Option<String> {
    let shell_path = shell_path.trim();
    if shell_path.is_empty() {
        return None;
    }

    let path = Path::new(shell_path);
    if path.is_absolute() || shell_path.contains(std::path::MAIN_SEPARATOR) {
        return path.is_file().then(|| shell_path.to_owned());
    }

    let path_env = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path_env) {
        let candidate = directory.join(shell_path);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }

    None
}

fn resolve_working_directory(path: Option<&Path>) -> Option<PathBuf> {
    let home = user_home_dir();
    let expanded = path.and_then(|path| expand_working_directory(path, home.as_deref()));

    if let Some(path) = expanded {
        if path.is_dir() {
            return Some(path);
        }
    }

    home.map(PathBuf::from).filter(|path| path.is_dir())
}

#[cfg(windows)]
fn user_home_dir() -> Option<String> {
    std::env::var("USERPROFILE")
        .ok()
        .filter(|home| !home.is_empty())
        .or_else(|| {
            let drive = std::env::var("HOMEDRIVE")
                .ok()
                .filter(|drive| !drive.is_empty())?;
            let path = std::env::var("HOMEPATH")
                .ok()
                .filter(|path| !path.is_empty())?;
            Some(format!("{drive}{path}"))
        })
        .or_else(|| std::env::var("HOME").ok().filter(|home| !home.is_empty()))
}

#[cfg(not(windows))]
fn user_home_dir() -> Option<String> {
    std::env::var("HOME").ok().filter(|home| !home.is_empty())
}

fn expand_working_directory(path: &Path, home: Option<&str>) -> Option<PathBuf> {
    let text = path.to_string_lossy().trim().to_owned();
    if text.is_empty() {
        return None;
    }

    if text == "~" {
        return home.map(PathBuf::from);
    }
    if let Some(rest) = text.strip_prefix("~/") {
        return home.map(|home| PathBuf::from(home).join(rest));
    }

    let path = PathBuf::from(text);
    if path.is_absolute() {
        return Some(path);
    }

    home.map(|home| PathBuf::from(home).join(&path))
}

impl Drop for LocalPty {
    fn drop(&mut self) {
        if let Ok(mut wakeup) = self.wakeup.lock() {
            *wakeup = None;
        }
        self.commands.send(PtyCommand::Shutdown);
        join_worker(&mut self.worker, "local PTY");
    }
}

impl WakeupCallback {
    pub fn new(callback: extern "C" fn(*mut c_void), user_data: *mut c_void) -> Self {
        Self {
            callback,
            user_data: user_data as usize,
        }
    }

    pub(crate) fn raw_parts(self) -> (extern "C" fn(*mut c_void), *mut c_void) {
        (self.callback, self.user_data as *mut c_void)
    }

    pub fn call(self) {
        if !WAKEUP_CALLBACKS_ENABLED.load(Ordering::Acquire) {
            return;
        }
        (self.callback)(self.user_data as *mut c_void);
    }
}

pub(crate) fn disable_wakeup_callbacks() {
    WAKEUP_CALLBACKS_ENABLED.store(false, Ordering::Release);
}

pub(crate) fn enable_wakeup_callbacks() {
    WAKEUP_CALLBACKS_ENABLED.store(true, Ordering::Release);
}

pub(crate) fn join_worker(worker: &mut Option<JoinHandle<()>>, name: &str) {
    let Some(worker) = worker.take() else {
        return;
    };
    if worker.thread().id() == thread::current().id() {
        eprintln!("nauterm: refusing to join {name} worker from itself");
        return;
    }
    if worker.join().is_err() {
        eprintln!("nauterm: {name} worker panicked during shutdown");
    }
}

impl PtyCommandSender {
    fn send(&self, command: PtyCommand) {
        let _ = self.sender.send(command);
        let _ = self.poller.notify();
    }
}

fn spawn_pty_worker(
    pty: Pty,
    poller: Arc<Poller>,
    receiver: Receiver<PtyCommand>,
    exited: Arc<AtomicBool>,
    input_visible: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    wakeup: WakeupSlot,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("nauterm-pty".to_owned())
        .spawn(move || run_pty_worker(pty, poller, receiver, exited, input_visible, output, wakeup))
}

fn run_pty_worker(
    mut pty: Pty,
    poller: Arc<Poller>,
    receiver: Receiver<PtyCommand>,
    exited: Arc<AtomicBool>,
    input_visible: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    wakeup: WakeupSlot,
) {
    let poll_mode = PollMode::Level;
    let mut interest = PollingEvent::readable(PTY_READ_WRITE_TOKEN);
    if unsafe { pty.register(&poller, interest, poll_mode) }.is_err() {
        terminate_pty(&pty);
        exited.store(true, Ordering::Release);
        notify_wakeup(&wakeup);
        return;
    }
    update_input_visibility(&pty, &input_visible);

    let mut events = Events::with_capacity(NonZeroUsize::new(1024).expect("non-zero capacity"));
    let mut pending_input = Vec::new();

    loop {
        if !drain_commands(&receiver, &mut pty, &mut pending_input) {
            break;
        }
        if flush_pending_input(&mut pty, &mut pending_input).is_err() {
            break;
        }
        update_input_visibility(&pty, &input_visible);
        if sync_write_interest(&mut pty, &poller, &mut interest, poll_mode, &pending_input).is_err()
        {
            break;
        }

        events.clear();
        if let Err(error) = poller.wait(&mut events, None) {
            if error.kind() == ErrorKind::Interrupted {
                continue;
            }
            break;
        }

        if !drain_commands(&receiver, &mut pty, &mut pending_input) {
            break;
        }

        let mut should_stop = false;
        for event in events.iter() {
            match event.key {
                PTY_READ_WRITE_TOKEN => {
                    if event.readable && read_available_output(&mut pty, &output, &wakeup).is_err()
                    {
                        should_stop = true;
                        break;
                    }
                    if event.readable {
                        update_input_visibility(&pty, &input_visible);
                    }
                    if event.writable && flush_pending_input(&mut pty, &mut pending_input).is_err()
                    {
                        should_stop = true;
                        break;
                    }
                    if event.writable {
                        update_input_visibility(&pty, &input_visible);
                    }
                }
                PTY_CHILD_EVENT_TOKEN if pty.next_child_event().is_some() => {
                    exited.store(true, Ordering::Release);
                    notify_wakeup(&wakeup);
                    let _ = pty.deregister(&poller);
                    return;
                }
                _ => {}
            }
        }
        if should_stop {
            break;
        }

        if sync_write_interest(&mut pty, &poller, &mut interest, poll_mode, &pending_input).is_err()
        {
            break;
        }
    }

    let _ = pty.deregister(&poller);
    terminate_pty(&pty);
    exited.store(true, Ordering::Release);
    notify_wakeup(&wakeup);
}

#[cfg(unix)]
fn update_input_visibility(pty: &Pty, input_visible: &Arc<AtomicBool>) {
    let mode = read_slave_input_visibility(pty).or_else(|| read_master_input_visibility(pty));
    if let Some(enabled) = mode {
        input_visible.store(enabled, Ordering::Release);
    }
}

#[cfg(unix)]
fn read_slave_input_visibility(pty: &Pty) -> Option<bool> {
    let path = slave_name_for_pty(pty)?;
    read_input_visibility_from_path(&path)
}

#[cfg(target_os = "macos")]
fn read_master_input_visibility(_pty: &Pty) -> Option<bool> {
    None
}

#[cfg(all(unix, not(target_os = "macos")))]
fn read_master_input_visibility(pty: &Pty) -> Option<bool> {
    read_input_visibility_from_fd(pty.file().as_raw_fd())
}

#[cfg(target_os = "macos")]
fn slave_name_for_pty(pty: &Pty) -> Option<CString> {
    let mut buffer = [0 as libc::c_char; 128];
    let result = unsafe {
        libc::ioctl(
            pty.file().as_raw_fd(),
            libc::TIOCPTYGNAME.into(),
            buffer.as_mut_ptr(),
        )
    };
    if result == 0 {
        return Some(unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_owned());
    }
    slave_name_from_ptsname(pty)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn slave_name_for_pty(pty: &Pty) -> Option<CString> {
    slave_name_from_ptsname(pty)
}

#[cfg(unix)]
fn slave_name_from_ptsname(pty: &Pty) -> Option<CString> {
    let path = unsafe { libc::ptsname(pty.file().as_raw_fd()) };
    if path.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(path) }.to_owned())
}

#[cfg(unix)]
fn read_input_visibility_from_path(path: &CStr) -> Option<bool> {
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_NOCTTY) };
    if fd < 0 {
        return None;
    }
    let result = read_input_visibility_from_fd(fd);
    unsafe {
        libc::close(fd);
    }
    result
}

#[cfg(unix)]
fn read_input_visibility_from_fd(fd: std::os::fd::RawFd) -> Option<bool> {
    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    let result = unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) };
    if result == 0 {
        let termios = unsafe { termios.assume_init() };
        let echo = (termios.c_lflag & libc::ECHO) != 0;
        let canonical = (termios.c_lflag & libc::ICANON) != 0;
        return Some(echo || !canonical);
    }
    None
}

#[cfg(not(unix))]
fn update_input_visibility(_pty: &Pty, input_visible: &Arc<AtomicBool>) {
    input_visible.store(true, Ordering::Release);
}

fn drain_commands(
    receiver: &Receiver<PtyCommand>,
    pty: &mut Pty,
    pending_input: &mut Vec<u8>,
) -> bool {
    loop {
        match receiver.try_recv() {
            Ok(PtyCommand::Input(bytes)) => pending_input.extend_from_slice(&bytes),
            Ok(PtyCommand::Resize(geometry)) => pty.on_resize(window_size(geometry)),
            Ok(PtyCommand::Shutdown) | Err(TryRecvError::Disconnected) => return false,
            Err(TryRecvError::Empty) => return true,
        }
    }
}

#[cfg(unix)]
fn terminate_pty(pty: &Pty) {
    let pid = pty.child().id() as libc::pid_t;
    let master_fd = pty.file().as_raw_fd();
    unsafe {
        // Login wrappers can put the interactive shell in a different foreground
        // process group. Killing only the spawned child/group leaves that shell
        // alive and Alacritty's Pty::drop then blocks forever in Child::wait.
        let foreground_pgid = libc::tcgetpgrp(master_fd);
        if foreground_pgid > 0 {
            libc::kill(-foreground_pgid, libc::SIGKILL);
        }
        libc::kill(-pid, libc::SIGKILL);
        libc::kill(pid, libc::SIGKILL);

        // Alacritty waits for the child before dropping its master fd. Replace
        // the master with /dev/null first so the controlling terminal is closed
        // and every process still attached to it receives the kernel hangup.
        // dup2 keeps the owned fd valid, avoiding a double-close when Pty drops.
        let dev_null = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR | libc::O_CLOEXEC);
        if dev_null >= 0 && dev_null != master_fd {
            libc::dup2(dev_null, master_fd);
            libc::close(dev_null);
        }
    }
}

#[cfg(not(unix))]
fn terminate_pty(_pty: &Pty) {}

fn flush_pending_input(pty: &mut Pty, pending_input: &mut Vec<u8>) -> io::Result<()> {
    while !pending_input.is_empty() {
        match pty.writer().write(pending_input) {
            Ok(0) => break,
            Ok(written) => {
                pending_input.drain(..written);
            }
            Err(error) if is_transient_io(&error) => break,
            Err(error) => return Err(error),
        }
    }

    Ok(())
}

fn read_available_output(
    pty: &mut Pty,
    output: &Arc<Mutex<Vec<u8>>>,
    wakeup: &WakeupSlot,
) -> io::Result<()> {
    let mut buffer = [0; PTY_READ_BUFFER_SIZE];
    let mut has_output = false;
    let mut bytes_read = 0;

    loop {
        match pty.reader().read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                has_output = true;
                bytes_read += read;
                append_output(output, &buffer[..read]);
                if bytes_read >= PTY_MAX_READ_BURST_BYTES {
                    break;
                }
            }
            Err(error) if is_transient_io(&error) => break,
            #[cfg(target_os = "linux")]
            Err(error) if error.raw_os_error() == Some(libc::EIO) => break,
            Err(error) => return Err(error),
        }
    }

    if has_output {
        notify_wakeup(wakeup);
    }

    Ok(())
}

fn sync_write_interest(
    pty: &mut Pty,
    poller: &Arc<Poller>,
    interest: &mut PollingEvent,
    poll_mode: PollMode,
    pending_input: &[u8],
) -> io::Result<()> {
    let wants_write = !pending_input.is_empty();
    if interest.writable == wants_write {
        return Ok(());
    }

    interest.writable = wants_write;
    pty.reregister(poller, *interest, poll_mode)
}

fn notify_wakeup(wakeup: &WakeupSlot) {
    if let Ok(wakeup) = wakeup.lock() {
        if let Some(callback) = *wakeup {
            callback.call();
        }
    }
}

fn is_transient_io(error: &io::Error) -> bool {
    matches!(error.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock)
}

fn window_size(geometry: TerminalGeometry) -> WindowSize {
    WindowSize {
        num_lines: geometry.rows as u16,
        num_cols: geometry.columns as u16,
        cell_width: CELL_WIDTH_PIXELS,
        cell_height: CELL_HEIGHT_PIXELS,
    }
}

#[cfg(test)]
mod shutdown_tests {
    #[cfg(target_os = "macos")]
    use super::LocalPty;
    use super::{configure_history_filtering, configure_shell_integration, join_worker};
    #[cfg(target_os = "macos")]
    use crate::terminal::{TerminalGeometry, TerminalOptions};
    use alacritty_terminal::tty::Options;
    use std::sync::atomic::{AtomicBool, Ordering};
    #[cfg(target_os = "macos")]
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread;
    #[cfg(target_os = "macos")]
    use std::time::{Duration, Instant};

    #[test]
    fn join_worker_waits_for_completion_and_consumes_handle() {
        let completed = Arc::new(AtomicBool::new(false));
        let worker_completed = completed.clone();
        let mut worker = Some(thread::spawn(move || {
            worker_completed.store(true, Ordering::Release);
        }));

        join_worker(&mut worker, "test");

        assert!(worker.is_none());
        assert!(completed.load(Ordering::Acquire));
    }

    #[test]
    fn bash_sessions_ignore_leading_space_bootstrap_commands() {
        let mut options = Options::default();
        configure_history_filtering(&mut options, Some("/bin/bash"));

        assert_eq!(
            options.env.get("HISTCONTROL").map(String::as_str),
            Some("ignorespace")
        );

        options
            .env
            .insert("HISTCONTROL".to_owned(), "ignoredups".to_owned());
        configure_history_filtering(&mut options, Some("/bin/bash"));
        assert_eq!(
            options.env.get("HISTCONTROL").map(String::as_str),
            Some("ignorespace:ignoredups")
        );
    }

    #[test]
    fn zsh_integration_is_loaded_before_the_shell_without_typing_a_command() {
        let mut options = Options::default();
        options
            .env
            .insert("ZDOTDIR".to_owned(), "/custom/zsh".to_owned());

        let files = configure_shell_integration(&mut options, Some("/bin/zsh"))
            .expect("configure shell integration")
            .expect("zsh integration files");
        let temporary_zdotdir = options
            .env
            .get("ZDOTDIR")
            .expect("temporary ZDOTDIR")
            .clone();

        assert_eq!(
            options.env.get("NAUTERM_ZSH_ZDOTDIR").map(String::as_str),
            Some("/custom/zsh")
        );
        assert_eq!(
            options
                .env
                .get("NAUTERM_ZSH_ZDOTDIR_SET")
                .map(String::as_str),
            Some("1")
        );
        assert!(std::path::Path::new(&temporary_zdotdir)
            .join(".zshenv")
            .is_file());

        drop(files);
        assert!(!std::path::Path::new(&temporary_zdotdir).exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn zsh_accepts_the_generated_startup_integration() {
        let mut options = Options::default();
        options.env.insert(
            "ZDOTDIR".to_owned(),
            "/dev/null/nauterm-no-user-config".to_owned(),
        );
        options.env.insert(
            "NAUTERM_SHELL_INTEGRATION_TOKEN".to_owned(),
            "test-token".to_owned(),
        );
        let _files = configure_shell_integration(&mut options, Some("/bin/zsh"))
            .expect("configure shell integration")
            .expect("zsh integration files");

        let output = std::process::Command::new("/bin/zsh")
            .args([
                "-dic",
                "[[ $__nauterm_ai_token == test-token ]] && [[ $(whence -w __nauterm_ai_park_line) == *function* ]]",
            ])
            .envs(&options.env)
            .output()
            .expect("start zsh");

        assert!(
            output.status.success(),
            "zsh rejected startup integration: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn unsupported_shell_does_not_mutate_startup_environment() {
        let mut options = Options::default();
        let files = configure_shell_integration(&mut options, Some("/bin/dash"))
            .expect("configure shell integration");

        assert!(files.is_none());
        assert!(!options.env.contains_key("ZDOTDIR"));
    }

    #[test]
    fn fish_integration_uses_and_then_restores_xdg_data_dirs() {
        let mut options = Options::default();
        options
            .env
            .insert("XDG_DATA_DIRS".to_owned(), "/custom/share".to_owned());
        options.env.insert(
            "NAUTERM_SHELL_INTEGRATION_TOKEN".to_owned(),
            "test-token".to_owned(),
        );
        let files = configure_shell_integration(&mut options, Some("/usr/local/bin/fish"))
            .expect("configure fish integration")
            .expect("fish integration files");
        let integration_dir = options
            .env
            .get("NAUTERM_FISH_XDG_DIR")
            .expect("fish integration directory");

        let expected_xdg_data_dirs = format!("{integration_dir}:/custom/share");
        assert_eq!(
            options.env.get("XDG_DATA_DIRS").map(String::as_str),
            Some(expected_xdg_data_dirs.as_str())
        );
        assert!(files
            .root
            .join("fish/vendor_conf.d/nauterm-shell-integration.fish")
            .is_file());
        let integration_script = files
            .root
            .join("fish/vendor_conf.d/nauterm-shell-integration.fish");

        let Some(fish) = super::resolve_shell_program("fish") else {
            return;
        };
        let output = std::process::Command::new(fish)
            .args([
                "-i",
                "-c",
                "source \"$argv[1]\"; or exit 10; emit fish_prompt; fish_prompt; functions -q __nauterm_ai_park_line; or exit 11; test \"$__nauterm_ai_token\" = test-token; or exit 12; test \"$XDG_DATA_DIRS\" = /custom/share; or exit 13",
            ])
            .arg(integration_script)
            .envs(&options.env)
            .env("HOME", &files.root)
            .env("XDG_CONFIG_HOME", files.root.join("config"))
            .env("TERM", "xterm-256color")
            .output()
            .expect("start fish with integration");
        assert!(
            output.status.success(),
            "fish did not load startup integration (status {}): stdout: {} stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output
            .stdout
            .windows(b"\x1b]133;A;cl=line;aid=".len())
            .any(|window| window == b"\x1b]133;A;cl=line;aid="));
        assert!(output
            .stdout
            .windows(b"\x1b]133;B;aid=".len())
            .any(|window| window == b"\x1b]133;B;aid="));
    }

    #[test]
    fn generated_bash_and_fish_scripts_pass_available_shell_syntax_checks() {
        assert!(super::BASH_INTEGRATION.contains(r#"\C-x\C-]":"\C-a\C-k"#));
        assert!(!super::BASH_INTEGRATION.contains(r#"\C-u"#));
        assert!(!super::BASH_INTEGRATION.contains("bind -x"));
        assert!(!super::BASH_INTEGRATION.contains("READLINE_LINE"));

        let bash_name = if cfg!(windows) { "bash.exe" } else { "bash" };
        let Some(bash_program) = super::resolve_shell_program(bash_name) else {
            return;
        };
        let bash = std::process::Command::new(&bash_program)
            .args(["-n", "-c", super::BASH_INTEGRATION])
            .status()
            .expect("check bash integration syntax");
        assert!(bash.success());

        let bash_root = super::create_shell_integration_root().expect("create bash test root");
        let bash_files = super::ShellIntegrationFiles { root: bash_root };
        let bash_script = super::write_shell_integration_file(
            &bash_files.root,
            "nauterm.bash",
            super::BASH_INTEGRATION,
        )
        .expect("write bash integration");
        let bash = std::process::Command::new(bash_program)
            .args([
                "--noprofile",
                "--norc",
                "-ic",
                "source \"$1\"; [[ $(type -t __nauterm_ai_precmd) == function ]] && [[ $__nauterm_ai_token == test-token ]] && ! shopt -qo posix",
                "nauterm-test",
            ])
            .arg(bash_script)
            .env("HOME", &bash_files.root)
            .env("NAUTERM_SHELL_INTEGRATION_TOKEN", "test-token")
            .env("TERM", "xterm-256color")
            .output()
            .expect("start bash with integration");
        assert!(
            bash.status.success(),
            "bash did not load startup integration: {}",
            String::from_utf8_lossy(&bash.stderr)
        );

        let Some(fish) = super::resolve_shell_program("fish") else {
            return;
        };
        let root = super::create_shell_integration_root().expect("create integration root");
        let files = super::ShellIntegrationFiles { root };
        let script = super::write_shell_integration_file(
            &files.root,
            "nauterm.fish",
            super::FISH_INTEGRATION,
        )
        .expect("write fish integration");
        let fish = std::process::Command::new(fish)
            .arg("-n")
            .arg(script)
            .status()
            .expect("check fish integration syntax");
        assert!(fish.success());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apple_system_bash_keeps_the_safe_fallback_path() {
        let mut options = Options::default();
        let files = configure_shell_integration(&mut options, Some("/bin/bash"))
            .expect("configure bash integration");

        assert!(files.is_none());
        assert!(!options.env.contains_key("ENV"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn local_pty_closes_after_login_shell_reaches_first_prompt() {
        let (finished_tx, finished_rx) = mpsc::channel();
        thread::spawn(move || {
            for _ in 0..8 {
                let options = TerminalOptions {
                    shell_path: Some("/bin/zsh".to_owned()),
                    ..TerminalOptions::default()
                };
                let mut pty = LocalPty::spawn(
                    TerminalGeometry {
                        columns: 80,
                        rows: 24,
                    },
                    &options,
                )
                .expect("spawn local PTY");

                let output_deadline = Instant::now() + Duration::from_secs(3);
                while Instant::now() < output_deadline {
                    if !pty.drain_output().output.is_empty() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                drop(pty);
            }
            let _ = finished_tx.send(());
        });

        finished_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("local PTY shutdown must not wait forever for /usr/bin/login");
    }
}
