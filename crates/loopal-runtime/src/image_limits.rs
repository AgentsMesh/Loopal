pub(crate) const MAX_TOOL_IMAGES: usize = 16;
const ABSOLUTE_MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

pub(crate) fn image_byte_limit(configured_bytes: u64) -> usize {
    configured_bytes.min(ABSOLUTE_MAX_IMAGE_BYTES) as usize
}
