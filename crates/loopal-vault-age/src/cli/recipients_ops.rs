use std::io::{BufRead, Write};
use std::path::Path;

use loopal_vault_api::Vault;

use super::RecipientsCmd;
use super::common::{open_vault_or_exit, recipients_path_named};
use crate::Recipients;

pub async fn run(cwd: &Path, vault_name: &str, op: RecipientsCmd) -> i32 {
    match op {
        RecipientsCmd::List => list(cwd, vault_name),
        RecipientsCmd::Add { pubkey_path } => add(cwd, vault_name, &pubkey_path).await,
        RecipientsCmd::Remove { label } => remove(cwd, vault_name, &label).await,
    }
}

fn list(cwd: &Path, vault_name: &str) -> i32 {
    let rec_path = recipients_path_named(cwd, vault_name);
    let rec = match Recipients::load(&rec_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("failed to read {}: {e}", rec_path.display());
            return 1;
        }
    };
    let entries = rec.entries();
    if entries.is_empty() {
        println!("(no recipients in {vault_name}.vault)");
        return 0;
    }
    for e in entries {
        println!("{}\t{}", e.label, e.line);
    }
    0
}

async fn add(cwd: &Path, vault_name: &str, pubkey_path: &Path) -> i32 {
    let pubkey_line = match std::fs::read_to_string(pubkey_path) {
        Ok(s) => s.trim().to_string(),
        Err(e) => {
            eprintln!("failed to read {}: {e}", pubkey_path.display());
            return 1;
        }
    };
    let rec_path = recipients_path_named(cwd, vault_name);
    let mut rec = match Recipients::load(&rec_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("failed to load recipients: {e}");
            return 1;
        }
    };
    if let Err(e) = rec.add_line(&pubkey_line) {
        eprintln!("invalid public key: {e}");
        return 1;
    }
    if let Err(e) = rec.write(&rec_path) {
        eprintln!("failed to write {}: {e}", rec_path.display());
        return 1;
    }
    if let Some(store) = open_vault_or_exit(cwd, vault_name)
        && let Err(e) = store.rekey().await
    {
        eprintln!("failed to rekey vault: {e}");
        return 1;
    }
    println!("added recipient + rekeyed {vault_name}.vault");
    0
}

async fn remove(cwd: &Path, vault_name: &str, label: &str) -> i32 {
    let rec_path = recipients_path_named(cwd, vault_name);
    let mut rec = match Recipients::load(&rec_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("failed to load recipients: {e}");
            return 1;
        }
    };
    if let Err(e) = rec.remove_by_label(label) {
        eprintln!("{e}");
        return 1;
    }

    let names = list_secret_names(cwd, vault_name).await;
    if !force_rotation_confirmation(vault_name, label, &names) {
        eprintln!("aborted; recipient NOT removed");
        return 1;
    }
    if let Err(e) = rec.write(&rec_path) {
        eprintln!("failed to write {}: {e}", rec_path.display());
        return 1;
    }
    if let Some(store) = open_vault_or_exit(cwd, vault_name)
        && let Err(e) = store.rekey().await
    {
        eprintln!("failed to rekey vault: {e}");
        return 1;
    }
    println!("removed recipient {label} + rekeyed {vault_name}.vault");
    println!(
        "REMINDER: rotate the provider values for these {} secret(s):",
        names.len()
    );
    for n in &names {
        println!("  - {n}");
    }
    0
}

async fn list_secret_names(cwd: &Path, vault_name: &str) -> Vec<String> {
    match open_vault_or_exit(cwd, vault_name) {
        Some(store) => store.list_names().await,
        None => Vec::new(),
    }
}

fn force_rotation_confirmation(vault_name: &str, label: &str, names: &[String]) -> bool {
    eprintln!();
    eprintln!("⚠️  Removing recipient '{label}' from {vault_name}.vault.");
    eprintln!("    The removed recipient can still decrypt OLD ciphertext in git history.");
    eprintln!("    You MUST rotate the underlying values at the provider for:");
    if names.is_empty() {
        eprintln!("      (vault is empty — nothing to rotate)");
    } else {
        for n in names {
            eprintln!("      - {n}");
        }
    }
    eprintln!();
    eprint!("Type 'rotated' to confirm you have done so: ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).is_err() {
        return false;
    }
    line.trim() == "rotated"
}
