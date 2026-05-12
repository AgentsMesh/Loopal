pub fn supports_temperature(model: &str) -> bool {
    let m = model.to_lowercase();
    m.contains("claude-3") || m.contains("claude-2") || m.contains("claude-instant")
}
