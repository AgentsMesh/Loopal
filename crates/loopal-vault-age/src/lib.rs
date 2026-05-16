pub mod cli;
pub mod discovery;
pub mod editor;
pub mod identity;
pub mod recipients;
pub mod ssh_agent;
pub mod store;
pub(crate) mod vault_io;

pub use discovery::list_initialized_vaults;
pub use editor::{EditSession, EditorAction, SystemEditor};
pub use identity::{DiscoveredIdentity, discover, discover_in, load};
pub use recipients::{RecipientEntry, Recipients};
pub use ssh_agent::{is_agent_available, passphrase_warning};
pub use store::AgeVault;
