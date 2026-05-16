use std::fs;
use std::path::Path;

use loopal_vault_api::{VaultError, VaultResult};

#[derive(Debug, Clone)]
pub struct RecipientEntry {
    pub line: String,
    pub label: String,
    pub recipient: age::ssh::Recipient,
}

impl RecipientEntry {
    pub fn parse(raw: &str) -> VaultResult<Self> {
        let trimmed = raw.trim();
        let recipient: age::ssh::Recipient =
            trimmed
                .parse()
                .map_err(|e: age::ssh::ParseRecipientKeyError| {
                    VaultError::InvalidRecipient(format!("{trimmed}: {e:?}"))
                })?;
        let label = extract_label(trimmed);
        Ok(Self {
            line: trimmed.to_string(),
            label,
            recipient,
        })
    }
}

fn extract_label(line: &str) -> String {
    let mut parts = line.splitn(3, char::is_whitespace);
    let _kind = parts.next();
    let key = parts.next();
    let comment = parts.next();
    if let Some(c) = comment {
        let c = c.trim();
        if !c.is_empty() {
            return c.to_string();
        }
    }
    if let Some(k) = key {
        let prefix: String = k.chars().take(8).collect();
        return format!("pk:{prefix}");
    }
    "<unknown>".to_string()
}

#[derive(Debug, Clone)]
enum Item {
    Recipient(Box<RecipientEntry>),
    Comment(String),
    Blank,
}

#[derive(Debug, Default, Clone)]
pub struct Recipients {
    items: Vec<Item>,
}

impl Recipients {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: &Path) -> VaultResult<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content =
            fs::read_to_string(path).map_err(|e| VaultError::Backend(format!("read: {e}")))?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> VaultResult<Self> {
        let mut items = Vec::new();
        for raw in content.lines() {
            let line = raw.trim_end();
            if line.trim().is_empty() {
                items.push(Item::Blank);
                continue;
            }
            if line.trim_start().starts_with('#') {
                items.push(Item::Comment(line.to_string()));
                continue;
            }
            let entry = RecipientEntry::parse(line)?;
            items.push(Item::Recipient(Box::new(entry)));
        }
        Ok(Self { items })
    }

    pub fn write(&self, path: &Path) -> VaultResult<()> {
        let mut buf = String::new();
        for item in &self.items {
            match item {
                Item::Recipient(e) => buf.push_str(&e.line),
                Item::Comment(c) => buf.push_str(c),
                Item::Blank => {}
            }
            buf.push('\n');
        }
        fs::write(path, buf).map_err(|e| VaultError::Backend(format!("write: {e}")))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = fs::metadata(path)
                .map_err(|e| VaultError::Backend(format!("stat: {e}")))?
                .permissions();
            p.set_mode(0o644);
            fs::set_permissions(path, p).map_err(|e| VaultError::Backend(format!("chmod: {e}")))?;
        }
        Ok(())
    }

    pub fn add_line(&mut self, line: &str) -> VaultResult<()> {
        let entry = RecipientEntry::parse(line)?;
        self.items.push(Item::Recipient(Box::new(entry)));
        Ok(())
    }

    pub fn remove_by_label(&mut self, label: &str) -> VaultResult<()> {
        let mut removed = false;
        self.items.retain(|item| match item {
            Item::Recipient(e) if !removed && matches_label(e, label) => {
                removed = true;
                false
            }
            _ => true,
        });
        if !removed {
            return Err(VaultError::RecipientNotFound(label.to_string()));
        }
        Ok(())
    }

    pub fn entries(&self) -> Vec<&RecipientEntry> {
        self.items
            .iter()
            .filter_map(|i| match i {
                Item::Recipient(e) => Some(e.as_ref()),
                _ => None,
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries().is_empty()
    }

    pub(crate) fn recipients_for_encryption(&self) -> Vec<Box<dyn age::Recipient + Send>> {
        self.entries()
            .iter()
            .map(|e| Box::new(e.recipient.clone()) as Box<dyn age::Recipient + Send>)
            .collect()
    }
}

fn matches_label(entry: &RecipientEntry, query: &str) -> bool {
    if entry.label == query {
        return true;
    }
    let mut parts = entry.line.splitn(3, char::is_whitespace);
    let _kind = parts.next();
    if let Some(key) = parts.next()
        && key.starts_with(query)
        && query.len() >= 6
    {
        return true;
    }
    false
}
