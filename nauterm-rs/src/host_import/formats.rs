//! Import the same concrete session fields as the Flutter reference. No SSH
//! directives, registry commands or terminal application scripts are executed.
use super::*;
use regex::Regex;

type Values = BTreeMap<String, String>;
fn candidate(
    name: String,
    address: String,
    values: &Values,
    group: Option<&str>,
    key: &str,
) -> Candidate {
    let port = values
        .get("port")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let parsed = port.map(str::parse::<u16>).transpose();
    let issue = if address.trim().is_empty() || address.chars().any(char::is_whitespace) {
        Some("Address must be nonempty and contain no whitespace.".into())
    } else if parsed.as_ref().is_err() || matches!(parsed, Ok(Some(0))) {
        Some("Port must be between 1 and 65535.".into())
    } else {
        None
    };
    Candidate {
        name: if name.trim().is_empty() {
            address.clone()
        } else {
            name
        },
        address,
        port: parsed.ok().flatten().unwrap_or(22),
        protocol: "ssh".into(),
        username: values.get("user").cloned().unwrap_or_default(),
        password: Zeroizing::new(String::new()),
        groups: group.map(groups).unwrap_or_default(),
        tags: vec![],
        identity_file: values
            .get(key)
            .filter(|value| !value.trim().is_empty())
            .cloned(),
        issue,
    }
}
fn groups(value: &str) -> Vec<String> {
    let mut parts: Vec<_> = value
        .split(['/', '\\'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    if parts.len() > 2 {
        parts = vec![parts[0].clone(), parts[1..].join(" / ")];
    }
    parts
}
fn words(input: &str) -> Vec<String> {
    let mut words = vec![];
    let mut word = String::new();
    let mut quote = None;
    for ch in input.chars() {
        if Some(ch) == quote {
            quote = None;
        } else if quote.is_none() && matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if ch == '#' && quote.is_none() {
            break;
        } else if ch.is_whitespace() && quote.is_none() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(ch);
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}
pub fn openssh(input: &str) -> Vec<Candidate> {
    let mut global = Values::new();
    let mut blocks: Vec<(Vec<String>, Values)> = vec![];
    let mut matched = false;
    for line in input.lines() {
        let line = line.trim();
        let Some(end) = line.find(|ch: char| ch.is_whitespace() || ch == '=') else {
            continue;
        };
        let key = line[..end].to_ascii_lowercase();
        let value = line[end..]
            .trim_start()
            .trim_start_matches('=')
            .trim_start();
        if key == "host" {
            matched = false;
            blocks.push((words(value), Values::new()));
        } else if key == "match" {
            matched = true;
        } else if !matched && matches!(key.as_str(), "hostname" | "port" | "user" | "identityfile")
        {
            let value = words(value).join(" ");
            if !value.is_empty() {
                match blocks.last_mut() {
                    Some((_, values)) => {
                        values.entry(key).or_insert(value);
                    }
                    None => {
                        global.entry(key).or_insert(value);
                    }
                }
            }
        }
    }
    let mut defaults = Values::new();
    for (_, values) in blocks
        .iter()
        .filter(|(aliases, _)| aliases.iter().any(|alias| alias == "*"))
    {
        for (key, value) in values {
            defaults.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    let mut output = vec![];
    for (aliases, values) in blocks {
        for alias in aliases.into_iter().filter(|alias| {
            !alias.starts_with('!') && !alias.contains(['*', '?']) && !alias.is_empty()
        }) {
            let mut merged = values.clone();
            for source in [&global, &defaults] {
                for (key, value) in source {
                    merged.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
            output.push(candidate(
                alias.clone(),
                merged.get("hostname").cloned().unwrap_or(alias),
                &merged,
                None,
                "identityfile",
            ));
        }
    }
    output
}
pub fn known_hosts(input: &str) -> Vec<Candidate> {
    let mut output = vec![];
    for line in input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let fields: Vec<_> = line.split_whitespace().collect();
        let index = usize::from(fields.first().is_some_and(|field| field.starts_with('@')));
        if fields.len() <= index + 1 {
            continue;
        }
        for pattern in fields[index].split(',').filter(|pattern| {
            !pattern.is_empty() && !pattern.starts_with(['|', '!']) && !pattern.contains(['*', '?'])
        }) {
            let (host, port) = if let Some(bracketed) = pattern.strip_prefix('[') {
                let Some((host, port)) = bracketed.rsplit_once("]:") else {
                    continue;
                };
                (host, port)
            } else {
                (pattern, "22")
            };
            let values = Values::from([("port".into(), port.into())]);
            let row = candidate(host.into(), host.into(), &values, None, "identityfile");
            if row.issue.is_none() {
                output.push(row);
            }
        }
    }
    output
}
fn registry_name(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = vec![];
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && bytes[index + 1].is_ascii_hexdigit()
            && bytes[index + 2].is_ascii_hexdigit()
            && let Ok(byte) = u8::from_str_radix(&value[index + 1..index + 3], 16)
        {
            decoded.push(byte);
            index += 3;
        } else {
            decoded.push(if bytes[index] == b'+' {
                b' '
            } else {
                bytes[index]
            });
            index += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
pub fn putty(input: &str) -> Vec<Candidate> {
    let section = Regex::new(r"(?i)^\[.*\\PuTTY\\Sessions\\([^\]]+)\]$").unwrap();
    let string = Regex::new(r#"^"([^"]+)"="(.*)"$"#).unwrap();
    let dword = Regex::new(r#"(?i)^"([^"]+)"=dword:([0-9a-f]+)$"#).unwrap();
    let mut values = Values::new();
    let mut name = None;
    let mut output = vec![];
    let flush = |name: &Option<String>, values: &Values, output: &mut Vec<Candidate>| {
        if let (Some(name), Some(host)) = (
            name,
            values
                .get("hostname")
                .filter(|host| !host.trim().is_empty()),
        ) {
            let mut mapped = values.clone();
            if let Some(port) = values.get("portnumber") {
                mapped.insert("port".into(), port.clone());
            }
            if let Some(user) = values.get("username") {
                mapped.insert("user".into(), user.clone());
            }
            output.push(candidate(
                registry_name(name),
                host.trim().into(),
                &mapped,
                None,
                "publickeyfile",
            ));
        }
    };
    for line in input.lines().map(str::trim) {
        if line.starts_with('[') {
            flush(&name, &values, &mut output);
            name = section
                .captures(line)
                .map(|captures| captures[1].to_owned());
            values.clear();
        } else if let Some(captures) = string.captures(line) {
            let key = captures[1].to_ascii_lowercase();
            if !matches!(
                key.as_str(),
                "hostname" | "portnumber" | "username" | "publickeyfile"
            ) {
                continue;
            }
            values.insert(key, captures[2].replace(r"\\", r"\").replace(r#"\""#, "\""));
        } else if let Some(captures) = dword.captures(line)
            && let Ok(value) = u32::from_str_radix(&captures[2], 16)
        {
            values.insert(captures[1].to_ascii_lowercase(), value.to_string());
        }
    }
    flush(&name, &values, &mut output);
    output
}
pub fn moba(input: &str) -> Vec<Candidate> {
    let mut group = None;
    let mut output = vec![];
    for line in input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with(';'))
    {
        if let Some(section) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            group = section
                .to_ascii_lowercase()
                .starts_with("bookmarks")
                .then(|| section.to_owned());
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        let value = value.trim();
        if name.eq_ignore_ascii_case("subrep") {
            group = (!value.is_empty()).then(|| value.into());
            continue;
        }
        if !value.starts_with("#109#") {
            continue;
        }
        let fields: Vec<_> = value.split('%').collect();
        if fields.len() < 2 || fields[1].trim().is_empty() {
            continue;
        }
        let values = Values::from([
            ("port".into(), fields.get(2).unwrap_or(&"").trim().into()),
            ("user".into(), fields.get(3).unwrap_or(&"").trim().into()),
        ]);
        output.push(candidate(
            name.into(),
            fields[1].trim().into(),
            &values,
            group
                .as_deref()
                .filter(|group| !group.eq_ignore_ascii_case("bookmarks")),
            "identityfile",
        ));
    }
    output
}
pub fn securecrt(input: &str, name: String, group: Option<&str>) -> Option<Candidate> {
    let string = Regex::new(r#"^S:"([^"]+)"=(.*)$"#).unwrap();
    let dword = Regex::new(r#"(?i)^D:"([^"]+)"=([0-9a-f]+)$"#).unwrap();
    let mut values = Values::new();
    for line in input.lines().map(str::trim) {
        if let Some(captures) = string.captures(line) {
            let key = captures[1].to_ascii_lowercase();
            if matches!(
                key.as_str(),
                "hostname" | "port" | "[ssh2] port" | "username" | "identity filename v2"
            ) {
                values.insert(key, captures[2].trim().into());
            }
        } else if let Some(captures) = dword.captures(line)
            && matches!(
                captures[1].to_ascii_lowercase().as_str(),
                "port" | "[ssh2] port"
            )
            && let Ok(value) = u32::from_str_radix(&captures[2], 16)
        {
            values.insert(captures[1].to_ascii_lowercase(), value.to_string());
        }
    }
    let host = values
        .get("hostname")
        .filter(|value| !value.is_empty())?
        .clone();
    if let Some(port) = values.get("[ssh2] port").cloned() {
        values.insert("port".into(), port);
    }
    if let Some(user) = values.get("username").cloned() {
        values.insert("user".into(), user);
    }
    Some(candidate(
        name,
        host,
        &values,
        group,
        "identity filename v2",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn openssh_imports_concrete_aliases_with_defaults_and_quoted_key_paths() {
        let rows = openssh(
            "User default-user\nHost *\n Port 2022\nHost staging stage-alias\n HostName stage.example.test\n User deploy\n Port=2222\n IdentityFile \"./id # work\" # comment\nHost backup\n HostName backup.test\nMatch exec touch-no-file\n HostName wrong.test\nHost !excluded *.test\n User ignored\n",
        );
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].name, "staging");
        assert_eq!(rows[0].address, "stage.example.test");
        assert_eq!(rows[0].port, 2222);
        assert_eq!(rows[0].username, "deploy");
        assert_eq!(rows[0].identity_file.as_deref(), Some("./id # work"));
        assert_eq!(rows[2].address, "backup.test");
        assert_eq!(rows[2].port, 2022);
        assert_eq!(rows[2].username, "default-user");
    }
    #[test]
    fn known_hosts_import_skips_hashes_patterns_and_invalid_ports() {
        let rows = known_hosts(
            "github.test,192.0.2.1 ssh-ed25519 AAAA\n[git.test]:2222 ssh-rsa BBBB\n|1|hash|value ssh-ed25519 CCCC\n@cert-authority *.test ssh-ed25519 DDDD\n[::1]:2022 ssh-ed25519 EEEE\n[bad.test]:0 ssh-ed25519 FFFF\n",
        );
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[2].port, 2222);
        assert_eq!(rows[3].address, "::1");
        assert_eq!(rows[3].port, 2022);
    }
    #[test]
    fn putty_decodes_registry_names_paths_and_hexadecimal_ports() {
        let rows = putty(
            r#"Windows Registry Editor Version 5.00
[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\Production%20SSH]
"HostName"="prod.test"
"PortNumber"=dword:000008ae
"UserName"="root"
"PublicKeyFile"="C:\\Users\\me\\id.ppk"
"Password"="SHOULD_NOT_IMPORT"
[HKEY_CURRENT_USER\Other]
"HostName"="wrong.test"
"#,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Production SSH");
        assert_eq!(rows[0].port, 2222);
        assert_eq!(
            rows[0].identity_file.as_deref(),
            Some(r"C:\Users\me\id.ppk")
        );
        assert!(rows[0].password.is_empty());
        assert_eq!(registry_name("%你+%E6%B5%8B%E8%AF%95"), "%你 测试");
    }
    #[test]
    fn terminal_exports_preserve_groups_and_block_invalid_endpoints() {
        let rows = moba(
            "[Bookmarks_1]\nSubRep=Production/Apps/Backend\nProd=#109#0%prod.test%2200%deploy%%-1%-1\nDesktop=#91#0%desktop.test%3389%user\n",
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].port, 2200);
        assert_eq!(rows[0].groups, ["Production", "Apps / Backend"]);
        let row=securecrt("S:\"Hostname\"=router.test\nD:\"[SSH2] Port\"=00000016\nS:\"Username\"=network\nS:\"Identity Filename V2\"=./id_ed25519\n", "Router".into(), Some("Network/Core")).unwrap();
        assert_eq!(row.port, 22);
        assert_eq!(row.username, "network");
        assert_eq!(row.groups, ["Network", "Core"]);
        assert!(
            securecrt(
                "S:\"Hostname\"=bad host\nD:\"Port\"=00010000\n",
                "Bad".into(),
                None
            )
            .unwrap()
            .issue
            .is_some()
        );
    }
}
