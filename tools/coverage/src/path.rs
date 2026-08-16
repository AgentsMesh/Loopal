use std::path::{Component, Path, PathBuf};

use crate::model::Manifest;

pub fn normalize(raw: &str, workspace: &Path, manifest: &Manifest) -> Option<String> {
    let raw = raw.trim().replace('\\', "/");
    candidates(&raw, workspace)
        .into_iter()
        .filter_map(clean)
        .find(|candidate| manifest.sources.contains(candidate))
}

fn candidates(raw: &str, workspace: &Path) -> Vec<String> {
    let mut values = vec![raw.to_owned()];
    let workspace = workspace.to_string_lossy().replace('\\', "/");
    if raw == workspace {
        values.push(String::new());
    } else if let Some(rest) = raw.strip_prefix(&(workspace + "/")) {
        values.push(rest.into());
    }
    if let Some((_, execroot_path)) = raw.rsplit_once("/execroot/")
        && let Some((_, workspace_path)) = execroot_path.split_once('/')
    {
        values.push(workspace_path.into());
    }
    values
}

fn clean(value: String) -> Option<String> {
    let mut out = PathBuf::new();
    for component in Path::new(&value).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out.to_string_lossy().replace('\\', "/"))
}
