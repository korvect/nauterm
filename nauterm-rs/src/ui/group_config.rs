//! Group settings are copied into editor drafts, matching the reference form.
use super::*;

impl Nauterm {
    pub(super) fn set_editor_ssh_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let Some(editor) = self
            .editor
            .as_mut()
            .filter(|editor| matches!(editor.kind, ItemKind::Host | ItemKind::Group))
            && let Some(draft) = &mut editor.host
            && !draft.busy
        {
            draft.ssh_enabled = enabled;
            draft.ssh_configured = true;
            if !enabled {
                draft.cancel_inheritance();
            }
        }
        cx.notify();
    }
    pub(super) fn group_editor_sections(
        &self,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let draft = editor.host.as_ref().unwrap();
        let mut sections = vec![self.editor_section(
            "General",
            vec![
                self.editor_field(editor, 0, "Name *"),
                self.parent_group_field(editor),
            ],
        )];
        if draft.ssh_enabled {
            sections.push(self.ssh_editor_section(editor, cx));
        }
        if draft.telnet.enabled {
            sections.push(self.telnet_editor_section(editor, cx));
        }
        sections.push(self.add_protocol_control(editor, cx));
        sections
    }
    pub(super) fn inherit_editor_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let group = self
            .editor
            .as_ref()
            .and_then(|editor| editor.group)
            .and_then(|id| self.data.groups.iter().find(|group| group.id == id))
            .cloned();
        let Some(editor) = &mut self.editor else {
            return;
        };
        let Some(draft) = &mut editor.host else {
            return;
        };
        let Some(group) = group else {
            draft.cancel_inheritance();
            cx.notify();
            return;
        };
        if draft.kind == HostKind::Local || draft.busy {
            return;
        }
        // A Group's Proxy is independent of whether it defines SSH defaults.
        editor.proxy_id = editor.proxy_id.or(group.proxy_id);
        if let Some(parent) = &group.telnet {
            draft.telnet.inherit(parent, window, cx);
        }
        if !draft.ssh_configured && group.ssh.is_some() {
            draft.ssh_enabled = true;
            draft.ssh_configured = true;
        }
        if draft.ssh_enabled
            && let Some(config) = &group.mosh
        {
            draft.mosh.inherit(config, window, cx);
        }
        let Some(profile) = group.profile() else {
            self.load_editor_password(window, cx);
            cx.notify();
            return;
        };
        if !draft.ssh_enabled {
            self.load_editor_password(window, cx);
            cx.notify();
            return;
        }
        let mut host = Host {
            port: editor.fields[2]
                .1
                .read(cx)
                .value()
                .trim()
                .parse()
                .unwrap_or(0),
            username: editor.fields[3].1.read(cx).value().trim().to_owned(),
            credential: editor.credential,
            proxy_id: editor.proxy_id,
            theme: editor.theme.as_ref().and_then(|draft| draft.selected),
            ssh: draft.setup.clone(),
            ..Default::default()
        };
        let previous_password = host.ssh.password_id.clone();
        draft.inheritance.password_configured = editor
            .selects
            .as_ref()
            .is_some_and(|selects| selects.auth.manual_password_present(cx));
        crate::host_session::inherit_defaults(
            &mut host,
            &profile,
            group.proxy_id,
            &mut draft.inheritance,
        );
        if editor.fields[2].1.read(cx).value().as_ref() != host.port.to_string() {
            editor.fields[2].1.update(cx, |input, cx| {
                input.set_value(host.port.to_string(), window, cx)
            });
        }
        if editor.fields[3].1.read(cx).value().as_ref() != host.username {
            editor.fields[3]
                .1
                .update(cx, |input, cx| input.set_value(host.username, window, cx));
        }
        editor.credential = host.credential;
        if let Some(selects) = &mut editor.selects {
            selects
                .auth
                .inherited_password(&previous_password, &host.ssh.password_id);
        }
        editor.proxy_id = host.proxy_id;
        if let Some(theme) = &mut editor.theme {
            theme.selected = host.theme;
        }
        draft.setup = host.ssh;
        if let Some(id) = profile.setup.environment_id
            && (draft.setup.environment_id.as_ref() != Some(&id) || draft.environment_changed)
            && !draft.environment_merges.contains(&id)
        {
            draft.environment_merges.push(id);
        }
        self.merge_editor_environments(cx);
        self.load_editor_password(window, cx);
        cx.notify();
    }
    pub(super) fn merge_editor_environments(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let token = editor.fields[0].1.entity_id();
        let Some(draft) = &mut editor.host else {
            return;
        };
        if draft.environment_merges.is_empty() {
            return;
        }
        draft.inheritance_generation = draft.inheritance_generation.wrapping_add(1);
        let generation = draft.inheritance_generation;
        draft.inheriting = true;
        draft.inheritance_error = None;
        let local = draft.environment.clone();
        let base = draft.setup.environment_id.clone();
        let inherited = draft.environment_merges.clone();
        let vault = self.ensure_keychain(cx).read(cx).vault();
        cx.spawn(async move |this, cx| {
            let result = match vault {
                Some(vault) => vault.merge_host_environments(local, base, inherited).await,
                None => Err(anyhow::anyhow!("The encrypted Group environment could not be loaded. Resolve the Keychain storage error and retry.")),
            };
            let _ = this.update(cx, |this, cx| {
                let Some(editor) = this.find_editor_mut(&|editor| editor.fields[0].1.entity_id() == token) else { return; };
                let Some(draft) = &mut editor.host else { return; };
                if draft.inheritance_generation != generation { return; }
                draft.inheriting = false;
                match result {
                    Ok(values) => { draft.environment = Some(values); draft.environment_changed = true; draft.environment_merges.clear(); draft.inheritance_error = None; }
                    Err(error) => draft.inheritance_error = Some(error.to_string()),
                }
                cx.notify();
            });
        }).detach();
    }
}
