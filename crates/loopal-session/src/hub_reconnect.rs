/// Address + token a UI client needs to re-attach to a Hub it just
/// detached from. Bootstrap injects this into UI state at attach time;
/// the UI surfaces it to the user on `/detach-hub` so they can reconnect.
#[derive(Clone, Debug)]
pub struct HubReconnectInfo {
    pub addr: String,
    pub token: String,
}
