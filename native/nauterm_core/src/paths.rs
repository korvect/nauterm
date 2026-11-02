//! Per-client application paths. Pure resolution never opens application data.
use std::{env, path::PathBuf};

pub const DATABASE_FILE_NAME: &str = "nauterm.sqlite";

#[derive(Clone, Copy)]
pub enum Platform {
    Macos,
    Windows,
    Linux,
}

impl Platform {
    fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

#[derive(Default)]
pub struct Environment {
    pub home: Option<PathBuf>,
    pub user_profile: Option<PathBuf>,
    pub app_data: Option<PathBuf>,
    pub xdg_config: Option<PathBuf>,
    pub xdg_data: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationPaths {
    pub config_directory: PathBuf,
    pub data_directory: PathBuf,
    pub rs_config_directory: PathBuf,
    pub rs_data_directory: PathBuf,
    pub additional_theme_directories: Vec<PathBuf>,
}

impl ApplicationPaths {
    pub fn discover() -> Self {
        Self::resolve(
            Platform::current(),
            Environment {
                home: env::var_os("HOME").map(PathBuf::from),
                user_profile: env::var_os("USERPROFILE").map(PathBuf::from),
                app_data: env::var_os("APPDATA").map(PathBuf::from),
                xdg_config: env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
                xdg_data: env::var_os("XDG_DATA_HOME").map(PathBuf::from),
            },
        )
        .for_client(crate::application::current())
    }

    /// Pure path resolution; does not create directories or open any data.
    pub fn resolve(platform: Platform, environment: Environment) -> Self {
        let home = environment
            .home
            .or(environment.user_profile.clone())
            .unwrap_or_else(|| PathBuf::from("."));
        match platform {
            Platform::Macos => {
                let base = home.join("Library/Application Support");
                Self {
                    config_directory: base.join("Nauterm"),
                    data_directory: base.join("Nauterm"),
                    rs_config_directory: base.join("Nauterm RS"),
                    rs_data_directory: base.join("Nauterm RS"),
                    additional_theme_directories: vec![home.join(".config/Nauterm/themes")],
                }
            }
            Platform::Windows => {
                let base = environment.app_data.unwrap_or_else(|| {
                    environment
                        .user_profile
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join("AppData/Roaming")
                });
                Self {
                    config_directory: base.join("Nauterm"),
                    data_directory: base.join("Nauterm"),
                    rs_config_directory: base.join("Nauterm RS"),
                    rs_data_directory: base.join("Nauterm RS"),
                    additional_theme_directories: vec![],
                }
            }
            Platform::Linux => {
                let config = environment
                    .xdg_config
                    .unwrap_or_else(|| home.join(".config"));
                let data = environment
                    .xdg_data
                    .unwrap_or_else(|| home.join(".local/share"));
                Self {
                    config_directory: config.join("nauterm"),
                    data_directory: data.join("nauterm"),
                    rs_config_directory: config.join("nauterm-rs"),
                    rs_data_directory: data.join("nauterm-rs"),
                    additional_theme_directories: vec![],
                }
            }
        }
    }

    pub fn database(&self) -> PathBuf {
        self.data_directory.join(DATABASE_FILE_NAME)
    }

    /// Select the client's complete data root, including trust, themes, history
    /// and recordings. RS never falls back to the original client's themes.
    pub fn for_client(mut self, client: crate::application::Client) -> Self {
        if client == crate::application::Client::NautermRs {
            self.config_directory = self.rs_config_directory.clone();
            self.data_directory = self.rs_data_directory.clone();
            self.additional_theme_directories.clear();
        }
        self
    }

    /// An explicit development override isolates resources, preferences and
    /// recovery together. Normal launches use the selected client's roots.
    pub fn isolated_rs(mut self, directory: PathBuf) -> Self {
        self.config_directory = directory.clone();
        self.data_directory = directory.clone();
        self.rs_config_directory = directory.clone();
        self.rs_data_directory = directory;
        self.additional_theme_directories.clear();
        self
    }

    pub fn known_hosts(&self) -> PathBuf {
        self.config_directory.join("known_hosts")
    }

    pub fn themes(&self) -> PathBuf {
        self.config_directory.join("themes")
    }

    pub fn captures(&self) -> PathBuf {
        self.data_directory.join("terminal-logs")
    }

    pub fn shell_history(&self) -> PathBuf {
        self.data_directory.join("shell-history")
    }

    pub fn nauterm_workspace_state(&self) -> PathBuf {
        self.data_directory.join("workspace-state.json")
    }

    pub fn rs_workspace_state(&self) -> PathBuf {
        self.rs_data_directory.join("workspace-state.json")
    }

    pub fn rs_config(&self) -> PathBuf {
        self.rs_config_directory.join("config.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_resource_paths_are_disjoint_on_every_platform() {
        for platform in [Platform::Macos, Platform::Windows, Platform::Linux] {
            let original = ApplicationPaths::resolve(
                platform,
                Environment {
                    home: Some("/example/home".into()),
                    user_profile: Some("/example/user".into()),
                    app_data: Some("/example/roaming".into()),
                    xdg_config: Some("/example/config".into()),
                    xdg_data: Some("/example/data".into()),
                },
            );
            let rs = original
                .clone()
                .for_client(crate::application::Client::NautermRs);
            for (original, rs) in [
                (original.database(), rs.database()),
                (original.known_hosts(), rs.known_hosts()),
                (original.themes(), rs.themes()),
                (original.captures(), rs.captures()),
                (original.shell_history(), rs.shell_history()),
                (original.nauterm_workspace_state(), rs.rs_workspace_state()),
            ] {
                assert_ne!(original, rs);
            }
            assert_eq!(rs.config_directory, rs.rs_config_directory);
            assert_eq!(rs.data_directory, rs.rs_data_directory);
            assert!(rs.additional_theme_directories.is_empty());
        }
    }

    #[test]
    fn macos_keeps_rs_config_and_workspace_state_separate() {
        let paths = ApplicationPaths::resolve(
            Platform::Macos,
            Environment {
                home: Some("/example/home".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            paths.rs_config(),
            PathBuf::from("/example/home/Library/Application Support/Nauterm RS/config.json")
        );
        assert_eq!(
            paths.database(),
            PathBuf::from("/example/home/Library/Application Support/Nauterm/nauterm.sqlite")
        );
        assert_eq!(
            paths.known_hosts(),
            paths.config_directory.join("known_hosts")
        );
        assert_eq!(paths.captures(), paths.data_directory.join("terminal-logs"));
        assert_eq!(
            paths.shell_history(),
            paths.data_directory.join("shell-history")
        );
        assert_eq!(
            paths.rs_workspace_state(),
            PathBuf::from(
                "/example/home/Library/Application Support/Nauterm RS/workspace-state.json"
            )
        );
        assert_eq!(
            paths.nauterm_workspace_state(),
            PathBuf::from("/example/home/Library/Application Support/Nauterm/workspace-state.json")
        );
    }

    #[test]
    fn xdg_paths_keep_config_resources_out_of_data_directory() {
        let paths = ApplicationPaths::resolve(
            Platform::Linux,
            Environment {
                xdg_config: Some("/config".into()),
                xdg_data: Some("/data".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            paths.rs_config(),
            PathBuf::from("/config/nauterm-rs/config.json")
        );
        assert_eq!(
            paths.known_hosts(),
            PathBuf::from("/config/nauterm/known_hosts")
        );
        assert_eq!(
            paths.database(),
            PathBuf::from("/data/nauterm/nauterm.sqlite")
        );
        assert_eq!(
            paths.rs_workspace_state(),
            PathBuf::from("/data/nauterm-rs/workspace-state.json")
        );
        assert_eq!(
            paths.nauterm_workspace_state(),
            PathBuf::from("/data/nauterm/workspace-state.json")
        );
    }

    #[test]
    fn explicit_rs_override_isolates_all_resources_and_client_state() {
        let paths = ApplicationPaths::resolve(Platform::Macos, Environment::default())
            .isolated_rs("/isolated/rs".into());
        assert_eq!(
            paths.database(),
            PathBuf::from("/isolated/rs/nauterm.sqlite")
        );
        assert_eq!(
            paths.known_hosts(),
            PathBuf::from("/isolated/rs/known_hosts")
        );
        assert_eq!(
            paths.captures(),
            PathBuf::from("/isolated/rs/terminal-logs")
        );
        assert_eq!(
            paths.shell_history(),
            PathBuf::from("/isolated/rs/shell-history")
        );
        assert_eq!(paths.rs_config(), PathBuf::from("/isolated/rs/config.json"));
        assert_eq!(
            paths.rs_workspace_state(),
            PathBuf::from("/isolated/rs/workspace-state.json")
        );
        assert!(paths.additional_theme_directories.is_empty());
    }
}
