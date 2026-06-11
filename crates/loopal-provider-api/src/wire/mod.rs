pub mod message;
pub mod normalize;
pub mod origin;
pub mod turn_projection;

pub use message::{ContentBlock, ImageSource, Message, MessageRole};
pub use normalize::normalize_messages;
pub use origin::MessageOrigin;
pub use turn_projection::{project_turn_to_messages, project_turns_to_messages, trigger_llm_text};
