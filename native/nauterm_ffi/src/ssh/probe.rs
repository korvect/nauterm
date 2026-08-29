use super::*;

pub(super) async fn detect_host_os(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    certificate: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<&SshProxyConfig>,
    host_key_trust_mode: HostKeyTrustMode,
) -> HostOsDetection {
    let config = ssh_client_config();
    let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
    let output = Arc::new(Mutex::new(Vec::new()));
    let wakeup = Arc::new(Mutex::new(None));
    let handler = SshClientHandler {
        host: host.to_owned(),
        port,
        known_hosts_path: known_hosts_path.map(PathBuf::from),
        host_key_trust_mode,
        output,
        events: events.clone(),
        wakeup: wakeup.clone(),
    };
    let result = async {
        let mut handle = connect_ssh_with_timeout(config, host, port, proxy, handler).await?;
        authenticate(
            &mut handle,
            username,
            private_key,
            certificate,
            passphrase,
            password,
            &events,
            &wakeup,
        )
        .await
        .map_err(|error| format!("authentication failed: {error}"))?;

        let shell = run_ssh_exec_command(&mut handle, "echo $SHELL").await.ok();
        let shell_name = command_stdout(&shell).to_ascii_lowercase();

        let router_os = run_ssh_exec_command(&mut handle, ":put [/system resource get platform]")
            .await
            .ok();
        if let Some(platform) = detected_os_from_routeros(router_os.as_ref()) {
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
            return Ok(platform);
        }

        let unix_script = if shell_name.contains("fish") {
            r#"if set name (uname) = "Linux"
    cat /etc/*release
else
    uname
end"#
        } else {
            r#"HISTFILE=;
SA_OS_TYPE="Linux"
REAL_OS_NAME=`uname`
if [ "$REAL_OS_NAME" != "$SA_OS_TYPE" ] ;
then
echo `uname`
else
DISTRIB_ID="`cat /etc/*release`"
echo $DISTRIB_ID;
fi;
exit;"#
        };
        let unix_detection = run_ssh_exec_command(&mut handle, unix_script).await.ok();
        if let Some(platform) = detected_os_from_unix_probe(unix_detection.as_ref()) {
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
            return Ok(platform);
        }

        let _ = handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        Err("unable to detect host OS".to_owned())
    }
    .await;
    let captured_events = events
        .lock()
        .map(|events| events.clone())
        .unwrap_or_default();
    match result {
        Ok(platform) => HostOsDetection {
            os: Some(platform.os),
            distro: platform.distro,
            events: captured_events,
            error: None,
        },
        Err(error) => HostOsDetection {
            os: None,
            distro: None,
            events: captured_events,
            error: Some(error),
        },
    }
}

const HOST_SYSTEM_INFO_SCRIPT: &str = r#"LC_ALL=C
nauterm_emit() { printf '%s=%s\n' "$1" "$2"; }

nauterm_emit hostname "$(hostname 2>/dev/null || uname -n 2>/dev/null)"
nauterm_emit kernel "$(uname -sr 2>/dev/null)"
nauterm_emit architecture "$(uname -m 2>/dev/null)"

if [ -r /etc/os-release ]; then
  . /etc/os-release
  nauterm_emit os_name "${PRETTY_NAME:-${NAME:-Linux}}"
else
  nauterm_emit os_name "$(uname -s 2>/dev/null)"
fi

if [ -r /proc/uptime ]; then
  nauterm_emit uptime_seconds "$(awk '{printf "%.0f", $1}' /proc/uptime)"
elif command -v sysctl >/dev/null 2>&1; then
  nauterm_boot="$(sysctl -n kern.boottime 2>/dev/null | sed -E 's/.*sec = ([0-9]+).*/\1/')"
  if [ -n "$nauterm_boot" ]; then
    nauterm_emit uptime_seconds "$(($(date +%s) - nauterm_boot))"
  fi
fi

if [ -r /proc/loadavg ]; then
  awk '{printf "load_average=%s\nload_average_5=%s\nload_average_15=%s\n", $1, $2, $3}' /proc/loadavg
else
  nauterm_emit load_average "$(uptime 2>/dev/null | sed -E 's/.*load averages?:[[:space:]]*([0-9.]+).*/\1/')"
fi

nauterm_cpu_count="$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null)"
nauterm_emit cpu_count "$nauterm_cpu_count"

if [ -r /proc/stat ]; then
  set -- $(head -n 1 /proc/stat)
  shift
  nauterm_total_1=0
  for nauterm_value in "$@"; do nauterm_total_1=$((nauterm_total_1 + nauterm_value)); done
  nauterm_idle_1=$((${4:-0} + ${5:-0}))
  sleep 0.2 2>/dev/null || sleep 1
  set -- $(head -n 1 /proc/stat)
  shift
  nauterm_total_2=0
  for nauterm_value in "$@"; do nauterm_total_2=$((nauterm_total_2 + nauterm_value)); done
  nauterm_idle_2=$((${4:-0} + ${5:-0}))
  nauterm_emit cpu_usage_percent "$(awk -v t1="$nauterm_total_1" -v t2="$nauterm_total_2" -v i1="$nauterm_idle_1" -v i2="$nauterm_idle_2" 'BEGIN { d=t2-t1; if (d>0) printf "%.1f", 100*(d-(i2-i1))/d }')"
elif command -v ps >/dev/null 2>&1; then
  nauterm_emit cpu_usage_percent "$(ps -A -o %cpu= 2>/dev/null | awk -v n="${nauterm_cpu_count:-1}" '{s+=$1} END {if (n<1)n=1; v=s/n; if(v>100)v=100; printf "%.1f", v}')"
fi

if [ -r /proc/meminfo ]; then
  awk '/^MemTotal:/ {t=$2*1024} /^MemAvailable:/ {a=$2*1024} /^SwapTotal:/ {st=$2*1024} /^SwapFree:/ {sf=$2*1024} END {if(t>0){printf "memory_total_bytes=%.0f\nmemory_used_bytes=%.0f\n", t, t-a} if(st>=0){printf "swap_total_bytes=%.0f\nswap_used_bytes=%.0f\n", st, st-sf}}' /proc/meminfo
elif command -v sysctl >/dev/null 2>&1; then
  nauterm_memory_total="$(sysctl -n hw.memsize 2>/dev/null || sysctl -n hw.physmem 2>/dev/null)"
  nauterm_emit memory_total_bytes "$nauterm_memory_total"
  if command -v vm_stat >/dev/null 2>&1 && [ -n "$nauterm_memory_total" ]; then
    nauterm_page_size="$(sysctl -n hw.pagesize 2>/dev/null)"
    nauterm_free_pages="$(vm_stat 2>/dev/null | awk '/Pages free/ {gsub("\\.","",$3); f=$3} /Pages speculative/ {gsub("\\.","",$3); s=$3} END {print f+s}')"
    nauterm_emit memory_used_bytes "$(awk -v t="$nauterm_memory_total" -v p="${nauterm_page_size:-4096}" -v f="${nauterm_free_pages:-0}" 'BEGIN {printf "%.0f", t-p*f}')"
  fi
fi

df -Pk / 2>/dev/null | awk 'NR==2 {printf "disk_total_bytes=%.0f\ndisk_used_bytes=%.0f\n", $2*1024, $3*1024}'
ps -eo rss=,pcpu=,comm= --sort=-rss 2>/dev/null | head -n 6 | awk '{printf "process=%.0f|%s|%s\n", $1*1024, $2, $3}'
if [ -r /proc/net/dev ]; then
  awk -F: 'NR>2 {name=$1; gsub(/[[:space:]]/,"",name); split($2,v,/ +/); if(name!="lo") printf "network=%s|%s|%s\n", name, v[2], v[10]}' /proc/net/dev
fi
df -Pk 2>/dev/null | awk 'NR>1 {printf "filesystem=%s|%.0f|%.0f\n", $6, $2*1024, $3*1024}'
"#;

pub(super) async fn collect_host_system_info(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    certificate: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<&SshProxyConfig>,
    host_key_trust_mode: HostKeyTrustMode,
) -> HostSystemInfo {
    let config = ssh_client_config();
    let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
    let output = Arc::new(Mutex::new(Vec::new()));
    let wakeup = Arc::new(Mutex::new(None));
    let handler = SshClientHandler {
        host: host.to_owned(),
        port,
        known_hosts_path: known_hosts_path.map(PathBuf::from),
        host_key_trust_mode,
        output,
        events: events.clone(),
        wakeup: wakeup.clone(),
    };
    let result = async {
        let mut handle = connect_ssh_with_timeout(config, host, port, proxy, handler).await?;
        authenticate(
            &mut handle,
            username,
            private_key,
            certificate,
            passphrase,
            password,
            &events,
            &wakeup,
        )
        .await
        .map_err(|error| format!("authentication failed: {error}"))?;
        let command = run_ssh_exec_command(&mut handle, HOST_SYSTEM_INFO_SCRIPT).await;
        let _ = handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        let command = command?;
        if command.stdout.trim().is_empty() {
            return Err("system information command returned no data".to_owned());
        }
        Ok(parse_host_system_info(&command.stdout))
    }
    .await;
    let captured_events = events
        .lock()
        .map(|events| events.clone())
        .unwrap_or_default();
    match result {
        Ok(mut info) => {
            info.events = captured_events;
            info
        }
        Err(error) => HostSystemInfo {
            events: captured_events,
            error: Some(error),
            ..HostSystemInfo::default()
        },
    }
}

const SHELL_HISTORY_SCRIPT: &str = r#"
nauterm_shell=${SHELL##*/}
nauterm_shell_path=$SHELL
if [ -z "$nauterm_shell" ]; then
  nauterm_login_shell=$(getent passwd "$USER" 2>/dev/null | awk -F: 'NR == 1 { print $7 }')
  if [ -z "$nauterm_login_shell" ] && [ -r /etc/passwd ]; then
    nauterm_login_shell=$(awk -F: -v user="$USER" '$1 == user { print $7; exit }' /etc/passwd)
  fi
  nauterm_shell=${nauterm_login_shell##*/}
  nauterm_shell_path=$nauterm_login_shell
fi
printf '__NAUTERM_SHELL__%s\n' "$nauterm_shell"
case "$nauterm_shell" in
  zsh|bash)
    nauterm_history_probe=$(
      "$nauterm_shell_path" -ic \
        'printf "\n__NAUTERM_HISTFILE__%s\n" "${HISTFILE-}"' \
        2>/dev/null
    )
    nauterm_history_file=$(
      printf '%s\n' "$nauterm_history_probe" |
        sed -n 's/^__NAUTERM_HISTFILE__//p' |
        tail -n 1
    )
    if [ -z "$nauterm_history_file" ]; then
      case "$nauterm_shell" in
        zsh) nauterm_history_file=$HOME/.zsh_history ;;
        bash) nauterm_history_file=$HOME/.bash_history ;;
      esac
    fi
    tail -n 4000 "$nauterm_history_file" 2>/dev/null
    ;;
  fish) tail -n 4000 "${XDG_DATA_HOME:-$HOME/.local/share}/fish/fish_history" 2>/dev/null ;;
  ksh|ksh93|mksh) "$nauterm_shell_path" -ic 'fc -l -4000' 2>/dev/null ;;
  csh|tcsh) "$nauterm_shell_path" -ic 'history -h 4000' 2>/dev/null ;;
  nu) "$nauterm_shell_path" -c 'history --long | last 4000 | to json -r' 2>/dev/null ;;
  pwsh|powershell) tail -n 4000 "${XDG_DATA_HOME:-$HOME/.local/share}/powershell/PSReadLine/ConsoleHost_history.txt" 2>/dev/null ;;
esac
"#;

pub(super) fn shell_history_command() -> String {
    // An SSH exec request is parsed by the account's login shell. Run the
    // collector explicitly through POSIX sh so it also works when that login
    // shell is fish, csh, tcsh, or ksh.
    format!("sh -c {}", shell_quote(SHELL_HISTORY_SCRIPT))
}

pub(super) fn parse_shell_history_output(stdout: &str) -> Result<(Option<String>, String), String> {
    const HEADER: &str = "__NAUTERM_SHELL__";
    let marker_start = stdout
        .match_indices(HEADER)
        .find(|(index, _)| *index == 0 || stdout.as_bytes()[*index - 1] == b'\n')
        .map(|(index, _)| index)
        .ok_or_else(|| "shell history command returned no header".to_owned())?;
    let marked = &stdout[marker_start..];
    let (header, content) = marked.split_once('\n').unwrap_or((marked, ""));
    let shell = header
        .strip_prefix(HEADER)
        .map(str::trim)
        .filter(|shell| !shell.is_empty())
        .map(str::to_owned);
    if shell.is_none() {
        return Err("remote login shell could not be detected".to_owned());
    }
    Ok((shell, content.to_owned()))
}

pub(super) const EXPORT_PUBLIC_KEY_SCRIPT: &str = r#"if test ! -e "$1"; then
  mkdir -p "$1"
  chmod 700 "$1"
fi
if test ! -e "$1/$2"; then
  touch "$1/$2"
  chmod 600 "$1/$2"
fi
printf '%s\n' "$3" >> "$1/$2"
"#;

pub(super) async fn export_public_key(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    certificate: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<&SshProxyConfig>,
    public_key: &str,
    location: &str,
    filename: &str,
    script: &str,
    host_key_trust_mode: HostKeyTrustMode,
) -> SshPublicKeyExportResult {
    let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
    let result = async {
        let public_key = normalize_public_key_for_export(public_key)?;
        let location = validate_export_location(location)?;
        let filename = validate_export_filename(filename)?;
        let script = if script.trim().is_empty() {
            EXPORT_PUBLIC_KEY_SCRIPT.to_owned()
        } else {
            validate_export_script(script)?
        };
        let command = build_public_key_export_command(&script, &public_key, &location, &filename);
        let config = ssh_client_config();
        let output = Arc::new(Mutex::new(Vec::new()));
        let wakeup = Arc::new(Mutex::new(None));
        let handler = SshClientHandler {
            host: host.to_owned(),
            port,
            known_hosts_path: known_hosts_path.map(PathBuf::from),
            host_key_trust_mode,
            output,
            events: events.clone(),
            wakeup: wakeup.clone(),
        };
        let mut handle = connect_ssh_with_timeout(config, host, port, proxy, handler).await?;
        authenticate(
            &mut handle,
            username,
            private_key,
            certificate,
            passphrase,
            password,
            &events,
            &wakeup,
        )
        .await
        .map_err(|error| format!("authentication failed: {error}"))?;
        let command_result = run_ssh_exec_command(&mut handle, &command).await;
        let _ = handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        let command_result = command_result?;
        if command_result.exit_status != 0 {
            return Err(format!(
                "remote key export failed with exit status {}",
                command_result.exit_status
            ));
        }
        Ok(())
    }
    .await;
    let captured_events = events
        .lock()
        .map(|events| events.clone())
        .unwrap_or_default();
    match result {
        Ok(()) => SshPublicKeyExportResult {
            ok: true,
            events: captured_events,
            error: None,
        },
        Err(error) => SshPublicKeyExportResult {
            ok: false,
            events: captured_events,
            error: Some(error),
        },
    }
}

pub(super) fn normalize_public_key_for_export(value: &str) -> Result<String, String> {
    let lines = value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() != 1 {
        return Err("public key must contain exactly one non-empty line".to_owned());
    }
    let line = lines[0];
    let mut parts = line.split_whitespace();
    let key_type = parts.next().unwrap_or_default();
    let key_data = parts.next().unwrap_or_default();
    let supported_type = key_type.starts_with("ssh-")
        || key_type.starts_with("ecdsa-")
        || key_type.starts_with("sk-ssh-")
        || key_type.starts_with("sk-ecdsa-");
    if !supported_type || key_data.is_empty() {
        return Err("public key is not in OpenSSH authorized_keys format".to_owned());
    }
    Ok(line.to_owned())
}

pub(super) fn validate_export_location(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty()
        || value
            .chars()
            .any(|character| matches!(character, '\0' | '\r' | '\n'))
    {
        return Err("export location is invalid".to_owned());
    }
    Ok(value.to_owned())
}

pub(super) fn validate_export_filename(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty()
        || value == "."
        || value == ".."
        || value
            .chars()
            .any(|character| matches!(character, '\0' | '\r' | '\n' | '/' | '\\'))
    {
        return Err("export filename is invalid".to_owned());
    }
    Ok(value.to_owned())
}

pub(super) fn validate_export_script(value: &str) -> Result<String, String> {
    if value.trim().is_empty()
        || value
            .chars()
            .any(|character| matches!(character, '\0' | '\r'))
    {
        return Err("export script is invalid".to_owned());
    }
    Ok(value.to_owned())
}

pub(super) fn build_public_key_export_command(
    script: &str,
    public_key: &str,
    location: &str,
    filename: &str,
) -> String {
    format!(
        "sh -c {} nauterm-key-export {} {} {}",
        shell_quote(script),
        shell_quote(location),
        shell_quote(filename),
        shell_quote(public_key)
    )
}

pub(super) fn parse_host_system_info(output: &str) -> HostSystemInfo {
    let values = output
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect::<HashMap<_, _>>();
    let text = |key: &str| {
        values
            .get(key)
            .filter(|value| !value.is_empty())
            .map(|value| (*value).to_owned())
    };
    let unsigned = |key: &str| values.get(key).and_then(|value| value.parse::<u64>().ok());
    let decimal = |key: &str| values.get(key).and_then(|value| value.parse::<f64>().ok());
    let processes = output
        .lines()
        .filter_map(|line| line.strip_prefix("process="))
        .filter_map(|value| {
            let mut parts = value.splitn(3, '|');
            Some(HostProcessInfo {
                memory_bytes: parts.next()?.parse().ok()?,
                cpu_usage_percent: parts.next()?.parse().ok()?,
                command: parts.next()?.trim().to_owned(),
            })
        })
        .collect();
    let network_interfaces = output
        .lines()
        .filter_map(|line| line.strip_prefix("network="))
        .filter_map(|value| {
            let mut parts = value.splitn(3, '|');
            Some(HostNetworkInterface {
                name: parts.next()?.trim().to_owned(),
                received_bytes: parts.next()?.parse().ok()?,
                transmitted_bytes: parts.next()?.parse().ok()?,
            })
        })
        .collect();
    let filesystems = output
        .lines()
        .filter_map(|line| line.strip_prefix("filesystem="))
        .filter_map(|value| {
            let mut parts = value.splitn(3, '|');
            Some(HostFilesystemInfo {
                path: parts.next()?.trim().to_owned(),
                total_bytes: parts.next()?.parse().ok()?,
                used_bytes: parts.next()?.parse().ok()?,
            })
        })
        .collect();
    HostSystemInfo {
        hostname: text("hostname"),
        os_name: text("os_name"),
        kernel: text("kernel"),
        architecture: text("architecture"),
        uptime_seconds: unsigned("uptime_seconds"),
        load_average: decimal("load_average"),
        load_average_5: decimal("load_average_5"),
        load_average_15: decimal("load_average_15"),
        cpu_count: unsigned("cpu_count").and_then(|value| u32::try_from(value).ok()),
        cpu_usage_percent: decimal("cpu_usage_percent").map(|value| value.clamp(0.0, 100.0)),
        memory_total_bytes: unsigned("memory_total_bytes"),
        memory_used_bytes: unsigned("memory_used_bytes"),
        swap_total_bytes: unsigned("swap_total_bytes"),
        swap_used_bytes: unsigned("swap_used_bytes"),
        disk_total_bytes: unsigned("disk_total_bytes"),
        disk_used_bytes: unsigned("disk_used_bytes"),
        processes,
        network_interfaces,
        filesystems,
        ..HostSystemInfo::default()
    }
}

pub(super) async fn run_ssh_exec_command(
    handle: &mut client::Handle<SshClientHandler>,
    command: &str,
) -> Result<SshCommandOutput, String> {
    tokio::time::timeout(Duration::from_secs(6), async {
        let mut channel = handle
            .channel_open_session()
            .await
            .map_err(|error| format!("failed to open SSH command channel: {error}"))?;
        channel
            .exec(false, command)
            .await
            .map_err(|error| format!("failed to request SSH command: {error}"))?;

        let mut stdout = Vec::new();
        let mut exit_status = 0;
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                ChannelMsg::ExtendedData { .. } => {}
                ChannelMsg::ExitStatus {
                    exit_status: status,
                } => exit_status = status,
                ChannelMsg::Eof | ChannelMsg::Close => break,
                _ => {}
            }
        }
        let _ = channel.close().await;
        let decoded = String::from_utf8_lossy(&stdout).trim().to_owned();
        stdout.zeroize();
        Ok(SshCommandOutput {
            stdout: decoded,
            exit_status,
        })
    })
    .await
    .map_err(|_| "SSH command timed out.".to_owned())?
}

pub(super) fn command_stdout(output: &Option<SshCommandOutput>) -> String {
    output
        .as_ref()
        .filter(|output| output.exit_status == 0)
        .map(|output| first_non_empty_line(&output.stdout).unwrap_or_default())
        .unwrap_or_default()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DetectedHostPlatform {
    pub(super) os: String,
    pub(super) distro: Option<String>,
}

pub(super) fn detected_os_from_routeros(
    output: Option<&SshCommandOutput>,
) -> Option<DetectedHostPlatform> {
    let output = output?;
    if output.exit_status != 0 {
        return None;
    }
    let platform = first_non_empty_line(&output.stdout)?;
    let lower = platform.to_ascii_lowercase();
    if platform.starts_with(':') || lower.contains("not found") || lower.contains("unknown") {
        return None;
    }
    Some(DetectedHostPlatform {
        os: "routeros".to_owned(),
        distro: None,
    })
}

pub(super) fn detected_os_from_unix_probe(
    output: Option<&SshCommandOutput>,
) -> Option<DetectedHostPlatform> {
    let output = output?;
    if output.exit_status != 0 && output.stdout.trim().is_empty() {
        return None;
    }
    let stdout = output.stdout.trim();
    if stdout.is_empty() {
        return None;
    }
    if let Some(distro) = parse_os_release(Some(stdout)) {
        // Non-Linux OS detected via os-release
        let non_linux = ["android", "chromeos"];
        let os = if non_linux.contains(&distro.as_str()) {
            distro.clone()
        } else {
            "linux".to_owned()
        };
        return Some(DetectedHostPlatform {
            os,
            distro: Some(distro),
        });
    }
    let first = first_non_empty_line(stdout)?;
    let os = match first.to_ascii_lowercase().as_str() {
        "darwin" | "mac" | "macos" => "macos",
        "freebsd" => "freebsd",
        "netbsd" => "netbsd",
        "openbsd" => "openbsd",
        "dragonfly" | "dragonflybsd" => "dragonflybsd",
        "linux" => "linux",
        "sunos" | "solaris" => "solaris",
        "aix" => "aix",
        "hp-ux" | "hpux" => "hpux",
        "minix" => "minix",
        "haiku" => "haiku",
        _ => "unknown",
    };
    Some(DetectedHostPlatform {
        os: os.to_owned(),
        distro: None,
    })
}

pub(super) fn parse_os_release(contents: Option<&str>) -> Option<String> {
    let contents = contents?;
    let id = extract_os_release_value(contents, "ID=")
        .or_else(|| extract_os_release_value(contents, "DISTRIB_ID="));
    let lower = contents.to_ascii_lowercase();
    if id.is_none()
        && !lower.contains("linux")
        && !lower.contains("alpine")
        && !lower.contains("raspbian")
        && !lower.contains("ubuntu")
    {
        return None;
    }
    // Check for non-Linux OS first
    let id_lower = id.as_deref().unwrap_or_default().to_ascii_lowercase();
    if id_lower == "android" || lower.contains("android") {
        return Some("android".to_owned());
    }
    if id_lower == "chromeos" || lower.contains("chrome os") {
        return Some("chromeos".to_owned());
    }
    normalize_linux_distribution(id.as_deref(), contents)
}

pub(super) fn normalize_linux_distribution(id: Option<&str>, contents: &str) -> Option<String> {
    let id = id.unwrap_or_default().trim().to_ascii_lowercase();
    let normalized = match id.as_str() {
        "alpine" => "alpine",
        "alma" | "almalinux" => "alma",
        "amzn" | "amazon" | "amazonlinux" => "amazon",
        "arch" | "archlinux" => "arch",
        "centos" => "centos",
        "debian" => "debian",
        "fedora" => "fedora",
        "gentoo" => "gentoo",
        "kali" | "kalilinux" | "kali-linux" => "kali",
        "mageia" => "mageia",
        "manjaro" | "manjarolinux" => "manjaro",
        "mint" | "linuxmint" | "linux-mint" => "mint",
        "nixos" | "nix" => "nixos",
        "ol" | "oracle" | "oraclelinux" => "oracle",
        "pi" | "raspberry-pi" | "raspberrypi" => "pi",
        "pop" | "popos" | "pop-os" => "popos",
        "raspbian" => "raspbian",
        "redhat" | "rhel" => "redhat",
        "rocky" | "rockylinux" | "rocky-linux" => "rocky",
        "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" | "sled" | "sles" | "suse" => "suse",
        "ubuntu" => "ubuntu",
        "void" | "voidlinux" | "void-linux" => "void",
        "zorin" => "zorin",
        "immortalwrt" | "openwrt" => "openwrt",
        _ => {
            let lower = contents.to_ascii_lowercase();
            if lower.contains("alpine") {
                "alpine"
            } else if lower.contains("almalinux") || lower.contains("alma linux") {
                "alma"
            } else if lower.contains("amazon linux") {
                "amazon"
            } else if lower.contains("arch linux") {
                "arch"
            } else if lower.contains("rocky linux") || lower.contains("rocky") {
                "rocky"
            } else if lower.contains("centos") {
                "centos"
            } else if lower.contains("debian") {
                "debian"
            } else if lower.contains("fedora") {
                "fedora"
            } else if lower.contains("gentoo") {
                "gentoo"
            } else if lower.contains("kali") {
                "kali"
            } else if lower.contains("mageia") {
                "mageia"
            } else if lower.contains("manjaro") {
                "manjaro"
            } else if lower.contains("linux mint") || lower.contains("linuxmint") {
                "mint"
            } else if lower.contains("nixos") {
                "nixos"
            } else if lower.contains("oracle linux") {
                "oracle"
            } else if lower.contains("raspbian") {
                "raspbian"
            } else if lower.contains("raspberry pi") {
                "pi"
            } else if lower.contains("red hat") {
                "redhat"
            } else if lower.contains("opensuse") || lower.contains("suse linux") {
                "suse"
            } else if lower.contains("ubuntu") {
                "ubuntu"
            } else if lower.contains("void linux") {
                "void"
            } else if lower.contains("zorin") {
                "zorin"
            } else if lower.contains("immortalwrt") || lower.contains("openwrt") {
                "openwrt"
            } else {
                return Some("linux".to_owned());
            }
        }
    };
    Some(normalized.to_owned())
}

pub(super) fn extract_os_release_value(contents: &str, key: &str) -> Option<String> {
    let start = contents
        .match_indices(key)
        .find(|(index, _)| {
            *index == 0
                || contents[..*index]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace)
        })?
        .0
        + key.len();
    let rest = contents[start..].trim_start();
    if let Some(quoted) = rest.strip_prefix('"') {
        return quoted
            .split_once('"')
            .and_then(|(value, _)| unquote_os_release_value(value));
    }
    if let Some(quoted) = rest.strip_prefix('\'') {
        return quoted
            .split_once('\'')
            .and_then(|(value, _)| unquote_os_release_value(value));
    }
    let value = rest
        .split(|character: char| character.is_whitespace())
        .next()
        .unwrap_or("");
    unquote_os_release_value(value)
}

pub(super) fn unquote_os_release_value(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('"').trim_matches('\'').trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

pub(super) fn first_non_empty_line(value: &str) -> Option<String> {
    value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}
