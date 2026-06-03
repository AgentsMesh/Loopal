use ratatui::style::Color;

pub(crate) struct ActivityInputs {
    pub thinking: bool,
    pub compacting: bool,
    pub streaming: bool,
    pub pending_permission: bool,
    pub agent_idle: bool,
    pub has_subagents: bool,
    pub recently_or_active: bool,
}

pub(crate) fn pick_label(i: &ActivityInputs) -> (bool, Color, &'static str) {
    if i.thinking {
        (true, Color::Magenta, "Thinking")
    } else if i.compacting {
        (true, Color::Cyan, "Compacting")
    } else if i.streaming {
        (true, Color::Green, "Streaming")
    } else if i.pending_permission {
        (false, Color::Yellow, "Waiting")
    } else if !i.agent_idle {
        (true, Color::Cyan, "Working")
    } else if i.has_subagents {
        (true, Color::Blue, "Agents")
    } else if i.recently_or_active {
        (true, Color::Cyan, "Working")
    } else {
        (false, Color::DarkGray, "Idle")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> ActivityInputs {
        ActivityInputs {
            thinking: false,
            compacting: false,
            streaming: false,
            pending_permission: false,
            agent_idle: true,
            has_subagents: false,
            recently_or_active: false,
        }
    }

    #[test]
    fn idle_when_nothing_active() {
        let (spin, color, label) = pick_label(&base());
        assert_eq!((spin, color, label), (false, Color::DarkGray, "Idle"));
    }

    #[test]
    fn compacting_when_banner_present_and_agent_idle() {
        let i = ActivityInputs { compacting: true, ..base() };
        let (spin, color, label) = pick_label(&i);
        assert_eq!(label, "Compacting");
        assert_eq!(color, Color::Cyan);
        assert!(spin, "compacting must animate the spinner");
    }

    #[test]
    fn thinking_outranks_compacting() {
        let i = ActivityInputs { thinking: true, compacting: true, ..base() };
        assert_eq!(pick_label(&i).2, "Thinking");
    }

    #[test]
    fn compacting_outranks_streaming() {
        let i = ActivityInputs { compacting: true, streaming: true, ..base() };
        assert_eq!(pick_label(&i).2, "Compacting");
    }

    #[test]
    fn streaming_when_only_streaming() {
        let i = ActivityInputs { streaming: true, ..base() };
        assert_eq!(pick_label(&i).2, "Streaming");
    }

    #[test]
    fn waiting_uses_dot_not_spinner() {
        let i = ActivityInputs { pending_permission: true, ..base() };
        let (spin, color, label) = pick_label(&i);
        assert_eq!((spin, color, label), (false, Color::Yellow, "Waiting"));
    }

    #[test]
    fn working_when_backend_not_idle() {
        let i = ActivityInputs { agent_idle: false, ..base() };
        assert_eq!(pick_label(&i).2, "Working");
    }

    #[test]
    fn agents_when_subagents_live() {
        let i = ActivityInputs { has_subagents: true, ..base() };
        assert_eq!(pick_label(&i).2, "Agents");
    }

    #[test]
    fn working_during_activity_grace() {
        let i = ActivityInputs { recently_or_active: true, ..base() };
        assert_eq!(pick_label(&i).2, "Working");
    }
}
