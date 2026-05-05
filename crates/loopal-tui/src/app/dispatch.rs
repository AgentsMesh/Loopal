use super::App;

impl App {
    pub fn dispatch_event(&mut self, event: loopal_protocol::AgentEvent) {
        if let loopal_protocol::AgentEventPayload::SubAgentSpawned {
            ref name,
            ref parent,
            ..
        } = event.payload
            && !self.view_clients.contains_key(name)
        {
            let parent_name = parent.as_ref().map(|p| p.agent.clone());
            let vc = crate::view_client::ViewClient::empty(name);
            if let Some(parent_name) = parent_name {
                vc.with_view_mut(|view| view.parent = Some(parent_name));
            }
            self.view_clients.insert(name.clone(), vc);
        }
        for vc in self.view_clients.values() {
            vc.apply_event(&event);
        }
        self.session.handle_event(event);
    }

    pub fn push_system_message(&self, content: String) {
        self.with_active_conversation_mut(|conv| {
            conv.messages.push(loopal_view_state::SessionMessage {
                role: "system".into(),
                content,
                ui_local: true,
                ..Default::default()
            });
        });
    }

    pub fn set_transient_status(&mut self, msg: impl Into<String>) {
        self.transient_status = Some((msg.into(), std::time::Instant::now()));
    }

    /// Clear transient status, but only if it has been visible for at least
    /// 1 second — short flashes (e.g. paste failed → user immediately confirms
    /// modal) must remain readable until 3s natural expiry.
    pub fn clear_transient_status(&mut self) {
        if let Some((_, t)) = self.transient_status.as_ref()
            && t.elapsed() >= std::time::Duration::from_secs(1)
        {
            self.transient_status = None;
        }
    }

    pub fn current_transient_status(&self) -> Option<&str> {
        let (msg, t) = self.transient_status.as_ref()?;
        if t.elapsed() < std::time::Duration::from_secs(3) {
            Some(msg.as_str())
        } else {
            None
        }
    }

    pub fn push_welcome(&self, model: &str, path: &str) {
        let banner = loopal_view_state::SessionMessage {
            role: "welcome".into(),
            content: format!("{model}\n{path}"),
            ui_local: true,
            ..Default::default()
        };
        if let Some(vc) = self.view_clients.get("main") {
            vc.with_conversation_mut(|conv| conv.messages.push(banner));
        }
    }

    pub fn load_display_history(&self, projected: Vec<loopal_protocol::ProjectedMessage>) {
        let msgs: Vec<loopal_view_state::SessionMessage> = projected
            .into_iter()
            .map(loopal_session::into_session_message)
            .map(|mut m| {
                m.ui_local = true;
                m
            })
            .collect();
        if let Some(vc) = self.view_clients.get("main") {
            vc.with_conversation_mut(|conv| conv.messages = msgs);
        }
    }

    pub fn load_sub_agent_history(
        &mut self,
        name: &str,
        session_id: &str,
        parent: Option<&str>,
        model: Option<&str>,
        projected: Vec<loopal_protocol::ProjectedMessage>,
    ) {
        let msgs: Vec<loopal_view_state::SessionMessage> = projected
            .into_iter()
            .map(loopal_session::into_session_message)
            .map(|mut m| {
                m.ui_local = true;
                m
            })
            .collect();
        if !self.view_clients.contains_key(name) {
            self.view_clients.insert(
                name.to_string(),
                crate::view_client::ViewClient::empty(name),
            );
        }
        let vc = self.view_clients.get(name).expect("inserted above");
        vc.with_view_mut(|view| {
            view.session_id = Some(session_id.to_string());
            view.parent = parent.map(|s| s.to_string());
            if let Some(m) = model {
                view.observable.model = m.to_string();
            }
            view.observable.status = loopal_protocol::AgentStatus::Finished;
            view.conversation.messages = msgs;
        });
    }
}
