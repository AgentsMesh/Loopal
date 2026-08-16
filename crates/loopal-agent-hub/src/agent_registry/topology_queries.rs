use crate::topology::AgentInfo;

use super::AgentRegistry;

impl AgentRegistry {
    pub fn descendants(&self, name: &str) -> Vec<String> {
        let mut descendants = Vec::new();
        let Some(root_generation) = self.generation(name) else {
            return descendants;
        };
        let mut pending = vec![(name.to_string(), root_generation)];
        while let Some((parent, parent_generation)) = pending.pop() {
            let children = self
                .agent_info(&parent)
                .map(|info| info.children.clone())
                .unwrap_or_default();
            for child in children {
                if self.parent_generation(&child) != Some(parent_generation) {
                    continue;
                }
                if let Some(child_generation) = self.generation(&child) {
                    pending.push((child.clone(), child_generation));
                }
                descendants.push(child);
            }
        }
        descendants
    }

    fn parent_generation(&self, name: &str) -> Option<u64> {
        self.agents
            .get(name)
            .and_then(|agent| agent.parent_generation)
            .or_else(|| {
                self.completed
                    .get(name)
                    .and_then(|agent| agent.parent_generation)
            })
    }

    pub fn topology_snapshot(&self) -> serde_json::Value {
        let mut agents: Vec<serde_json::Value> = self
            .agents
            .iter()
            .map(|(name, agent)| (name, &agent.info, agent.state.is_shadow()))
            .chain(
                self.completed
                    .iter()
                    .map(|(name, agent)| (name, &agent.info, agent.shadow)),
            )
            .map(topology_entry)
            .collect();
        agents.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        serde_json::json!({ "agents": agents })
    }
}

fn topology_entry((name, info, shadow): (&String, &AgentInfo, bool)) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "parent": info.parent.as_ref().map(|parent| parent.to_string()),
        "children": info.children,
        "lifecycle": info.lifecycle.state(),
        "error": info.lifecycle.error(),
        "model": info.model,
        "shadow": shadow,
    })
}
