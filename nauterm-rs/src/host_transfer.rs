//! Reference Hosts CSV format. Passwords are transient and never serializable.
use crate::model::{AppState, Group, Host, HostKind, Id};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::Path,
};
use zeroize::Zeroizing;

const MAX_BYTES: u64 = 16 * 1024 * 1024;
const HEADERS: [&str; 8] = [
    "Label",
    "Hostname/IP",
    "Protocol",
    "Port",
    "Username",
    "Password",
    "Groups",
    "Tags",
];
#[derive(Clone)]
pub struct Candidate {
    pub name: String,
    pub address: String,
    pub protocol: String,
    pub port: u16,
    pub username: String,
    pub password: Zeroizing<String>,
    pub groups: Vec<String>,
    pub tags: Vec<String>,
    pub issue: Option<String>,
    pub identity_file: Option<String>,
}
impl Candidate {
    pub fn matches(&self, query: &str) -> bool {
        let text = format!(
            "{} {} {} {} {}",
            self.name,
            self.address,
            self.username,
            self.groups.join("/"),
            self.tags.join(",")
        )
        .to_lowercase();
        query
            .split_whitespace()
            .all(|part| text.contains(&part.to_lowercase()))
    }
}
fn rows(input: &str) -> Result<Zeroizing<Vec<Vec<String>>>> {
    ensure!(
        input.len() as u64 <= MAX_BYTES && !input.contains('\0'),
        "Choose a UTF-8 CSV of at most 16 MiB without NUL bytes."
    );
    let mut rows = Zeroizing::new(Vec::new());
    let mut row = Zeroizing::new(Vec::new());
    let mut field = Zeroizing::new(String::new());
    let mut input = input.trim_start_matches('\u{feff}').chars().peekable();
    let mut quoted = false;
    let mut closed = false;
    while let Some(ch) = input.next() {
        if quoted {
            if ch == '"' {
                if input.peek() == Some(&'"') {
                    input.next();
                    field.push('"');
                } else {
                    quoted = false;
                    closed = true;
                }
            } else {
                field.push(ch);
            }
            continue;
        }
        ensure!(
            !closed || matches!(ch, ',' | '\r' | '\n' | ' ' | '\t'),
            "Unexpected text after a quoted CSV field."
        );
        match ch {
            '"' => {
                ensure!(field.is_empty(), "Quotes in CSV fields must be escaped.");
                quoted = true;
            }
            ',' | '\r' | '\n' => {
                row.push(std::mem::take(&mut *field));
                closed = false;
                ensure!(row.len() <= 256, "CSV has too many columns.");
                if ch != ',' {
                    if ch == '\r' && input.peek() == Some(&'\n') {
                        input.next();
                    }
                    if row.iter().any(|value: &String| !value.trim().is_empty()) {
                        rows.push(std::mem::take(&mut *row));
                    } else {
                        row.clear();
                    }
                    ensure!(
                        rows.len() <= 10001,
                        "Import at most 10,000 hosts at a time."
                    );
                }
            }
            _ => field.push(ch),
        }
    }
    ensure!(!quoted, "CSV has an unterminated quoted field.");
    row.push(std::mem::take(&mut *field));
    ensure!(row.len() <= 256, "CSV has too many columns.");
    if row.iter().any(|value| !value.trim().is_empty()) {
        rows.push(std::mem::take(&mut *row));
    }
    ensure!(
        rows.len() <= 10001,
        "Import at most 10,000 hosts at a time."
    );
    Ok(rows)
}
pub fn parse(input: &str) -> Result<Vec<Candidate>> {
    let rows = rows(input)?;
    let Some(headers) = rows.first() else {
        return Ok(vec![]);
    };
    let headers: BTreeMap<_, _> = headers
        .iter()
        .enumerate()
        .map(|(index, name)| (name.trim().to_lowercase(), index))
        .collect();
    ensure!(
        headers.contains_key("hostname/ip"),
        "CSV must contain a Hostname/IP column."
    );
    let mut candidates = Vec::new();
    for row in rows.iter().skip(1) {
        let value = |name: &str| {
            headers
                .get(name)
                .and_then(|index| row.get(*index))
                .map(String::as_str)
                .unwrap_or("")
        };
        let address = value("hostname/ip").trim();
        if address.is_empty() {
            continue;
        }
        let protocol = if value("protocol").trim().is_empty() {
            "ssh".into()
        } else {
            value("protocol").trim().to_lowercase()
        };
        let port = if value("port").trim().is_empty() {
            Ok(if protocol == "telnet" { 23 } else { 22 })
        } else {
            value("port").trim().parse::<u16>()
        };
        let mut issue = if !matches!(protocol.as_str(), "ssh" | "telnet") {
            Some(format!("Unsupported protocol: {protocol}"))
        } else if !matches!(port, Ok(1..=65535)) {
            Some("Port must be between 1 and 65535.".into())
        } else if address.chars().any(char::is_whitespace) {
            Some("Address must not contain whitespace.".into())
        } else {
            None
        };
        if crate::host_password::validate(value("password")).is_err() {
            issue = Some("Password is invalid or too large.".into());
        }
        let mut groups: Vec<_> = value("groups")
            .split(['/', '\\'])
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect();
        // Reference import folds deeper source paths into its two-level tree.
        if groups.len() > 2 {
            groups = vec![groups[0].clone(), groups[1..].join(" / ")];
        }
        candidates.push(Candidate {
            identity_file: None,
            name: if value("label").trim().is_empty() {
                address.into()
            } else {
                value("label").trim().into()
            },
            address: address.into(),
            protocol,
            port: port.unwrap_or(22),
            username: value("username").trim().into(),
            password: Zeroizing::new(value("password").into()),
            groups,
            tags: value("tags")
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(str::to_owned)
                .collect(),
            issue,
        });
    }
    Ok(candidates)
}
pub fn read(path: &Path) -> Result<Vec<Candidate>> {
    parse(&crate::host_import::read_text(path, MAX_BYTES)?)
}

type Signature = (String, String, u16, String, String);
fn signature(protocol: &str, name: &str, address: &str, port: u16, username: &str) -> Signature {
    (
        protocol.into(),
        address.to_lowercase(),
        port,
        username.into(),
        name.to_lowercase(),
    )
}
pub struct Imported {
    pub data: AppState,
    pub ids: Vec<Id>,
    pub skipped: usize,
    pub key_bindings: BTreeMap<Id, String>,
}
pub fn merge(
    data: &AppState,
    rows: &[Candidate],
    passwords: &[Option<String>],
) -> Result<Imported> {
    ensure!(
        rows.len() == passwords.len(),
        "Imported password references do not match the selected rows."
    );
    let mut data = data.clone();
    let mut signatures: BTreeSet<_> = data
        .hosts
        .iter()
        .flat_map(|host| {
            let ssh = host
                .supports_ssh()
                .then(|| signature("ssh", &host.name, &host.address, host.port, &host.username));
            let telnet = host
                .is_remote()
                .then_some(host.connections.telnet.as_ref())
                .flatten()
                .map(|profile| {
                    signature(
                        "telnet",
                        &host.name,
                        &host.address,
                        profile.port,
                        &profile.username,
                    )
                });
            ssh.into_iter().chain(telnet)
        })
        .collect();
    let mut tags: BTreeMap<_, _> = data
        .hosts
        .iter()
        .flat_map(|host| &host.tags)
        .map(|tag| (tag.to_lowercase(), tag.clone()))
        .collect();
    for tag in &data.tags {
        tags.entry(tag.name.to_lowercase())
            .or_insert_with(|| tag.name.clone());
    }
    let mut ids = vec![];
    let mut key_bindings = BTreeMap::new();
    let mut skipped = 0;
    for (row, password) in rows.iter().zip(passwords) {
        ensure!(
            row.issue.is_none()
                && matches!(row.protocol.as_str(), "ssh" | "telnet")
                && row.port != 0,
            "An unavailable host was selected for import."
        );
        if !signatures.insert(signature(
            &row.protocol,
            &row.name,
            &row.address,
            row.port,
            &row.username,
        )) {
            skipped += 1;
            continue;
        }
        ensure!(
            row.password.is_empty() || password.is_some(),
            "An imported password has not been saved to the encrypted database."
        );
        let mut parent = None;
        for name in &row.groups {
            parent = Some(
                if let Some(group) = data.groups.iter().find(|group| {
                    group.parent == parent && group.name.to_lowercase() == name.to_lowercase()
                }) {
                    group.id
                } else {
                    ensure!(data.next_id < u64::MAX - 1, "No workspace IDs remain.");
                    let id = data.allocate_id();
                    data.groups.push(Group {
                        mosh: None,
                        telnet: None,
                        id,
                        uuid: None,
                        name: name.clone(),
                        parent,
                        proxy_id: None,
                        ssh: None,
                    });
                    id
                },
            );
        }
        ensure!(data.next_id < u64::MAX - 1, "No workspace IDs remain.");
        let id = data.allocate_id();
        let mut seen_tags = BTreeSet::new();
        let mut host = Host {
            id,
            name: row.name.clone(),
            address: row.address.clone(),
            port: row.port,
            username: row.username.clone(),
            group_id: parent,
            tags: row
                .tags
                .iter()
                .filter_map(|tag| {
                    let key = tag.to_lowercase();
                    seen_tags
                        .insert(key.clone())
                        .then(|| tags.entry(key).or_insert_with(|| tag.clone()).clone())
                })
                .collect(),
            ssh: crate::host_session::Setup {
                password_id: password.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        if row.protocol == "telnet" {
            host.connections = crate::model::ConnectionProfiles {
                mosh_command: crate::mosh::DEFAULT_COMMAND.into(),
                ssh: false,
                mosh: false,
                telnet: Some(crate::telnet::Profile {
                    port: row.port,
                    username: row.username.clone(),
                    password_id: password.clone(),
                    ..Default::default()
                }),
            };
            host.port = 22;
            host.username.clear();
            host.ssh = Default::default();
        }
        if row.protocol == "ssh"
            && let Some(path) = &row.identity_file
        {
            key_bindings.insert(id, path.clone());
        }
        data.hosts.push(host);
        ids.push(id);
    }
    crate::tags::materialize(&mut data);
    data.validate()?;
    Ok(Imported {
        data,
        ids,
        skipped,
        key_bindings,
    })
}
pub struct Export {
    pub hosts: Vec<Host>,
    groups: Vec<Group>,
}
impl Export {
    pub fn new(data: &AppState) -> Self {
        Self {
            hosts: data
                .hosts
                .iter()
                .filter(|host| host.is_remote())
                .cloned()
                .collect(),
            groups: data.groups.clone(),
        }
    }
    pub fn password_ids(&self) -> Vec<String> {
        self.hosts
            .iter()
            .filter_map(|host| export_login(host).3.cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}
// Match the reference CSV's Telnet preference for mixed-protocol Hosts, while
// exporting Telnet's own port rather than the hidden SSH port.
fn export_login(host: &Host) -> (&str, u16, &str, Option<&String>) {
    if let Some(profile) = &host.connections.telnet {
        (
            "telnet",
            profile.port,
            &profile.username,
            profile.password_id.as_ref(),
        )
    } else {
        (
            "ssh",
            host.port,
            &host.username,
            host.ssh.password_id.as_ref(),
        )
    }
}
fn group_path(groups: &[Group], group: Option<Id>) -> String {
    let mut group = group;
    let mut names = vec![];
    let mut seen = BTreeSet::new();
    while let Some(current) = group.and_then(|id| groups.iter().find(|group| group.id == id)) {
        if !seen.insert(current.id) {
            break;
        }
        names.push(current.name.as_str());
        group = current.parent;
    }
    names.reverse();
    names.join("/")
}
fn write_row(output: &mut String, fields: &[&str]) {
    if !output.is_empty() {
        output.push_str("\r\n");
    }
    for (index, value) in fields.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        if value.contains([',', '"', '\r', '\n']) {
            output.push('"');
            for ch in value.chars() {
                if ch == '"' {
                    output.push('"');
                }
                output.push(ch);
            }
            output.push('"');
        } else {
            output.push_str(value);
        }
    }
}
pub fn csv(
    data: &Export,
    passwords: &BTreeMap<String, Zeroizing<String>>,
) -> Result<Zeroizing<String>> {
    let mut output = Zeroizing::new(String::new());
    write_row(&mut output, &HEADERS);
    for host in data
        .hosts
        .iter()
        .filter(|host| host.kind == HostKind::Remote)
    {
        let (protocol, port, username, password_id) = export_login(host);
        let password = match password_id {
            Some(id) => passwords
                .get(id)
                .context("An exported Host password is unavailable.")?
                .as_str(),
            None => "",
        };
        write_row(
            &mut output,
            &[
                &host.name,
                &host.address,
                protocol,
                &port.to_string(),
                username,
                password,
                &group_path(&data.groups, host.group_id),
                &host.tags.join(","),
            ],
        );
    }
    Ok(output)
}
pub fn write(path: &Path, contents: &str) -> Result<()> {
    let target = crate::file_commit::DownloadTarget::new(path, true)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(target.staged())?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    target.publish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_and_telnet_import_keep_protocols_and_encrypted_password_references_distinct()
    -> Result<()> {
        let rows = parse(concat!(
            "Label,Hostname/IP,Protocol,Port,Username,Password,Groups,Tags\n",
            "Both,example.test,ssh,2222,user,SYNTHETIC_SSH,prod,linux\n",
            "Both,example.test,telnet,2222,user,SYNTHETIC_TELNET,prod,linux\n"
        ))?;
        assert!(rows.iter().all(|row| row.issue.is_none()));
        let references = vec![
            Some("11111111111111111111111111111111".into()),
            Some("22222222222222222222222222222222".into()),
        ];
        let imported = merge(&AppState::default(), &rows, &references)?;
        assert_eq!(imported.ids.len(), 2);
        let ssh = &imported.data.hosts[0];
        assert!(ssh.supports_ssh());
        assert_eq!(ssh.ssh.password_id, references[0]);
        assert!(ssh.connections.telnet.is_none());
        let telnet = &imported.data.hosts[1];
        assert!(!telnet.supports_ssh());
        assert!(telnet.ssh.password_id.is_none());
        let profile = telnet.connections.telnet.as_ref().unwrap();
        assert_eq!(profile.port, 2222);
        assert_eq!(profile.username, "user");
        assert_eq!(profile.password_id, references[1]);
        let repeated = merge(&imported.data, &rows, &references)?;
        assert_eq!(repeated.skipped, 2);
        assert!(repeated.ids.is_empty());
        assert!(!serde_json::to_string(&imported.data)?.contains("SYNTHETIC_"));
        let defaults =
            parse("Hostname/IP,Protocol,Port\ndefault.test,telnet,\nunsupported.test,mosh,\n")?;
        assert_eq!(defaults[0].port, 23);
        assert!(defaults[0].issue.is_none());
        assert!(defaults[1].issue.is_some());
        Ok(())
    }
    #[test]
    fn export_uses_telnet_login_and_port_for_mixed_protocol_hosts() -> Result<()> {
        let ssh_password = "11111111111111111111111111111111".to_owned();
        let telnet_password = "22222222222222222222222222222222".to_owned();
        let data = AppState {
            hosts: vec![Host {
                id: 1,
                name: "Both".into(),
                address: "example.test".into(),
                port: 2022,
                username: "ssh-user".into(),
                ssh: crate::host_session::Setup {
                    password_id: Some(ssh_password),
                    ..Default::default()
                },
                connections: crate::model::ConnectionProfiles {
                    telnet: Some(crate::telnet::Profile {
                        port: 2323,
                        username: "telnet-user".into(),
                        password_id: Some(telnet_password.clone()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let export = Export::new(&data);
        assert_eq!(export.password_ids(), vec![telnet_password.clone()]);
        let encoded = csv(
            &export,
            &BTreeMap::from([(telnet_password, Zeroizing::new("SYNTHETIC_TELNET".into()))]),
        )?;
        let rows = parse(&encoded)?;
        assert_eq!(rows[0].protocol, "telnet");
        assert_eq!(rows[0].port, 2323);
        assert_eq!(rows[0].username, "telnet-user");
        assert_eq!(rows[0].password.as_str(), "SYNTHETIC_TELNET");
        Ok(())
    }
    #[test]
    fn reference_csv_preserves_quoted_passwords_and_marks_unavailable_rows() -> Result<()> {
        let input = concat!(
            "\u{feff}Label,Hostname/IP,Protocol,Port,Username,Password,Groups,Tags\r\n",
            "\"Build, \"\"A\"\"\",example.invalid,SSH,2022,Admin,\"  SECRET,\"\"x\"\"\r\ny  \",Prod/Apps/CI,\"linux, Build\"\r\n",
            ",fallback.invalid,,,,,,\r\n",
            "Other,telnet.invalid,telnet,23,,,,\r\n",
            "Bad port,bad.invalid,ssh,0,,,,\r\n",
        );
        let rows = parse(input)?;
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].name, "Build, \"A\"");
        assert_eq!(rows[0].password.as_str(), "  SECRET,\"x\"\r\ny  ");
        assert_eq!(rows[0].groups, ["Prod", "Apps / CI"]);
        assert_eq!(rows[0].tags, ["linux", "Build"]);
        assert!(rows[0].matches("admin CI build"));
        assert!(!rows[0].matches("SECRET"));
        assert_eq!(
            (rows[1].name.as_str(), rows[1].port),
            ("fallback.invalid", 22)
        );
        assert!(rows[0].issue.is_none() && rows[1].issue.is_none());
        assert!(rows[2].issue.is_none());
        assert!(rows[3].issue.is_some());
        for input in [
            "Label\nmissing",
            "Hostname/IP\n\"unclosed",
            "Hostname/IP\n\"host\"oops",
            "Hostname/IP\na\"b",
            "Hostname/IP\nnull\0",
        ] {
            assert!(parse(input).is_err());
        }
        Ok(())
    }

    #[test]
    fn import_deduplicates_and_merges_public_metadata_without_inheriting_credentials() -> Result<()>
    {
        let mut data = AppState {
            groups: vec![Group {
                mosh: None,
                telnet: None,
                uuid: None,
                id: 2,
                name: "PROD".into(),
                parent: None,
                proxy_id: Some(7),
                ssh: None,
            }],
            next_id: 4,
            ..Default::default()
        };
        data.hosts.push(Host {
            id: 3,
            name: "EXISTING".into(),
            address: "EXAMPLE.INVALID".into(),
            port: 22,
            username: "Admin".into(),
            tags: vec!["Linux".into()],
            ..Default::default()
        });
        let input = concat!(
            "Label,Hostname/IP,Protocol,Port,Username,Password,Groups,Tags\n",
            "existing,example.invalid,ssh,22,Admin,DUPLICATE,prod,linux\n",
            "existing,example.invalid,ssh,22,admin,SECRET,prod/Apps/CI,\"linux,Build,build\"\n"
        );
        let rows = parse(input)?;
        let duplicate_ref = "11".repeat(16);
        let used_ref = "22".repeat(16);
        let before = serde_json::to_value(&data)?;
        let result = merge(
            &data,
            &rows,
            &[Some(duplicate_ref.clone()), Some(used_ref.clone())],
        )?;
        assert_eq!(result.skipped, 1);
        assert_eq!(result.ids, [5]);
        assert_eq!(result.data.groups.len(), 2);
        assert_eq!(result.data.groups[1].name, "Apps / CI");
        assert_eq!(result.data.groups[1].parent, Some(2));
        let host = &result.data.hosts[1];
        assert_eq!(host.group_id, Some(4));
        assert_eq!(host.tags, ["Linux", "Build"]);
        assert!(host.credential.is_none() && host.proxy_id.is_none());
        assert_eq!(host.ssh.password_id.as_ref(), Some(&used_ref));
        let metadata = serde_json::to_string(&result.data)?;
        assert!(
            !metadata.contains("SECRET")
                && !metadata.contains("DUPLICATE")
                && !metadata.contains(&duplicate_ref)
        );
        assert!(merge(&data, &rows, &[None, None]).is_err());
        assert_eq!(serde_json::to_value(&data)?, before);
        Ok(())
    }

    #[test]
    fn export_roundtrips_reference_columns_and_atomically_replaces_selected_file() -> Result<()> {
        let password = Zeroizing::new("  SYNTHETIC,\"PASSWORD\"\n汉字  ".to_owned());
        let id = "aa".repeat(16);
        let data = AppState {
            hosts: vec![
                Host {
                    id: 2,
                    name: "SSH, \"host\"".into(),
                    address: "example.invalid".into(),
                    port: 2022,
                    username: "Admin".into(),
                    group_id: Some(3),
                    tags: vec!["Linux".into(), "Build".into()],
                    ssh: crate::host_session::Setup {
                        password_id: Some(id.clone()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Host {
                    id: 4,
                    kind: HostKind::Local,
                    name: "Local fixture".into(),
                    ..Default::default()
                },
            ],
            groups: vec![Group {
                mosh: None,
                telnet: None,
                uuid: None,
                id: 3,
                name: "Prod".into(),
                parent: None,
                proxy_id: None,
                ssh: None,
            }],
            next_id: 5,
            ..Default::default()
        };
        let export = Export::new(&data);
        assert_eq!(export.hosts.len(), 1);
        assert_eq!(export.password_ids().as_slice(), std::slice::from_ref(&id));
        assert!(csv(&export, &BTreeMap::new()).is_err());
        let csv = csv(&export, &BTreeMap::from([(id, password.clone())]))?;
        assert!(
            csv.starts_with("Label,Hostname/IP,Protocol,Port,Username,Password,Groups,Tags\r\n")
        );
        let rows = parse(&csv)?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, data.hosts[0].name);
        assert_eq!(rows[0].password, password);
        assert_eq!(rows[0].groups, ["Prod"]);
        assert_eq!(rows[0].tags, ["Linux", "Build"]);
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("chosen.csv");
        write(&path, &csv)?;
        assert_eq!(read(&path)?[0].password.as_str(), password.as_str());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path)?.permissions().mode() & 0o777,
                0o600
            );
        }
        write(&path, "Hostname/IP\r\nreplacement.invalid")?;
        assert_eq!(read(&path)?[0].address, "replacement.invalid");
        assert_eq!(std::fs::read_dir(temp.path())?.count(), 1);
        Ok(())
    }
}
