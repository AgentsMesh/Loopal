use std::path::Path;

use super::common::{discover_identity_or_exit, recipients_path_named, store_path};
use crate::{EditSession, SystemEditor};

pub async fn run(cwd: &Path, vault_name: &str) -> i32 {
    let Some(identity) = discover_identity_or_exit() else {
        return 1;
    };
    let session = EditSession {
        vault_path: &store_path(cwd, vault_name),
        recipients_path: &recipients_path_named(cwd, vault_name),
        identity: &identity,
    };
    match session.run(&SystemEditor) {
        Ok(()) => {
            println!("{vault_name}.vault updated");
            0
        }
        Err(e) => {
            eprintln!("edit failed: {e}");
            1
        }
    }
}
