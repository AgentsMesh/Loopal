use std::io::Read;
use std::path::Path;

pub const CONFIG_JSON_BYTE_LIMIT: u64 = 4 * 1024 * 1024;

pub fn read(path: &Path) -> std::io::Result<String> {
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(CONFIG_JSON_BYTE_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > CONFIG_JSON_BYTE_LIMIT {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "configuration JSON exceeds 4 MiB",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_json_before_parsing() {
        let root = std::env::temp_dir().join(format!("loopal-json-limit-{}", std::process::id()));
        let _ = std::fs::remove_file(&root);
        std::fs::write(&root, vec![b'x'; CONFIG_JSON_BYTE_LIMIT as usize + 1]).unwrap();
        let error = read(&root).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        let _ = std::fs::remove_file(root);
    }
}
