fn principal(facts: AgentRuntimeFacts) -> AgentPrincipal {
    AgentPrincipal::new(AgentExecutionRef::local(ROOT_AGENT_NAME, 1), facts)
}

#[test]
fn derives_owner_only_from_bound_managed_root_facts() {
    let root = tempfile::tempdir().unwrap();
    let mut facts = AgentRuntimeFacts::root(root.path().into(), SpawnAuthority::default());
    facts.session_id = Some("session-safe".into());
    let derived = owner(&principal(facts.clone()), &facts).unwrap();
    assert_eq!(derived.session_id, "session-safe");
    assert_eq!(derived.root_agent.agent, ROOT_AGENT_NAME);

    facts.session_id = None;
    assert!(owner(&principal(facts.clone()), &facts).is_err());
    facts.session_id = Some("../forged".into());
    assert!(owner(&principal(facts.clone()), &facts).is_err());
}

#[test]
fn rejects_child_external_and_remote_root_shapes() {
    let root = tempfile::tempdir().unwrap();
    let mut facts = AgentRuntimeFacts::root(root.path().into(), SpawnAuthority::default());
    facts.session_id = Some("session-safe".into());

    let mut child = facts.clone();
    child.origin = AgentOrigin::ManagedChild;
    child.depth = 1;
    child.parent = Some(AgentExecutionRef::local(ROOT_AGENT_NAME, 1));
    assert!(owner(&principal(child.clone()), &child).is_err());

    let mut external = facts.clone();
    external.origin = AgentOrigin::ExternalTcp;
    assert!(owner(&principal(external.clone()), &external).is_err());

    let mut remote = principal(facts.clone());
    remote.execution.address.hub.push("other-hub".into());
    assert!(owner(&remote, &facts).is_err());
}

#[tokio::test]
async fn capability_absence_fails_closed_after_exact_authority_check() {
    let (events, _rx) = mpsc::channel(8);
    let mut hub = Hub::new(events);
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = Connection::new(transport).into_listening().0;
    let execution = hub
        .registry
        .register_connection_with_parent_execution(ROOT_AGENT_NAME, connection, None, None, None)
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut facts = AgentRuntimeFacts::root(root.path().into(), SpawnAuthority::default());
    facts.session_id = Some("session-safe".into());
    assert!(hub.registry.set_runtime_facts(&execution, facts.clone()));
    let hub = Arc::new(Mutex::new(hub));
    let error = authority(&hub, &AgentPrincipal::new(execution, facts))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("backend is unavailable"));
}
