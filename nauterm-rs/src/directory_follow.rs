//! Event-driven SSH-directory following, independent of rendering and I/O.
use crate::model::Id;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub host_id: Option<Id>,
}
impl Endpoint {
    pub fn matches(&self, other: &Self) -> bool {
        self.host == other.host
            && self.port == other.port
            && self.username == other.username
            && !matches!((self.host_id, other.host_id), (Some(a), Some(b)) if a != b)
    }
}

#[derive(Default)]
pub struct DirectoryFollow {
    pub enabled: bool,
    directory: Option<String>,
    requested: Option<String>,
}
impl DirectoryFollow {
    pub fn directory(&self) -> Option<&str> {
        self.directory.as_deref()
    }
    pub fn update(&mut self, directory: Option<String>) -> bool {
        let directory = directory.filter(|path| {
            crate::recovery::safe_directory(path)
                && (path.starts_with('/') || path == "~" || path.starts_with("~/"))
        });
        if self.directory == directory {
            return false;
        }
        self.directory = directory;
        if self.directory.is_none() {
            self.requested = None;
        }
        true
    }
    pub fn rebind(&mut self, directory: Option<String>) {
        self.requested = None;
        self.update(directory);
    }
    pub fn stop(&mut self) {
        self.enabled = false;
        self.requested = None;
    }
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        self.requested = None;
    }
    /// Mark only an eligible event as requested. Busy/hidden/editing panes keep
    /// the newest directory pending until their ordinary completion callback.
    pub fn target(&mut self, related: bool, blocked: bool, current: &str) -> Option<String> {
        if !self.enabled || !related || blocked {
            return None;
        }
        let directory = self.directory.as_ref()?;
        if self.requested.as_ref() == Some(directory) {
            return None;
        }
        self.requested = Some(directory.clone());
        if directory.trim_end_matches('/') == current.trim_end_matches('/') {
            return None;
        }
        Some(directory.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_same_host_identity_port_and_user_may_follow() {
        let source = Endpoint {
            host: "example".into(),
            port: 22,
            username: "user".into(),
            host_id: Some(3),
        };
        assert!(source.matches(&source));
        for other in [
            Endpoint {
                host: "other".into(),
                ..source.clone()
            },
            Endpoint {
                username: "root".into(),
                ..source.clone()
            },
            Endpoint {
                port: 23,
                ..source.clone()
            },
            Endpoint {
                host_id: Some(4),
                ..source.clone()
            },
        ] {
            assert!(!source.matches(&other));
        }
    }
    #[test]
    fn pending_updates_coalesce_and_hidden_or_busy_panes_do_not_lose_the_latest_path() {
        let mut follow = DirectoryFollow::default();
        follow.toggle();
        assert!(follow.update(Some("/one".into())));
        assert!(!follow.update(Some("/one".into())));
        assert!(follow.target(true, true, "/").is_none());
        follow.update(Some("/two/工作区".into()));
        assert!(follow.target(false, false, "/").is_none());
        assert_eq!(follow.target(true, false, "/"), Some("/two/工作区".into()));
        assert!(follow.target(true, false, "/").is_none());
        follow.stop();
        follow.update(Some("/three".into()));
        assert!(follow.target(true, false, "/").is_none());
        follow.toggle();
        assert_eq!(follow.target(true, false, "/"), Some("/three".into()));
    }
    #[test]
    fn already_current_and_invalid_paths_do_not_cause_requests_or_retry_loops() {
        let mut follow = DirectoryFollow::default();
        follow.toggle();
        follow.update(Some("/current/".into()));
        assert!(follow.target(true, false, "/current").is_none());
        for path in ["", "relative", "/bad\ncommand", "/bad\u{1b}[m"] {
            follow.update(Some(path.into()));
            assert!(follow.directory().is_none());
            assert!(follow.target(true, false, "/").is_none());
        }
    }
    #[test]
    fn new_connections_can_follow_the_same_directory_again_without_polling() {
        let mut follow = DirectoryFollow::default();
        follow.toggle();
        follow.update(Some("/work".into()));
        assert_eq!(follow.target(true, false, "/"), Some("/work".into()));
        assert!(follow.update(None));
        assert!(follow.target(true, false, "/").is_none());
        follow.update(Some("/work".into()));
        assert_eq!(follow.target(true, false, "/"), Some("/work".into()));
        follow.rebind(Some("/work".into()));
        assert!(follow.target(false, false, "/").is_none());
        assert!(follow.target(true, true, "/").is_none());
        assert_eq!(follow.target(true, false, "/"), Some("/work".into()));
        assert!(follow.target(true, false, "/").is_none());
        follow.stop();
        follow.rebind(Some("/different".into()));
        assert!(!follow.enabled);
        assert!(follow.target(true, false, "/").is_none());
    }
}
