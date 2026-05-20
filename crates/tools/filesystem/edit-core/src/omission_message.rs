pub fn format_omission_error(category: &str, omissions: &[String]) -> String {
    format!(
        "Omission detected in {category}. The following patterns suggest code was skipped: {}. \
         Please provide the complete content.",
        omissions.join(", ")
    )
}
