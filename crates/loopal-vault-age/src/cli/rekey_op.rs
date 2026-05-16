use std::path::Path;

use loopal_vault_api::Vault;

use super::common::open_vault_or_exit;

pub async fn run(cwd: &Path, vault_name: &str) -> i32 {
    let Some(store) = open_vault_or_exit(cwd, vault_name) else {
        return 1;
    };
    match store.rekey().await {
        Ok(()) => {
            println!("{vault_name}.vault rekeyed with current recipients");
            0
        }
        Err(e) => {
            eprintln!("rekey failed: {e}");
            1
        }
    }
}
