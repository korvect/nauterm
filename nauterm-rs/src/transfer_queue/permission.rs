//! A failed task's explicit, one-operation sudo retry. Tickets never contain
//! credentials and become stale after cancel/dismiss/retry/shutdown.
use super::*;
use crate::sftp_client::{AuthorizationTarget, Sudo};

#[derive(Clone)]
pub struct PermissionTicket {
    service: WeakService,
    pub id: u64,
    pub attempt: u64,
    pub owner: u64,
}
pub struct PermissionTarget {
    pub authorization: AuthorizationTarget,
    pub label: String,
    pub source: String,
    pub target: String,
    pub error: String,
}
impl PermissionTicket {
    pub fn same(&self, other: &Self) -> bool {
        self.id == other.id
            && self.attempt == other.attempt
            && self.owner == other.owner
            && Weak::ptr_eq(&self.service.0, &other.service.0)
    }
    fn matches(&self, job: &Job) -> bool {
        job.id == self.id
            && job.attempt == self.attempt
            && job.owner == self.owner
            && job.status == Status::Failed
            && job.permission_retry
            && job.plan.is_some()
    }
    pub fn current(&self) -> bool {
        self.service.upgrade().is_some_and(|service| {
            let state = service.0.state.lock().unwrap();
            !state.closed
                && !state.draining
                && !is_shutting_down()
                && state.jobs.iter().any(|job| self.matches(job))
        })
    }
    pub fn target(&self) -> Result<PermissionTarget> {
        let service = self
            .service
            .upgrade()
            .context("The transfer queue closed.")?;
        let (plan, label, source, target, error) = {
            let state = service.0.state.lock().unwrap();
            ensure!(
                !state.closed && !state.draining && !is_shutting_down(),
                "The transfer queue is closing."
            );
            let job = state
                .jobs
                .iter()
                .find(|job| self.matches(job))
                .context("This retry request is no longer current.")?;
            (
                job.plan.as_ref().unwrap().clone(),
                job.label.clone(),
                job.source.clone(),
                job.target.clone(),
                job.error.clone().unwrap_or_default(),
            )
        };
        let core = match &plan.spec.core {
            Some(core) => core.clone(),
            None => Core::discover()?,
        };
        let remote = match &plan.spec.operation {
            Operation::Download { remote_path, .. } | Operation::Upload { remote_path, .. } => {
                remote_path
            }
            operation => operation.target(),
        };
        let directory = remote
            .rsplit_once('/')
            .map(|(parent, _)| if parent.is_empty() { "/" } else { parent })
            .context("This task does not have a remote directory.")?
            .to_owned();
        Ok(PermissionTarget {
            authorization: AuthorizationTarget {
                options: Arc::new(plan.spec.options.for_new_connection()),
                directory,
                sudo: Sudo::new_single_use(core)?,
            },
            label,
            source,
            target,
            error,
        })
    }
    pub fn retry(&self, sudo: Arc<Sudo>) -> Result<()> {
        ensure!(sudo.current(), "Sudo authorization expired before retry.");
        ensure!(
            sudo.single_use(),
            "This task requires a one-operation authorization."
        );
        let service = self
            .service
            .upgrade()
            .context("The transfer queue closed.")?;
        let mut state = service.0.state.lock().unwrap();
        ensure!(
            !state.closed && !state.draining && !is_shutting_down(),
            "The transfer queue is closing."
        );
        let job = state
            .jobs
            .iter_mut()
            .find(|job| self.matches(job))
            .context("This retry request is no longer current.")?;
        let previous = job.plan.as_ref().unwrap();
        let mut spec = previous.spec.clone();
        spec.core = Some(sudo.core());
        spec.sudo = Some(sudo);
        // Preserve the selected source, destination, replacement approval,
        // immutable payload/hooks and any private unpublished download stage.
        let download = previous.download.lock().unwrap().take();
        job.plan = Some(Arc::new(Plan {
            spec,
            download: Mutex::new(download),
        }));
        job.permission_retry = false;
        job.label.push_str(" · sudo retry");
        job.status = Status::Queued;
        job.error = None;
        Service::pump(&service.0, &mut state);
        service.wake();
        Ok(())
    }
}
impl Service {
    pub fn permission_ticket(&self, id: u64, attempt: u64) -> Option<PermissionTicket> {
        let state = self.0.state.lock().unwrap();
        if state.closed || state.draining || is_shutting_down() {
            return None;
        }
        state
            .jobs
            .iter()
            .find(|job| {
                job.id == id && job.attempt == attempt && job.permission_retry && job.plan.is_some()
            })
            .map(|job| PermissionTicket {
                service: self.downgrade(),
                id,
                attempt,
                owner: job.owner,
            })
    }
}
