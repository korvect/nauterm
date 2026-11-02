//! RS-only runtime checkpoints. The journal, lease and temporary writes share
//! the independent RS state directory, never Nauterm's recovery file. Native
//! handles and transient authentication options are not serialized.
use crate::model::{Id, RestoreBehavior};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Local {
        shell: Option<String>,
    },
    Host {
        id: Id,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port: Option<u16>,
    },
    MoshHost {
        id: Id,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port: Option<u16>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<String>,
    },
    TelnetHost {
        id: Id,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port: Option<u16>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionSnapshot {
    pub id: u64,
    pub title: String,
    pub target: Target,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaneSnapshot {
    pub id: u64,
    pub selected_session: u64,
    pub composer_visible: bool,
    pub directory: Option<String>,
    pub sessions: Vec<SessionSnapshot>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayoutSnapshot {
    Pane {
        pane: PaneSnapshot,
    },
    Split {
        id: u64,
        axis: Axis,
        fractions: Vec<f32>,
        children: Vec<LayoutSnapshot>,
    },
}
pub const DEFAULT_TOOLS_WIDTH: f32 = 350.;
pub const MIN_TOOLS_WIDTH: f32 = 240.;
pub const MAX_TOOLS_WIDTH: f32 = 640.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalTool {
    #[default]
    Sftp,
    SystemInfo,
    Snippets,
    History,
    Themes,
    Ai,
}
/// Presentation structure only. Never serialize Input entities/search text,
/// history commands, editor drafts, SFTP endpoints or native handles here.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalToolsSnapshot {
    pub open: bool,
    pub width: f32,
    pub mode: TerminalTool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabSnapshot {
    pub id: u64,
    pub selected_pane: u64,
    #[serde(default)]
    pub show_sftp: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<TerminalToolsSnapshot>,
    pub layout: LayoutSnapshot,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSnapshot {
    pub id: Id,
    pub name: String,
    pub color: u32,
    pub selected_tab: Option<u64>,
    pub tabs: Vec<TabSnapshot>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowMode {
    Windowed,
    Maximized,
    Fullscreen,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowGeometry {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub mode: WindowMode,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    /// Numeric resource IDs are meaningful only in the database that saved
    /// them. In particular, prototype IDs cannot be rebound to shared rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_database: Option<PathBuf>,
    pub selected_workspace: Option<Id>,
    pub workspaces: Vec<WorkspaceSnapshot>,
    pub window: Option<WindowGeometry>,
    /// Saved rule IDs only. Hosts, credentials and runtime handles are reloaded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_forwards: Vec<Id>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub version: u32,
    pub saved_at: String,
    pub clean_shutdown: bool,
    pub restore_on_next_launch: bool,
    pub snapshot: Snapshot,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchAction {
    None,
    Ask,
    Restore,
}
impl Checkpoint {
    fn running(snapshot: Snapshot) -> Self {
        Self {
            version: 1,
            saved_at: chrono::Utc::now().to_rfc3339(),
            clean_shutdown: false,
            restore_on_next_launch: false,
            snapshot,
        }
    }
    pub fn launch_action(&self, behavior: RestoreBehavior) -> LaunchAction {
        if !self.snapshot.has_work() {
            return LaunchAction::None;
        }
        if !self.clean_shutdown {
            return LaunchAction::Ask;
        }
        if !self.restore_on_next_launch {
            return LaunchAction::None;
        }
        match behavior {
            RestoreBehavior::Ask => LaunchAction::Ask,
            RestoreBehavior::Always => LaunchAction::Restore,
            RestoreBehavior::Never => LaunchAction::None,
        }
    }
}
fn positive(id: u64) -> bool {
    id > 0 && id <= i64::MAX as u64
}
pub fn change_directory_command(directory: &str) -> Option<String> {
    if !safe_directory(directory) || directory == "~" {
        return None;
    }
    let quote = |text: &str| format!("'{}'", text.replace('\'', "'\\''"));
    Some(if let Some(rest) = directory.strip_prefix("~/") {
        format!("cd -- ~/{path}\r", path = quote(rest))
    } else {
        format!("cd -- {path}\r", path = quote(directory))
    })
}
pub fn safe_directory(value: &str) -> bool {
    !value.is_empty() && value.len() <= 32_768 && !value.chars().any(char::is_control)
}
impl Snapshot {
    pub fn resources_match(&self, database: &Path) -> bool {
        self.resource_database.as_deref() == Some(database)
    }
    pub fn has_work(&self) -> bool {
        self.has_sessions() || !self.active_forwards.is_empty()
    }
    pub fn forward_plan(&self, data: &crate::model::AppState) -> Vec<Id> {
        self.active_forwards
            .iter()
            .copied()
            .filter(|id| data.forwards.iter().any(|rule| rule.id == *id))
            .collect()
    }
    pub fn has_sessions(&self) -> bool {
        self.workspaces
            .iter()
            .any(|workspace| !workspace.tabs.is_empty())
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.resource_database.as_ref().is_none_or(|path| {
                let value = path.to_string_lossy();
                path.is_absolute() && value.len() <= 32_768 && !value.contains('\0')
            }),
            "Invalid recovery resource database"
        );
        let mut forwards = BTreeSet::new();
        ensure!(
            self.active_forwards.len() <= 2048
                && self
                    .active_forwards
                    .iter()
                    .all(|id| positive(*id) && forwards.insert(*id)),
            "Duplicate, invalid or excessive recovery forwarding IDs"
        );
        ensure!(
            self.workspaces.len() <= 256,
            "Too many workspaces in recovery state"
        );
        if let Some(window) = self.window {
            ensure!(
                [window.left, window.top, window.width, window.height]
                    .iter()
                    .all(|v| v.is_finite())
                    && window.width > 0.
                    && window.height > 0.
                    && window.width < 100_000.
                    && window.height < 100_000.,
                "Invalid recovery window geometry"
            );
        }
        let mut ids = ValidIds::default();
        for workspace in &self.workspaces {
            ensure!(
                positive(workspace.id) && ids.workspaces.insert(workspace.id),
                "Duplicate or invalid recovery workspace ID"
            );
            ensure!(
                !workspace.name.trim().is_empty() && workspace.name.len() <= 4096,
                "Invalid recovery workspace name"
            );
            for tab in &workspace.tabs {
                ensure!(
                    positive(tab.id) && ids.tabs.insert(tab.id),
                    "Duplicate or invalid recovery tab ID"
                );
                if let Some(tools) = tab.tools {
                    ensure!(
                        tools.width.is_finite()
                            && (MIN_TOOLS_WIDTH..=MAX_TOOLS_WIDTH).contains(&tools.width),
                        "Invalid recovery terminal tools width"
                    );
                }
                tab.layout.validate(0, &mut ids)?;
            }
        }
        ensure!(
            ids.sessions.len() <= 4096 && ids.tabs.len() <= 2048,
            "Recovery state is too large"
        );
        Ok(())
    }
    pub fn maximum_runtime_id(&self) -> u64 {
        let mut ids = ValidIds::default();
        for tab in self.workspaces.iter().flat_map(|workspace| &workspace.tabs) {
            ids.tabs.insert(tab.id);
            let _ = tab.layout.validate(0, &mut ids);
        }
        ids.tabs
            .into_iter()
            .chain(ids.panes)
            .chain(ids.sessions)
            .max()
            .unwrap_or(0)
    }
    pub fn maximum_split_id(&self) -> u64 {
        let mut ids = ValidIds::default();
        for tab in self.workspaces.iter().flat_map(|workspace| &workspace.tabs) {
            let _ = tab.layout.validate(0, &mut ids);
        }
        ids.splits.into_iter().max().unwrap_or(0)
    }
}
#[derive(Default)]
struct ValidIds {
    workspaces: BTreeSet<u64>,
    tabs: BTreeSet<u64>,
    panes: BTreeSet<u64>,
    sessions: BTreeSet<u64>,
    splits: BTreeSet<u64>,
    nodes: usize,
}
impl LayoutSnapshot {
    fn validate(&self, depth: usize, ids: &mut ValidIds) -> Result<()> {
        ids.nodes += 1;
        ensure!(
            depth <= 32 && ids.nodes <= 8192,
            "Recovery layout is too deep or large"
        );
        match self {
            Self::Pane { pane } => {
                ensure!(
                    positive(pane.id) && ids.panes.insert(pane.id),
                    "Duplicate or invalid recovery pane ID"
                );
                ensure!(
                    !pane.sessions.is_empty(),
                    "A recovered pane must contain a session"
                );
                ensure!(
                    pane.directory.as_deref().is_none_or(safe_directory),
                    "Invalid recovery directory"
                );
                for session in &pane.sessions {
                    ensure!(
                        positive(session.id) && ids.sessions.insert(session.id),
                        "Duplicate or invalid recovery session ID"
                    );
                    ensure!(
                        session.title.len() <= 4096,
                        "Invalid recovery session title"
                    );
                    match &session.target {
                        Target::Local { shell: Some(shell) } => {
                            ensure!(safe_directory(shell), "Invalid recovered shell path")
                        }
                        Target::MoshHost { id, port, command } => {
                            ensure!(
                                *id > 0 && port.is_none_or(|port| port != 0),
                                "Invalid restored Mosh target"
                            );
                            if let Some(command) = command {
                                crate::mosh::Config::new(Some(command))?;
                            }
                        }
                        Target::Host { id, port } | Target::TelnetHost { id, port } => {
                            ensure!(
                                port.is_none_or(|port| port != 0),
                                "Invalid restored protocol port"
                            );
                            ensure!(positive(*id), "Invalid recovered host reference")
                        }
                        _ => {}
                    }
                }
            }
            Self::Split {
                id,
                fractions,
                children,
                ..
            } => {
                ensure!(
                    positive(*id) && ids.splits.insert(*id),
                    "Duplicate or invalid recovery split ID"
                );
                ensure!(
                    children.len() >= 2
                        && children.len() == fractions.len()
                        && fractions.iter().all(|v| v.is_finite() && *v > 0.)
                        && fractions.iter().sum::<f32>().is_finite(),
                    "Invalid recovery split weights"
                );
                for child in children {
                    child.validate(depth + 1, ids)?;
                }
            }
        }
        Ok(())
    }
}

struct Writer {
    generation: u64,
    pending: bool,
    candidate: Option<Checkpoint>,
    finished: bool,
    snapshot: Snapshot,
    behavior: RestoreBehavior,
}
pub struct Journal {
    directory: PathBuf,
    writer: Mutex<Writer>,
    _lease: File,
}
pub struct BeginRun {
    pub journal: Arc<Journal>,
    pub previous: Option<Checkpoint>,
}
impl Journal {
    pub fn begin(directory: &Path) -> Result<BeginRun> {
        fs::create_dir_all(directory)?;
        let lease_path = directory.join("workspace-state.lock");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let lease = options.open(lease_path)?;
        lease
            .try_lock()
            .context("Another Nauterm RS process is using this workspace state")?;
        let path = directory.join("workspace-state.json");
        let previous = match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_file()
                        && !metadata.file_type().is_symlink()
                        && metadata.len() <= 16 * 1024 * 1024,
                    "Invalid recovery state file; it was preserved"
                );
                let mut bytes = vec![];
                File::open(&path)?
                    .take(16 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() <= 16 * 1024 * 1024,
                    "Recovery state is too large"
                );
                let checkpoint: Checkpoint = serde_json::from_slice(&bytes)
                    .context("Cannot read recovery state; original file was preserved")?;
                ensure!(
                    checkpoint.version == 1,
                    "Unsupported recovery state version; original file was preserved"
                );
                chrono::DateTime::parse_from_rfc3339(&checkpoint.saved_at)
                    .context("Invalid recovery timestamp")?;
                checkpoint.snapshot.validate()?;
                Some(checkpoint)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let pending = previous.as_ref().is_some_and(|previous| {
            previous.snapshot.has_work()
                && (!previous.clean_shutdown || previous.restore_on_next_launch)
        });
        let snapshot = previous
            .as_ref()
            .filter(|_| pending)
            .map(|previous| previous.snapshot.clone())
            .unwrap_or_default();
        let journal = Arc::new(Self {
            directory: directory.into(),
            writer: Mutex::new(Writer {
                generation: 0,
                pending,
                candidate: previous.clone().filter(|_| pending),
                finished: false,
                snapshot: snapshot.clone(),
                behavior: RestoreBehavior::Ask,
            }),
            _lease: lease,
        });
        journal.write(&Checkpoint::running(snapshot))?;
        Ok(BeginRun { journal, previous })
    }
    fn write(&self, checkpoint: &Checkpoint) -> Result<()> {
        checkpoint.snapshot.validate()?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
        temporary.write_all(&serde_json::to_vec_pretty(checkpoint)?)?;
        temporary.as_file().sync_all()?;
        temporary.persist(self.directory.join("workspace-state.json"))?;
        Ok(())
    }
    /// Consume recovery before starting processes. A failed write leaves the
    /// candidate pending and must not cause any connection/PTY to be started.
    pub fn consume(&self, snapshot: Snapshot, behavior: RestoreBehavior) -> Result<()> {
        snapshot.validate()?;
        let mut writer = self.writer.lock().unwrap();
        ensure!(!writer.finished, "Recovery writer has already closed");
        self.write(&Checkpoint::running(snapshot.clone()))?;
        writer.generation += 1;
        writer.pending = false;
        writer.candidate = None;
        writer.snapshot = snapshot;
        writer.behavior = behavior;
        Ok(())
    }
    pub fn stage(&self, snapshot: Snapshot, behavior: RestoreBehavior) -> Result<Option<u64>> {
        snapshot.validate()?;
        let mut writer = self.writer.lock().unwrap();
        if writer.pending || writer.finished {
            return Ok(None);
        }
        writer.generation += 1;
        writer.snapshot = snapshot;
        writer.behavior = behavior;
        Ok(Some(writer.generation))
    }
    pub fn checkpoint(&self, generation: u64) -> Result<()> {
        let writer = self.writer.lock().unwrap();
        if writer.pending || writer.finished || writer.generation != generation {
            return Ok(());
        }
        self.write(&Checkpoint::running(writer.snapshot.clone()))
    }
    /// A macOS window can close while the process and journal lease stay alive.
    /// Preserve the same clean recovery candidate as Quit, without permanently
    /// finishing the writer. Unanswered launch questions retain their decision.
    #[cfg(any(target_os = "macos", test))]
    pub fn suspend_window(&self) -> Result<()> {
        let mut writer = self.writer.lock().unwrap();
        ensure!(!writer.finished, "Recovery writer has already closed");
        let checkpoint = writer.candidate.clone().unwrap_or_else(|| {
            let mut checkpoint = Checkpoint::running(writer.snapshot.clone());
            checkpoint.clean_shutdown = true;
            checkpoint.restore_on_next_launch =
                writer.behavior != RestoreBehavior::Never && checkpoint.snapshot.has_work();
            checkpoint
        });
        self.write(&checkpoint)?;
        writer.generation += 1;
        writer.pending = true;
        writer.candidate = Some(checkpoint);
        Ok(())
    }
    /// Start another window in this process using the ordinary launch policy.
    /// Mark its lifetime running before it can start any recovered connection.
    pub fn resume_window(self: &Arc<Self>) -> Result<BeginRun> {
        let writer = self.writer.lock().unwrap();
        ensure!(!writer.finished, "Recovery writer has already closed");
        self.write(&Checkpoint::running(writer.snapshot.clone()))?;
        Ok(BeginRun {
            journal: self.clone(),
            previous: writer.candidate.clone(),
        })
    }
    /// Final application quit only. Window suspension leaves the writer reusable.
    /// The generation/finished gate rejects a late debounced writer.
    pub fn finish(&self) -> Result<()> {
        let mut writer = self.writer.lock().unwrap();
        if writer.finished {
            return Ok(());
        }
        if let Some(candidate) = &writer.candidate {
            // An unanswered dialog is neither a discard decision nor a crash.
            self.write(candidate)?;
        } else {
            let mut checkpoint = Checkpoint::running(writer.snapshot.clone());
            checkpoint.clean_shutdown = true;
            checkpoint.restore_on_next_launch =
                writer.behavior != RestoreBehavior::Never && checkpoint.snapshot.has_work();
            self.write(&checkpoint)?;
        }
        writer.finished = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> Snapshot {
        Snapshot {
            resource_database: None,
            selected_workspace: Some(1),
            window: None,
            active_forwards: vec![],
            workspaces: vec![WorkspaceSnapshot {
                id: 1,
                name: "QA".into(),
                color: 0x075e92,
                selected_tab: Some(1),
                tabs: vec![TabSnapshot {
                    id: 1,
                    selected_pane: 1,
                    show_sftp: false,
                    tools: None,
                    layout: LayoutSnapshot::Pane {
                        pane: PaneSnapshot {
                            id: 1,
                            selected_session: 1,
                            composer_visible: false,
                            directory: Some("/tmp/工作区".into()),
                            sessions: vec![SessionSnapshot {
                                id: 1,
                                title: "QA host".into(),
                                target: Target::Host { id: 5, port: None },
                            }],
                        },
                    },
                }],
            }],
        }
    }
    #[test]
    fn close_window_and_quit_offer_the_same_recovery_policy_without_finishing_the_process()
    -> Result<()> {
        for (behavior, expected) in [
            (RestoreBehavior::Ask, LaunchAction::Ask),
            (RestoreBehavior::Always, LaunchAction::Restore),
            (RestoreBehavior::Never, LaunchAction::None),
        ] {
            let window_directory = tempfile::tempdir()?;
            let quit_directory = tempfile::tempdir()?;
            let window = Journal::begin(window_directory.path())?.journal;
            let quit = Journal::begin(quit_directory.path())?.journal;
            let state = snapshot();
            window.consume(state.clone(), behavior)?;
            quit.consume(state.clone(), behavior)?;
            let stale = window.stage(state.clone(), behavior)?.unwrap();
            window.suspend_window()?;
            quit.finish()?;
            window.checkpoint(stale)?;
            let closed = load(window_directory.path());
            let exited = load(quit_directory.path());
            assert_eq!(closed.snapshot, exited.snapshot);
            assert_eq!(closed.clean_shutdown, exited.clean_shutdown);
            assert_eq!(closed.restore_on_next_launch, exited.restore_on_next_launch);
            assert_eq!(closed.launch_action(behavior), expected);
            let reopened = window.resume_window()?.previous.unwrap();
            assert_eq!(reopened, closed);
            assert!(!load(window_directory.path()).clean_shutdown);
            // Start Fresh consumes the previous decision, then this process can
            // own another window and checkpoint again under the same lease.
            window.consume(Snapshot::default(), behavior)?;
            assert!(window.stage(state.clone(), behavior)?.is_some());
            window.suspend_window()?;
            assert_eq!(window.resume_window()?.previous.unwrap().snapshot, state);
            window.consume(Snapshot::default(), behavior)?;
            window.suspend_window()?;
            assert_eq!(
                window
                    .resume_window()?
                    .previous
                    .unwrap()
                    .launch_action(behavior),
                LaunchAction::None
            );
            window.finish()?;
        }
        Ok(())
    }
    #[test]
    fn closing_an_unanswered_recovery_window_preserves_the_original_candidate() -> Result<()> {
        let directory = tempfile::tempdir()?;
        {
            let run = Journal::begin(directory.path())?;
            run.journal.consume(snapshot(), RestoreBehavior::Always)?;
        }
        let run = Journal::begin(directory.path())?;
        let interrupted = run.previous.unwrap();
        assert!(!interrupted.clean_shutdown);
        run.journal.suspend_window()?;
        assert_eq!(load(directory.path()), interrupted);
        let resumed = run.journal.resume_window()?;
        assert_eq!(resumed.previous, Some(interrupted.clone()));
        assert_eq!(
            interrupted.launch_action(RestoreBehavior::Never),
            LaunchAction::Ask
        );
        run.journal.finish()?;
        assert_eq!(load(directory.path()), interrupted);
        Ok(())
    }
    #[test]
    fn protocol_attempt_ports_round_trip_without_changing_legacy_host_targets() -> Result<()> {
        let legacy: Target = serde_json::from_str(r#"{"kind":"host","id":5}"#)?;
        assert_eq!(legacy, Target::Host { id: 5, port: None });
        assert!(serde_json::to_value(&legacy)?.get("port").is_none());
        for target in [
            Target::Host {
                id: 5,
                port: Some(2022),
            },
            Target::TelnetHost {
                id: 5,
                port: Some(2323),
            },
        ] {
            let mut state = snapshot();
            let LayoutSnapshot::Pane { pane } = &mut state.workspaces[0].tabs[0].layout else {
                unreachable!()
            };
            pane.sessions[0].target = target.clone();
            state.validate()?;
            assert_eq!(
                serde_json::from_str::<Snapshot>(&serde_json::to_string(&state)?)?,
                state
            );
            let LayoutSnapshot::Pane { pane } = &mut state.workspaces[0].tabs[0].layout else {
                unreachable!()
            };
            pane.sessions[0].target = match target {
                Target::Host { id, .. } => Target::Host { id, port: Some(0) },
                Target::TelnetHost { id, .. } => Target::TelnetHost { id, port: Some(0) },
                _ => unreachable!(),
            };
            assert!(state.validate().is_err());
        }
        Ok(())
    }
    #[test]
    fn mosh_recovery_keeps_only_host_and_optional_attempt_overrides() -> Result<()> {
        let mut state = snapshot();
        let LayoutSnapshot::Pane { pane } = &mut state.workspaces[0].tabs[0].layout else {
            unreachable!()
        };
        pane.sessions[0].target = Target::MoshHost {
            id: 5,
            port: Some(2022),
            command: Some("/opt/mosh-server new -s".into()),
        };
        state.validate()?;
        let json = serde_json::to_value(&state)?;
        assert_eq!(serde_json::from_value::<Snapshot>(json)?, state);
        let LayoutSnapshot::Pane { pane } = &mut state.workspaces[0].tabs[0].layout else {
            unreachable!()
        };
        pane.sessions[0].target = Target::MoshHost {
            id: 5,
            port: None,
            command: Some("mosh-server\nother".into()),
        };
        assert!(state.validate().is_err());
        Ok(())
    }
    fn load(directory: &Path) -> Checkpoint {
        serde_json::from_slice(&fs::read(directory.join("workspace-state.json")).unwrap()).unwrap()
    }
    #[test]
    fn terminal_tools_round_trip_only_public_presentation_fields() -> Result<()> {
        for mode in [
            TerminalTool::Sftp,
            TerminalTool::SystemInfo,
            TerminalTool::Snippets,
            TerminalTool::History,
            TerminalTool::Themes,
            TerminalTool::Ai,
        ] {
            for open in [false, true] {
                let mut state = snapshot();
                state.workspaces[0].tabs[0].tools = Some(TerminalToolsSnapshot {
                    open,
                    width: 510.5,
                    mode,
                });
                state.validate()?;
                let json = serde_json::to_value(&state)?;
                let tools = json["workspaces"][0]["tabs"][0]["tools"]
                    .as_object()
                    .unwrap();
                assert_eq!(
                    tools.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                    BTreeSet::from(["mode", "open", "width"])
                );
                assert_eq!(serde_json::from_value::<Snapshot>(json)?, state);
            }
        }
        Ok(())
    }
    #[test]
    fn terminal_tools_legacy_snapshots_keep_default_presentation() -> Result<()> {
        let state = snapshot();
        let json = serde_json::to_value(&state)?;
        assert!(json["workspaces"][0]["tabs"][0].get("tools").is_none());
        let legacy: Snapshot = serde_json::from_value(json)?;
        assert_eq!(legacy, state);
        legacy.validate()?;
        Ok(())
    }
    #[test]
    fn terminal_tools_reject_invalid_geometry_and_unknown_fields() -> Result<()> {
        let mut state = snapshot();
        for width in [0., 239., 641., f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            state.workspaces[0].tabs[0].tools = Some(TerminalToolsSnapshot {
                open: true,
                width,
                mode: TerminalTool::Themes,
            });
            assert!(state.validate().is_err());
        }
        for width in [MIN_TOOLS_WIDTH, DEFAULT_TOOLS_WIDTH, MAX_TOOLS_WIDTH] {
            state.workspaces[0].tabs[0].tools = Some(TerminalToolsSnapshot {
                open: true,
                width,
                mode: TerminalTool::Themes,
            });
            state.validate()?;
        }
        assert!(
            serde_json::from_value::<TerminalToolsSnapshot>(
                serde_json::json!({"open": true, "width": 350, "mode": "unknown_tool"})
            )
            .is_err()
        );
        assert!(serde_json::from_value::<TerminalToolsSnapshot>(serde_json::json!({"open": true, "width": 350, "mode": "history", "search": "synthetic search text"})).is_err());
        Ok(())
    }
    #[test]
    fn clean_policy_and_unexpected_exit_have_distinct_launch_decisions() {
        let mut checkpoint = Checkpoint::running(snapshot());
        for behavior in [
            RestoreBehavior::Ask,
            RestoreBehavior::Always,
            RestoreBehavior::Never,
        ] {
            assert_eq!(checkpoint.launch_action(behavior), LaunchAction::Ask);
        }
        checkpoint.clean_shutdown = true;
        assert_eq!(
            checkpoint.launch_action(RestoreBehavior::Always),
            LaunchAction::None
        );
        checkpoint.restore_on_next_launch = true;
        assert_eq!(
            checkpoint.launch_action(RestoreBehavior::Ask),
            LaunchAction::Ask
        );
        assert_eq!(
            checkpoint.launch_action(RestoreBehavior::Always),
            LaunchAction::Restore
        );
        assert_eq!(
            checkpoint.launch_action(RestoreBehavior::Never),
            LaunchAction::None
        );
    }
    #[test]
    fn start_fresh_is_durable_and_close_window_cannot_resurrect_the_candidate() -> Result<()> {
        let directory = tempfile::tempdir()?;
        {
            let run = Journal::begin(directory.path())?;
            run.journal.consume(snapshot(), RestoreBehavior::Ask)?;
        }
        {
            let run = Journal::begin(directory.path())?;
            assert!(!run.previous.as_ref().unwrap().clean_shutdown);
            run.journal
                .consume(Snapshot::default(), RestoreBehavior::Ask)?;
            run.journal.finish()?;
        }
        let run = Journal::begin(directory.path())?;
        assert_eq!(
            run.previous.unwrap().launch_action(RestoreBehavior::Ask),
            LaunchAction::None
        );
        Ok(())
    }
    #[test]
    fn unfinished_prompt_preserves_candidate_and_late_writes_cannot_replace_clean_exit()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        {
            let run = Journal::begin(directory.path())?;
            run.journal.consume(snapshot(), RestoreBehavior::Always)?;
            let old = run
                .journal
                .stage(Snapshot::default(), RestoreBehavior::Always)?
                .unwrap();
            let current = run
                .journal
                .stage(snapshot(), RestoreBehavior::Always)?
                .unwrap();
            run.journal.checkpoint(old)?;
            run.journal.finish()?;
            run.journal.checkpoint(current)?;
            assert!(load(directory.path()).clean_shutdown);
        }
        {
            let run = Journal::begin(directory.path())?;
            assert!(run.previous.unwrap().restore_on_next_launch);
            assert!(
                run.journal
                    .stage(Snapshot::default(), RestoreBehavior::Ask)?
                    .is_none()
            );
            run.journal.finish()?;
        }
        let run = Journal::begin(directory.path())?;
        assert_eq!(
            run.previous.unwrap().launch_action(RestoreBehavior::Always),
            LaunchAction::Restore
        );
        Ok(())
    }
    #[test]
    fn invalid_state_is_preserved_and_another_process_cannot_take_the_live_lease() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let run = Journal::begin(directory.path())?;
        assert!(Journal::begin(directory.path()).is_err());
        drop(run);
        fs::write(directory.path().join("workspace-state.json"), b"{invalid")?;
        assert!(Journal::begin(directory.path()).is_err());
        assert_eq!(
            fs::read(directory.path().join("workspace-state.json"))?,
            b"{invalid"
        );
        Ok(())
    }
    #[test]
    fn snapshot_types_contain_references_not_credentials_or_terminal_output() -> Result<()> {
        let original = snapshot();
        original.validate()?;
        assert_eq!(original.maximum_runtime_id(), 1);
        let encoded = serde_json::to_string(&original)?;
        for forbidden in [
            "password",
            "private_key",
            "passphrase",
            "certificate",
            "scrollback",
            "terminal_output",
        ] {
            assert!(!encoded.contains(forbidden));
        }
        let mut bad = serde_json::to_value(original)?;
        bad["workspaces"][0]["tabs"][0]["layout"]["pane"]["sessions"][0]["target"]["password"] =
            "must-not-be-accepted".into();
        assert!(serde_json::from_value::<Snapshot>(bad).is_err());
        assert!(!safe_directory("/tmp/a\ncommand"));
        Ok(())
    }

    #[test]
    fn directory_commands_quote_metacharacters_and_reject_control_sequences() {
        assert_eq!(
            change_directory_command("/tmp/a'b;$(touch pwned)"),
            Some("cd -- '/tmp/a'\\''b;$(touch pwned)'\r".into())
        );
        assert_eq!(
            change_directory_command("~/with spaces/工作区"),
            Some("cd -- ~/'with spaces/工作区'\r".into())
        );
        for invalid in [
            "",
            "~",
            "/tmp/a\rcommand",
            "/tmp/a\ncommand",
            "/tmp/\u{1b}[31m",
            "/tmp/\0",
        ] {
            assert!(change_directory_command(invalid).is_none());
        }
    }

    #[test]
    fn invalid_ids_duplicate_nodes_and_split_weights_fail_before_launch() {
        let base = snapshot();
        let leaf = base.workspaces[0].tabs[0].layout.clone();
        for invalid_id in [0, u64::MAX] {
            let mut bad = base.clone();
            bad.workspaces[0].tabs[0].id = invalid_id;
            assert!(bad.validate().is_err());
        }
        for fractions in [
            vec![1.],
            vec![0., 1.],
            vec![f32::NAN, 1.],
            vec![f32::INFINITY, 1.],
            vec![f32::MAX, f32::MAX],
            vec![0.5, 0.5],
        ] {
            let mut bad = base.clone();
            bad.workspaces[0].tabs[0].layout = LayoutSnapshot::Split {
                id: 1,
                axis: Axis::Horizontal,
                fractions,
                children: vec![leaf.clone(), leaf.clone()],
            };
            // Even valid weights cannot make duplicate pane/session IDs safe.
            assert!(bad.validate().is_err());
        }
    }

    #[test]
    fn stale_checkpoint_generation_cannot_replace_a_newer_runtime_snapshot() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let run = Journal::begin(directory.path())?;
        run.journal.consume(snapshot(), RestoreBehavior::Ask)?;
        let old = run
            .journal
            .stage(Snapshot::default(), RestoreBehavior::Ask)?
            .unwrap();
        let current = run
            .journal
            .stage(snapshot(), RestoreBehavior::Ask)?
            .unwrap();
        run.journal.checkpoint(current)?;
        run.journal.checkpoint(old)?;
        assert!(load(directory.path()).snapshot.has_sessions());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(directory.path().join("workspace-state.json"))?
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        Ok(())
    }
    #[test]
    fn pre_quit_checkpoint_stays_running_until_the_final_exit_hook_finishes() -> Result<()> {
        let directory = tempfile::tempdir()?;
        {
            let run = Journal::begin(directory.path())?;
            run.journal.consume(snapshot(), RestoreBehavior::Always)?;
            let prepared = run
                .journal
                .stage(snapshot(), RestoreBehavior::Always)?
                .unwrap();
            run.journal.checkpoint(prepared)?;
            assert!(!load(directory.path()).clean_shutdown);
            // A native handoff which has not quit must not freeze future writes.
            let changed = run
                .journal
                .stage(Snapshot::default(), RestoreBehavior::Never)?
                .unwrap();
            run.journal.checkpoint(changed)?;
            assert!(!load(directory.path()).snapshot.has_work());
            assert!(!load(directory.path()).clean_shutdown);
            run.journal.finish()?;
            assert!(load(directory.path()).clean_shutdown);
        }
        let next = Journal::begin(directory.path())?;
        assert_eq!(
            next.previous
                .unwrap()
                .launch_action(RestoreBehavior::Always),
            LaunchAction::None
        );
        Ok(())
    }

    #[test]
    fn terminal_sftp_page_restores_and_older_checkpoints_default_to_terminal() -> Result<()> {
        let mut original = snapshot();
        original.workspaces[0].tabs[0].show_sftp = true;
        let mut encoded = serde_json::to_value(&original)?;
        assert!(
            serde_json::from_value::<Snapshot>(encoded.clone())?.workspaces[0].tabs[0].show_sftp
        );
        encoded["workspaces"][0]["tabs"][0]
            .as_object_mut()
            .unwrap()
            .remove("show_sftp");
        assert!(!serde_json::from_value::<Snapshot>(encoded)?.workspaces[0].tabs[0].show_sftp);
        Ok(())
    }
    #[test]
    fn saved_resource_ids_are_bound_to_the_original_database() -> Result<()> {
        let shared = Path::new(if cfg!(windows) {
            "C:\\example\\Nauterm\\nauterm.sqlite"
        } else {
            "/example/Nauterm/nauterm.sqlite"
        });
        let mut state = snapshot();
        assert!(!state.resources_match(shared));
        state.resource_database = Some(shared.into());
        state.validate()?;
        let decoded: Snapshot = serde_json::from_slice(&serde_json::to_vec(&state)?)?;
        assert!(decoded.resources_match(shared));
        assert!(!decoded.resources_match(&shared.with_file_name("another.sqlite")));
        state.resource_database = Some("relative.sqlite".into());
        assert!(state.validate().is_err());
        Ok(())
    }

    #[test]
    fn forward_only_checkpoints_follow_clean_policy_and_always_ask_after_a_crash() -> Result<()> {
        let snapshot = Snapshot {
            active_forwards: vec![3, 7],
            ..Default::default()
        };
        assert!(!snapshot.has_sessions());
        assert!(snapshot.has_work());
        snapshot.validate()?;
        let mut checkpoint = Checkpoint::running(snapshot);
        for behavior in [
            RestoreBehavior::Ask,
            RestoreBehavior::Always,
            RestoreBehavior::Never,
        ] {
            assert_eq!(checkpoint.launch_action(behavior), LaunchAction::Ask);
        }
        checkpoint.clean_shutdown = true;
        checkpoint.restore_on_next_launch = true;
        assert_eq!(
            checkpoint.launch_action(RestoreBehavior::Ask),
            LaunchAction::Ask
        );
        assert_eq!(
            checkpoint.launch_action(RestoreBehavior::Always),
            LaunchAction::Restore
        );
        assert_eq!(
            checkpoint.launch_action(RestoreBehavior::Never),
            LaunchAction::None
        );
        Ok(())
    }
    #[test]
    fn forward_only_start_fresh_is_durable_for_quit_and_close_window() -> Result<()> {
        for behavior in [
            RestoreBehavior::Ask,
            RestoreBehavior::Always,
            RestoreBehavior::Never,
        ] {
            let directory = tempfile::tempdir()?;
            {
                let run = Journal::begin(directory.path())?;
                run.journal.consume(
                    Snapshot {
                        active_forwards: vec![3],
                        ..Default::default()
                    },
                    behavior,
                )?;
                // No finish simulates the previous process's unexpected exit.
            }
            {
                let run = Journal::begin(directory.path())?;
                assert_eq!(
                    run.previous.as_ref().unwrap().launch_action(behavior),
                    LaunchAction::Ask
                );
                run.journal.consume(Snapshot::default(), behavior)?;
                run.journal.finish()?; // Shared Close Window / Quit operation.
            }
            let run = Journal::begin(directory.path())?;
            assert_eq!(
                run.previous.as_ref().unwrap().launch_action(behavior),
                LaunchAction::None
            );
            assert!(run.previous.unwrap().snapshot.active_forwards.is_empty());
        }
        Ok(())
    }
    #[test]
    fn clean_forward_only_exit_records_policy_and_preserves_unanswered_prompt() -> Result<()> {
        for behavior in [
            RestoreBehavior::Ask,
            RestoreBehavior::Always,
            RestoreBehavior::Never,
        ] {
            let directory = tempfile::tempdir()?;
            {
                let run = Journal::begin(directory.path())?;
                run.journal.consume(
                    Snapshot {
                        active_forwards: vec![3],
                        ..Default::default()
                    },
                    behavior,
                )?;
                run.journal.finish()?;
                assert_eq!(
                    load(directory.path()).restore_on_next_launch,
                    behavior != RestoreBehavior::Never
                );
            }
            {
                let run = Journal::begin(directory.path())?;
                let expected = match behavior {
                    RestoreBehavior::Ask => LaunchAction::Ask,
                    RestoreBehavior::Always => LaunchAction::Restore,
                    RestoreBehavior::Never => LaunchAction::None,
                };
                assert_eq!(
                    run.previous.as_ref().unwrap().launch_action(behavior),
                    expected
                );
                if behavior != RestoreBehavior::Never {
                    assert!(run.journal.stage(Snapshot::default(), behavior)?.is_none());
                }
                run.journal.finish()?;
            }
            let run = Journal::begin(directory.path())?;
            if behavior != RestoreBehavior::Never {
                assert_eq!(run.previous.unwrap().snapshot.active_forwards, vec![3]);
            }
        }
        Ok(())
    }
    #[test]
    fn forward_references_are_bounded_backwards_compatible_and_not_credentials() -> Result<()> {
        let mut value = serde_json::to_value(snapshot())?;
        assert!(value.get("active_forwards").is_none());
        assert!(
            serde_json::from_value::<Snapshot>(value.clone())?
                .active_forwards
                .is_empty()
        );
        value["active_forwards"] = serde_json::json!([3, 7]);
        let decoded: Snapshot = serde_json::from_value(value.clone())?;
        decoded.validate()?;
        assert_eq!(decoded.active_forwards, vec![3, 7]);
        for invalid in [vec![0], vec![u64::MAX], vec![3, 3], (1..=2049).collect()] {
            assert!(
                Snapshot {
                    active_forwards: invalid,
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
        value["active_forwards"] = serde_json::json!([{"id":3,"password":"must-not-be-accepted"}]);
        assert!(serde_json::from_value::<Snapshot>(value).is_err());
        Ok(())
    }
}
