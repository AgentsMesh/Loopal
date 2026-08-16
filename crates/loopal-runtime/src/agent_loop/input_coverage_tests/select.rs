use loopal_protocol::{AgentStatus, ControlCommand, MessageSource};
use tokio::sync::mpsc;

use super::super::SelectResult;
use super::support::{envelope, make_fixture};
use crate::agent_input::AgentInput;

#[tokio::test]
async fn select_input_covers_scheduled_and_rewake_channel_matrix() {
    let mut fixture = make_fixture();
    let (scheduled_tx, scheduled_rx) = mpsc::channel(1);
    let (rewake_tx, rewake_rx) = mpsc::channel(1);
    fixture.runner.trigger_rx = Some(scheduled_rx);
    fixture.runner.rewake_rx = Some(rewake_rx);
    let expected = envelope(MessageSource::Scheduled, "both-rewake");
    let expected_id = expected.id;
    let (selected, sent) = tokio::join!(fixture.runner.select_input(), rewake_tx.send(expected),);
    sent.unwrap();
    assert!(matches!(selected, SelectResult::Envelope(value) if value.id == expected_id));
    drop(scheduled_tx);

    let mut fixture = make_fixture();
    let (scheduled_tx, scheduled_rx) = mpsc::channel(1);
    let (rewake_tx, rewake_rx) = mpsc::channel(1);
    fixture.runner.trigger_rx = Some(scheduled_rx);
    fixture.runner.rewake_rx = Some(rewake_rx);
    let expected = envelope(MessageSource::Scheduled, "both-scheduled");
    let expected_id = expected.id;
    let (selected, sent) =
        tokio::join!(fixture.runner.select_input(), scheduled_tx.send(expected),);
    sent.unwrap();
    assert!(matches!(selected, SelectResult::Envelope(value) if value.id == expected_id));
    drop(rewake_tx);

    let mut fixture = make_fixture();
    let (scheduled_tx, scheduled_rx) = mpsc::channel(1);
    fixture.runner.trigger_rx = Some(scheduled_rx);
    let (selected, ()) = tokio::join!(fixture.runner.select_input(), async move {
        drop(scheduled_tx);
    });
    assert!(matches!(selected, SelectResult::ChannelClosed));
    assert!(fixture.runner.trigger_rx.is_none());

    let mut fixture = make_fixture();
    let (rewake_tx, rewake_rx) = mpsc::channel(1);
    fixture.runner.rewake_rx = Some(rewake_rx);
    let (selected, ()) = tokio::join!(fixture.runner.select_input(), async move {
        drop(rewake_tx);
    });
    assert!(matches!(selected, SelectResult::ChannelClosed));
    assert!(fixture.runner.rewake_rx.is_none());
}

#[tokio::test]
async fn try_select_preserves_deferred_order_and_suspend_gates_automatic_input() {
    let mut fixture = make_fixture();
    fixture
        .runner
        .deferred_frontend_inputs
        .push_back(AgentInput::Message(envelope(
            MessageSource::Agent("older".into()),
            "older",
        )));
    fixture
        .input_tx
        .send(AgentInput::Message(envelope(MessageSource::Human, "newer")))
        .await
        .unwrap();
    let selected = fixture.runner.try_select_input().await;
    assert!(matches!(
        selected,
        SelectResult::AgentInput(Some(AgentInput::Message(value)))
            if value.content.text == "older"
    ));
    let selected = fixture.runner.try_select_input().await;
    assert!(matches!(
        selected,
        SelectResult::AgentInput(Some(AgentInput::Message(value)))
            if value.content.text == "newer"
    ));

    let mut fixture = make_fixture();
    let (rewake_tx, rewake_rx) = mpsc::channel(1);
    fixture.runner.rewake_rx = Some(rewake_rx);
    rewake_tx
        .send(envelope(MessageSource::Scheduled, "automatic"))
        .await
        .unwrap();
    assert!(matches!(
        fixture.runner.try_select_input().await,
        SelectResult::Envelope(value) if value.content.text == "automatic"
    ));

    let mut fixture = make_fixture();
    fixture.runner.status = AgentStatus::Suspended;
    drop(fixture.input_tx);
    assert!(matches!(
        fixture.runner.try_select_input().await,
        SelectResult::AgentInput(None)
    ));
    assert!(
        fixture
            .runner
            .should_defer_frontend_input(&AgentInput::Message(envelope(
                MessageSource::Scheduled,
                "defer"
            )))
    );
    assert!(
        !fixture
            .runner
            .should_defer_frontend_input(&AgentInput::Control(ControlCommand::Clear))
    );
}
