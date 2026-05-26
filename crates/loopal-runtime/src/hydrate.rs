use std::sync::{Arc, OnceLock};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loopal_error::StorageError;
use loopal_provider_api::{ContentBlock, Message};
use loopal_storage::{FileResourceStore, ResourceStore};
use loopal_tool_invocation::ToolImageBlock;
use loopal_turn::{ToolExecState, Turn, TurnStep};
use tracing::warn;

static RESOURCE_STORE: OnceLock<Option<Arc<dyn ResourceStore>>> = OnceLock::new();

pub fn resource_store() -> Option<Arc<dyn ResourceStore>> {
    RESOURCE_STORE
        .get_or_init(|| match FileResourceStore::new() {
            Ok(s) => Some(s as Arc<dyn ResourceStore>),
            Err(e) => {
                warn!(error = %e, "no resource store available; large images stay inline");
                None
            }
        })
        .clone()
}

pub async fn maybe_persist_inline_images(
    store: &dyn ResourceStore,
    session_id: &str,
    images: &mut [ToolImageBlock],
    inline_threshold_bytes: usize,
) {
    for img in images.iter_mut() {
        let ToolImageBlock::Inline { media_type, data } = img else {
            continue;
        };
        if data.len() * 3 / 4 < inline_threshold_bytes {
            continue;
        }
        let bytes = match STANDARD.decode(data.as_bytes()) {
            Ok(b) => b,
            Err(e) => {
                warn!(error = %e, "image data not valid base64; keeping inline");
                continue;
            }
        };
        let mt = media_type.clone();
        let byte_size = bytes.len();
        match store.write(session_id, &mt, &bytes).await {
            Ok(id) => {
                *img = ToolImageBlock::SessionResource {
                    id,
                    media_type: mt,
                    byte_size,
                };
            }
            Err(e) => {
                warn!(error = %e, "resource store write failed; keeping inline");
            }
        }
    }
}

pub async fn hydrate_images(
    messages: &mut [Message],
    store: &dyn ResourceStore,
    session_id: &str,
) -> Result<(), StorageError> {
    for msg in messages.iter_mut() {
        for block in msg.content.iter_mut() {
            let ContentBlock::ToolResult { images, .. } = block else {
                continue;
            };
            hydrate_image_blocks(images, store, session_id).await?;
        }
    }
    Ok(())
}

/// Walk every turn's ToolBatch step and inline-hydrate any SessionResource
/// image attachments. Called before sending turns to the provider.
pub async fn hydrate_turn_images(
    turns: &mut [Turn],
    store: &dyn ResourceStore,
    session_id: &str,
) -> Result<(), StorageError> {
    for turn in turns.iter_mut() {
        for step in turn.body.steps.iter_mut() {
            let TurnStep::ToolBatch(batch) = step else {
                continue;
            };
            for item in batch.items.iter_mut() {
                if let ToolExecState::Done(ref mut r) = item.state {
                    hydrate_image_blocks(&mut r.images, store, session_id).await?;
                }
            }
        }
    }
    Ok(())
}

async fn hydrate_image_blocks(
    images: &mut [ToolImageBlock],
    store: &dyn ResourceStore,
    session_id: &str,
) -> Result<(), StorageError> {
    for img in images.iter_mut() {
        let resource = match img {
            ToolImageBlock::SessionResource { id, media_type, .. } => {
                Some((id.clone(), media_type.clone()))
            }
            ToolImageBlock::Inline { .. } => None,
        };
        if let Some((id, media_type)) = resource {
            let bytes = store.read(session_id, &id).await?;
            let data = STANDARD.encode(&bytes);
            *img = ToolImageBlock::Inline { media_type, data };
        }
    }
    Ok(())
}
