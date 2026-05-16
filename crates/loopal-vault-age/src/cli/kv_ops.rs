use std::path::Path;

use loopal_vault_api::Vault;
use secrecy::{ExposeSecret, SecretString};

use super::common::open_vault_or_exit;

pub async fn set(cwd: &Path, vault_name: &str, name: &str, value: Option<String>) -> i32 {
    let Some(store) = open_vault_or_exit(cwd, vault_name) else {
        return 1;
    };
    let resolved = match value {
        Some(v) => v,
        None => {
            use std::io::Read;
            let mut buf = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
                eprintln!("failed to read value from stdin: {e}");
                return 1;
            }
            buf.trim_end_matches(['\n', '\r']).to_string()
        }
    };
    if let Err(e) = store.put(name, SecretString::from(resolved)).await {
        eprintln!("failed to set: {e}");
        return 1;
    }
    println!("set {name} in {vault_name}.vault");
    0
}

pub async fn get(cwd: &Path, vault_name: &str, name: &str) -> i32 {
    let Some(store) = open_vault_or_exit(cwd, vault_name) else {
        return 1;
    };
    match store.get(name).await {
        Some(v) => {
            print!("{}", v.expose_secret());
            0
        }
        None => {
            eprintln!("no such secret: {name} in {vault_name}.vault");
            1
        }
    }
}

pub async fn list(cwd: &Path, vault_name: &str) -> i32 {
    let Some(store) = open_vault_or_exit(cwd, vault_name) else {
        return 1;
    };
    let names = store.list_names().await;
    if names.is_empty() {
        println!("(no secrets in {vault_name}.vault)");
    } else {
        for n in names {
            println!("{n}");
        }
    }
    0
}
