//! A probe is optional and never interrupts the session's success/error flow.
use super::*;
use crate::host_platform::Target;

impl Nauterm {
    pub(super) fn detect_terminal_platform(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(target) = self
            .session(id)
            .and_then(SessionView::host_id)
            .and_then(|id| self.data.hosts.iter().find(|host| host.id == id))
            .and_then(Target::for_host)
        else {
            return;
        };
        if self.host_os_requests.contains(&target) || !self.resources_ready() {
            return;
        }
        let Some(view) = self.session(id) else {
            return;
        };
        let Some(options) = view.ssh.as_deref() else {
            return;
        };
        // An attempt-only port override can be a different server. It must not
        // change the platform metadata of the configured saved endpoint.
        if options.host != target.address || options.port != target.port {
            return;
        }
        let TabContent::Terminal(terminal) = &view.content else {
            return;
        };
        let Ok(request) = terminal.read(cx).host_os_request(options) else {
            return;
        };
        self.publish_host_platform(target, request, cx);
    }

    pub(super) fn detect_sftp_platform(
        &mut self,
        event: &crate::sftp::RemoteConnected,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self
            .data
            .hosts
            .iter()
            .find(|host| host.id == event.host_id)
            .and_then(Target::for_host)
        else {
            return;
        };
        if self.host_os_requests.contains(&target)
            || !self.resources_ready()
            || target.address != event.options.host
            || target.port != event.options.port
        {
            return;
        }
        let request = crate::native::HostOsRequest::for_sftp(&event.options, event.owner.clone());
        self.publish_host_platform(target, request, cx);
    }

    fn publish_host_platform(
        &mut self,
        target: Target,
        request: crate::native::HostOsRequest,
        cx: &mut Context<Self>,
    ) {
        let publication = self.store.detect_host_platform(target.clone(), request);
        self.host_os_requests.insert(target.clone());
        cx.spawn(async move |this, cx| {
            let change = publication.await;
            let _ = this.update(cx, |this, cx| {
                this.host_os_requests.remove(&target);
                if let Ok(Some(_)) = change {
                    let _ = this.store.apply_host_platform(&mut this.data, &target);
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
