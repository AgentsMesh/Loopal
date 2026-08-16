use std::sync::{Arc, OnceLock};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loopal_error::StorageError;
use loopal_output_guard::{validate_decoded_image, validate_inline_image};
use loopal_provider_api::{ContentBlock, Message};
use loopal_storage::{FileResourceStore, ResourceStore};
use loopal_tool_invocation::ToolImageBlock;
use loopal_turn::{ToolExecState, Turn, TurnStep};
use tracing::warn;

use crate::image_limits::{MAX_TOOL_IMAGES, image_byte_limit};

static RESOURCE_STORE: OnceLock<Option<Arc<dyn ResourceStore>>> = OnceLock::new();

pub fn resource_store() -> Option<Arc<dyn ResourceStore>> {
    RESOURCE_STORE
        .get_or_init(|| match FileResourceStore::new() {
            Ok(store) => Some(store as Arc<dyn ResourceStore>),
            Err(error) => {
                warn!(error = %error, "no resource store available; large images stay inline");
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
    configured_image_bytes: u64,
) -> Result<(), StorageError> {
    let max_bytes = image_byte_limit(configured_image_bytes);
    validate_image_count(images)?;
    let mut total_bytes = 0usize;
    let mut validated = Vec::with_capacity(images.len());
    for image in images.iter() {
        let ToolImageBlock::Inline { media_type, data } = image else {
            return Err(StorageError::ResourceIntegrity);
        };
        let image = validate_inline_image(media_type, data, max_bytes)
            .map_err(|_| StorageError::ResourceIntegrity)?;
        add_total(&mut total_bytes, image.byte_size(), max_bytes)?;
        validated.push(image);
    }
    for (image, validated) in images.iter_mut().zip(validated) {
        if validated.byte_size() < inline_threshold_bytes {
            continue;
        }
        let media_type = validated.media_type().to_string();
        let byte_size = validated.byte_size();
        match store
            .write(session_id, &media_type, validated.bytes())
            .await
        {
            Ok(id) => {
                *image = ToolImageBlock::SessionResource {
                    id,
                    media_type,
                    byte_size,
                };
            }
            Err(error) => {
                warn!(error = %error, "resource store write failed; keeping inline");
            }
        }
    }
    Ok(())
}

pub async fn hydrate_images(
    messages: &mut [Message],
    store: &dyn ResourceStore,
    session_id: &str,
    configured_image_bytes: u64,
) -> Result<(), StorageError> {
    for message in messages {
        for block in &mut message.content {
            let ContentBlock::ToolResult { images, .. } = block else {
                continue;
            };
            hydrate_image_blocks(images, store, session_id, configured_image_bytes).await?;
        }
    }
    Ok(())
}

pub async fn hydrate_turn_images(
    turns: &mut [Turn],
    store: &dyn ResourceStore,
    session_id: &str,
    configured_image_bytes: u64,
) -> Result<(), StorageError> {
    for turn in turns {
        for step in &mut turn.body.steps {
            let TurnStep::ToolBatch(batch) = step else {
                continue;
            };
            for item in &mut batch.items {
                if let ToolExecState::Done(result) = &mut item.state {
                    hydrate_image_blocks(
                        &mut result.images,
                        store,
                        session_id,
                        configured_image_bytes,
                    )
                    .await?;
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
    configured_image_bytes: u64,
) -> Result<(), StorageError> {
    validate_image_count(images)?;
    let max_bytes = image_byte_limit(configured_image_bytes);
    let mut total_bytes = 0usize;
    let mut replacements = Vec::with_capacity(images.len());
    for image in images.iter() {
        let (byte_size, replacement) = hydrate_image(image, store, session_id, max_bytes).await?;
        add_total(&mut total_bytes, byte_size, max_bytes)?;
        replacements.push(replacement);
    }
    for (image, replacement) in images.iter_mut().zip(replacements) {
        if let Some(replacement) = replacement {
            *image = replacement;
        }
    }
    Ok(())
}

async fn hydrate_image(
    image: &ToolImageBlock,
    store: &dyn ResourceStore,
    session_id: &str,
    max_bytes: usize,
) -> Result<(usize, Option<ToolImageBlock>), StorageError> {
    let (id, media_type, declared_bytes) = match image {
        ToolImageBlock::Inline { media_type, data } => {
            let validated = validate_inline_image(media_type, data, max_bytes)
                .map_err(|_| StorageError::ResourceIntegrity)?;
            return Ok((validated.byte_size(), None));
        }
        ToolImageBlock::SessionResource {
            id,
            media_type,
            byte_size,
        } => (id, media_type, *byte_size),
    };
    if declared_bytes > max_bytes {
        return Err(StorageError::ResourceByteLimitExceeded { max_bytes });
    }
    let bytes = store.read_bounded(session_id, id, max_bytes).await?;
    if bytes.len() != declared_bytes {
        return Err(StorageError::ResourceIntegrity);
    }
    let validated = validate_decoded_image(media_type, bytes, max_bytes)
        .map_err(|_| StorageError::ResourceIntegrity)?;
    let byte_size = validated.byte_size();
    let replacement = ToolImageBlock::Inline {
        media_type: validated.media_type().to_string(),
        data: STANDARD.encode(validated.bytes()),
    };
    Ok((byte_size, Some(replacement)))
}

fn validate_image_count(images: &[ToolImageBlock]) -> Result<(), StorageError> {
    if images.len() > MAX_TOOL_IMAGES {
        Err(StorageError::ResourceIntegrity)
    } else {
        Ok(())
    }
}

fn add_total(total: &mut usize, bytes: usize, max_bytes: usize) -> Result<(), StorageError> {
    *total = total
        .checked_add(bytes)
        .ok_or(StorageError::ResourceByteLimitExceeded { max_bytes })?;
    if *total > max_bytes {
        Err(StorageError::ResourceByteLimitExceeded { max_bytes })
    } else {
        Ok(())
    }
}
