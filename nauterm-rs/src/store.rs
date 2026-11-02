mod catalog;
mod config;
use crate::{model::AppState, native::Core, vault::Vault};
use anyhow::{Context, Result};
use catalog::Catalog;
use nauterm_core::paths::ApplicationPaths;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

pub struct Store {
    pub paths: ApplicationPaths,
    /// Per-client window/workspace metadata, recovery journal and lease. Never
    /// derive this from another client's resource directory.
    pub state_directory: PathBuf,
    config_directory: PathBuf,
    vault: OnceLock<Vault>,
    sync_vault: OnceLock<Vault>,
    baseline: Arc<Mutex<Option<Catalog>>>,
}

impl Store {
    pub fn sync_service(&self) -> Result<crate::sync::Service> {
        // Network work may take minutes. Its connection uses the same database
        // but must not hold up credential reads or catalog edits in the UI.
        if self.sync_vault.get().is_none() {
            let vault = Vault::open(Core::discover()?, self.database_path())?;
            let _ = self.sync_vault.set(vault);
        }
        Ok(crate::sync::Service::new(
            self.sync_vault
                .get()
                .context("Sync storage is unavailable")?
                .clone(),
        ))
    }
    pub fn ai_providers(&self) -> Result<crate::ai::Service> {
        Ok(crate::ai::Service::new(
            self.vault()?.clone(),
            self.paths.data_directory.clone(),
        ))
    }
    /// Compiled platform defaults only; no user files, database or keyring.
    pub fn default_preferences() -> Result<AppState> {
        let mut state = AppState::default();
        config::Document::new()?.apply(&mut state)?;
        state.validate()?;
        Ok(state)
    }

    pub fn publish_snippets(
        &self,
        previous: &AppState,
        catalog: &crate::snippet_store::Catalog,
    ) -> Result<AppState> {
        catalog.validate()?;
        catalog.verify_import(&previous.legacy_snippets)?;
        let mut candidate = previous.clone();
        candidate.snippets = catalog.snippets.clone();
        candidate.legacy_snippets.clear();
        if !previous.legacy_snippets.is_empty() {
            self.save(&mut candidate)?;
        }
        Ok(candidate)
    }
    pub fn discover() -> Result<Self> {
        nauterm_core::application::configure(nauterm_core::application::Client::NautermRs)
            .map_err(anyhow::Error::msg)?;
        let mut paths = ApplicationPaths::discover();
        if let Some(directory) = std::env::var_os("NAUTERM_RS_DATA_DIR") {
            paths = paths.isolated_rs(directory.into());
        }
        let config_directory = paths.rs_config_directory.clone();
        let state_directory = paths.rs_data_directory.clone();
        Ok(Self {
            paths,
            state_directory,
            config_directory,
            vault: OnceLock::new(),
            sync_vault: OnceLock::new(),
            baseline: Arc::new(Mutex::new(None)),
        })
    }

    pub fn database_path(&self) -> PathBuf {
        self.paths.database()
    }
    /// Read startup dimensions without opening the resource DB or OS keyring.
    pub fn initial_window_size(&self) -> Result<crate::general_preferences::WindowSize> {
        let document = match read_optional(&self.config_directory.join("config.json"))? {
            Some(bytes) => config::Document::parse(&bytes)?,
            None => config::Document::new()?,
        };
        Ok(document.general_preferences()?.window)
    }

    /// Public startup appearance only. Opening the encrypted catalog is deferred
    /// until the native window has presented its first frame.
    pub fn initial_preferences(&self) -> Result<AppState> {
        let document = match read_optional(&self.config_directory.join("config.json"))? {
            Some(bytes) => config::Document::parse(&bytes)?,
            None => config::Document::new()?,
        };
        let mut state = AppState::default();
        document.apply(&mut state)?;
        state.validate()?;
        Ok(state)
    }

    fn vault(&self) -> Result<&Vault> {
        if self.vault.get().is_none() {
            let vault = Vault::open(Core::discover()?, self.database_path())?;
            let _ = self.vault.set(vault);
        }
        self.vault
            .get()
            .context("The Nauterm RS database is unavailable")
    }

    /// Workspace metadata and public preferences can be rendered before any
    /// encrypted resource is opened. Saved resource rows always come from SQL.
    pub fn initial_state(&self) -> Result<AppState> {
        let path = self.state_directory.join("workspace.json");
        let mut state: AppState = match read_optional(&path)? {
            Some(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("Cannot read {}", path.display()))?,
            None => self.initial_preferences()?,
        };
        state.hosts.clear();
        state.groups.clear();
        state.tags.clear();
        state.forwards.clear();
        state.proxies.clear();
        state.snippets.clear();
        state.legacy_snippets.clear();
        if let Some(bytes) = read_optional(&self.config_directory.join("config.json"))? {
            config::Document::parse(&bytes)?.apply(&mut state)?;
        }
        state
            .validate()
            .context("Invalid workspace metadata or config. Original files were preserved.")?;
        Ok(state)
    }

    pub async fn load(&self) -> Result<AppState> {
        let mut state = self.initial_state()?;
        let catalog = self
            .vault()?
            .dispatch(|database| database.inner.atomic(|database| Catalog::load(database)))
            .await?;
        catalog.apply(&mut state);
        state.validate()?;
        *self
            .baseline
            .lock()
            .map_err(|_| anyhow::anyhow!("Resource Store is unavailable"))? = Some(catalog);
        Ok(state)
    }

    pub fn refresh_resources(&self, state: &mut AppState) -> Result<()> {
        let mut baseline = self
            .baseline
            .lock()
            .map_err(|_| anyhow::anyhow!("Resource Store is unavailable"))?;
        let catalog = smol::block_on(
            self.vault()?
                .dispatch(|database| database.inner.atomic(|database| Catalog::load(database))),
        )?;
        let mut candidate = state.clone();
        catalog.apply(&mut candidate);
        candidate.validate()?;
        *baseline = Some(catalog);
        *state = candidate;
        Ok(())
    }

    pub fn detect_host_platform(
        &self,
        target: crate::host_platform::Target,
        request: crate::native::HostOsRequest,
    ) -> impl std::future::Future<Output = Result<Option<nauterm_core::HostPlatformChange>>> + use<>
    {
        let baseline = self.baseline.clone();
        let vault = self.vault().cloned();
        async move {
            let lease = request.lease.clone();
            let platform = smol::unblock(move || request.read()).await?;
            smol::unblock(move || {
                let vault = vault?;
                // Keep the same baseline -> database-worker lock order as Save.
                // Never hold this lock while waiting for the network probe.
                let mut baseline = baseline
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Resource Store is unavailable"))?;
                let reference = target.clone();
                let change = smol::block_on(vault.dispatch(move |database| {
                    lease.validate()?;
                    Ok(database.inner.update_host_platform(
                        i64::try_from(reference.id)?,
                        &reference.uuid,
                        &reference.address,
                        reference.port,
                        platform.os.as_deref().unwrap_or_default(),
                        platform.distro.as_deref(),
                    )?)
                }))?;
                // Publish our new stamp before any waiting Save can proceed.
                if let (Some(catalog), Some(change)) = (baseline.as_mut(), &change) {
                    catalog.apply_host_platform(&target, change);
                }
                Ok(change)
            })
            .await
        }
    }

    pub fn apply_host_platform(
        &self,
        state: &mut AppState,
        target: &crate::host_platform::Target,
    ) -> Result<()> {
        let baseline = self
            .baseline
            .lock()
            .map_err(|_| anyhow::anyhow!("Resource Store is unavailable"))?;
        if let Some(current) = baseline
            .as_ref()
            .and_then(|catalog| catalog.hosts.iter().find(|host| target.matches(host)))
            && let Some(host) = state.hosts.iter_mut().find(|host| target.matches(host))
        {
            // A refresh/save may have completed since the background result.
            // Read the latest projection instead of replaying an older result.
            host.platform = current.platform.clone();
        }
        Ok(())
    }

    pub fn save(&self, state: &mut AppState) -> Result<()> {
        self.save_with_keys(state, None).map(|_| ())
    }
    pub fn publish_host_import(
        &self,
        state: &mut AppState,
        keys: crate::host_import::KeyBatch,
    ) -> Result<crate::host_import::KeyReport> {
        self.save_with_keys(state, Some(keys))
    }
    fn save_with_keys(
        &self,
        state: &mut AppState,
        keys: Option<crate::host_import::KeyBatch>,
    ) -> Result<crate::host_import::KeyReport> {
        state.validate()?;
        let mut baseline = self
            .baseline
            .lock()
            .map_err(|_| anyhow::anyhow!("Resource Store is unavailable"))?;
        let before = baseline
            .as_ref()
            .context("Load the resource catalog before saving")?
            .clone();
        fs::create_dir_all(&self.state_directory)?;
        fs::create_dir_all(&self.config_directory)?;
        let config_path = self.config_directory.join("config.json");
        let previous_config = read_optional(&config_path)?;
        let document = match &previous_config {
            Some(bytes) => config::Document::parse(bytes)?,
            None => config::Document::new()?,
        };
        let config_bytes = document.encode(state)?;
        // Workspace structure belongs to this client. Resources live in shared
        // native tables; preferences have one independent config owner.
        let mut metadata = serde_json::to_value(&*state)?;
        for name in [
            "hosts",
            "groups",
            "tags",
            "forwards",
            "proxies",
            "snippets",
            "dark",
            "theme",
            "host_icons",
            "shortcuts",
            "general",
            "restore_behavior",
            "terminal",
            "terminal_preferences",
            "sftp_editors",
            "sftp",
            "recordings",
            "ai",
        ] {
            metadata.as_object_mut().unwrap().remove(name);
        }
        let mut files = vec![
            Publication::stage(config_path, previous_config, &config_bytes)?,
            Publication::stage(
                self.state_directory.join("workspace.json"),
                read_optional(&self.state_directory.join("workspace.json"))?,
                &serde_json::to_vec_pretty(&metadata)?,
            )?,
        ];
        let mut candidate = state.clone();
        let old = before.clone();
        let (catalog, report, requested) =
            smol::block_on(self.vault()?.dispatch(move |database| {
                let result = database.inner.atomic(|database| {
                    if let Some(batch) = &keys {
                        anyhow::ensure!(
                            batch
                                .bindings
                                .keys()
                                .all(|id| !before.hosts.iter().any(|host| host.id == *id)),
                            "Imported keys may only bind newly imported Hosts."
                        );
                    }
                    let report = keys
                        .map(|batch| batch.prepare(database, &mut candidate))
                        .transpose()?
                        .unwrap_or_default();
                    candidate.validate()?;
                    let catalog = before.persist(database, &candidate)?;
                    for file in &mut files {
                        file.publish()?;
                    }
                    Ok::<_, anyhow::Error>((catalog, report, candidate.clone()))
                });
                if result.is_err() {
                    let errors: Vec<_> = files
                        .iter_mut()
                        .rev()
                        .filter_map(|file| file.rollback().err())
                        .map(|error| error.to_string())
                        .collect();
                    if !errors.is_empty() {
                        anyhow::bail!(
                            "{}; restoring metadata also failed: {}",
                            result.err().unwrap(),
                            errors.join("; ")
                        );
                    }
                }
                result
            }))?;
        let catalog = old.after_save(&requested, catalog);
        catalog.apply(state);
        *baseline = Some(catalog);
        Ok(report)
    }
    /// Persist public metadata before consuming recovery. A metadata failure
    /// must not discard an unanswered candidate. If checkpoint publication
    /// fails, restore the previous metadata and leave the prompt pending.
    pub fn commit_recovery(
        &self,
        previous: &mut AppState,
        candidate: &mut AppState,
        journal: &crate::recovery::Journal,
        snapshot: crate::recovery::Snapshot,
    ) -> Result<()> {
        candidate.validate()?;
        snapshot.validate()?;
        self.save(candidate)?;
        if let Err(error) = journal.consume(snapshot, candidate.restore_behavior) {
            // Roll back workspace/preferences only. Newly discovered resource
            // rows must not become deletions in this rollback's public snapshot.
            if let Some(catalog) = self
                .baseline
                .lock()
                .map_err(|_| anyhow::anyhow!("Resource Store is unavailable"))?
                .as_ref()
            {
                catalog.apply(previous);
            }
            if let Err(rollback) = self.save(previous) {
                anyhow::bail!(
                    "Could not consume recovery: {error}. Restoring the previous metadata also failed: {rollback}"
                );
            }
            return Err(error.context("Could not consume recovery; previous metadata was restored"));
        }
        Ok(())
    }
}

fn write_pending(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn read_optional(path: &std::path::Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

struct Publication {
    path: PathBuf,
    pending: PathBuf,
    before: Option<Vec<u8>>,
    published: bool,
}
impl Publication {
    fn stage(path: PathBuf, before: Option<Vec<u8>>, bytes: &[u8]) -> Result<Self> {
        let pending = path.with_extension("json.pending");
        write_pending(&pending, bytes)?;
        Ok(Self {
            path,
            pending,
            before,
            published: false,
        })
    }
    fn publish(&mut self) -> Result<()> {
        anyhow::ensure!(
            read_optional(&self.path)? == self.before,
            "Configuration changed in another window; reload before saving"
        );
        fs::rename(&self.pending, &self.path)?;
        self.published = true;
        Ok(())
    }
    fn rollback(&mut self) -> Result<()> {
        if !self.published {
            return Ok(());
        }
        if let Some(bytes) = &self.before {
            write_pending(&self.pending, bytes)?;
            fs::rename(&self.pending, &self.path)?;
        } else {
            fs::remove_file(&self.path)?;
        }
        self.published = false;
        Ok(())
    }
}
impl Drop for Publication {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.pending);
    }
}

#[cfg(test)]
mod startup_tests {
    use super::*;
    use nauterm_core::paths::{Environment, Platform};

    #[test]
    fn first_frame_preferences_never_open_the_encrypted_catalog() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let paths = ApplicationPaths::resolve(
            Platform::Macos,
            Environment {
                home: Some(temporary.path().to_owned()),
                ..Default::default()
            },
        )
        .for_client(nauterm_core::application::Client::NautermRs);
        fs::create_dir_all(&paths.rs_config_directory)?;
        fs::create_dir_all(&paths.data_directory)?;
        let config = paths.rs_config_directory.join("config.json");
        fs::write(
            &config,
            br#"{"schemaVersion":1,"appearance":{"theme":"dark"},"window":{"width":1100,"height":700}}"#,
        )?;
        // Deliberately invalid synthetic data: startup must not inspect it,
        // repair it or ask the OS for a database key to render the first frame.
        let database = paths.database();
        fs::write(&database, b"unopened synthetic catalog")?;
        let store = Store {
            config_directory: paths.rs_config_directory.clone(),
            state_directory: paths.rs_data_directory.clone(),
            paths,
            vault: OnceLock::new(),
            sync_vault: OnceLock::new(),
            baseline: Arc::new(Mutex::new(None)),
        };
        let preferences = store.initial_preferences()?;
        assert_eq!(preferences.app_theme(), crate::model::AppTheme::Dark);
        assert_eq!(store.initial_window_size()?.width, 1100.);
        let mut workspace = AppState::default();
        workspace.workspaces[0].name = "Public workspace".into();
        fs::write(
            store.state_directory.join("workspace.json"),
            serde_json::to_vec(&workspace)?,
        )?;
        let initial = store.initial_state()?;
        assert_eq!(initial.workspaces[0].name, "Public workspace");
        assert_eq!(initial.app_theme(), crate::model::AppTheme::Dark);
        assert!(initial.hosts.is_empty());
        assert!(store.vault.get().is_none());
        assert!(store.sync_vault.get().is_none());
        assert!(store.baseline.lock().unwrap().is_none());
        assert_eq!(fs::read(&database)?, b"unopened synthetic catalog");
        assert_eq!(
            fs::read(store.state_directory.join("workspace.json"))?,
            serde_json::to_vec(&workspace)?
        );
        Ok(())
    }
}
