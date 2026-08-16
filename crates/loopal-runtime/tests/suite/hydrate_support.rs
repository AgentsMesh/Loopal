use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loopal_error::StorageError;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_storage::ResourceStore;
use loopal_tool_invocation::ToolImageBlock;

pub const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

pub fn png(size: usize) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.resize(size.max(bytes.len()), 0);
    bytes
}

pub fn b64(data: &[u8]) -> String {
    STANDARD.encode(data)
}

pub fn message(images: Vec<ToolImageBlock>) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tu".into(),
            content: String::new(),
            images,
            is_error: false,
            metadata: None,
        }],
        origin: None,
        ephemeral_in_history: false,
    }
}

pub fn images(message: &Message) -> &[ToolImageBlock] {
    let ContentBlock::ToolResult { images, .. } = &message.content[0] else {
        panic!("expected tool result")
    };
    images
}

pub struct WriteFailingStore;

#[async_trait]
impl ResourceStore for WriteFailingStore {
    async fn write(&self, _: &str, _: &str, _: &[u8]) -> Result<String, StorageError> {
        Err(StorageError::Io(std::io::Error::other(
            "simulated write failure",
        )))
    }

    async fn read_bounded(&self, _: &str, _: &str, _: usize) -> Result<Vec<u8>, StorageError> {
        unreachable!("read not invoked")
    }

    async fn delete_session(&self, _: &str) -> Result<(), StorageError> {
        Ok(())
    }
}

pub struct ReadFailingStore;

#[async_trait]
impl ResourceStore for ReadFailingStore {
    async fn write(&self, _: &str, _: &str, _: &[u8]) -> Result<String, StorageError> {
        unreachable!("write not invoked")
    }

    async fn read_bounded(&self, _: &str, _: &str, _: usize) -> Result<Vec<u8>, StorageError> {
        Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "simulated read failure",
        )))
    }

    async fn delete_session(&self, _: &str) -> Result<(), StorageError> {
        Ok(())
    }
}
