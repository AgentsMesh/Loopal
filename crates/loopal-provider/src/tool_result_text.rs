pub const IMAGE_ATTACHED_PLACEHOLDER: &str = "[Image attached]";

pub fn placeholder_text(content: &str, has_images: bool) -> &str {
    if content.is_empty() && has_images {
        IMAGE_ATTACHED_PLACEHOLDER
    } else {
        content
    }
}
