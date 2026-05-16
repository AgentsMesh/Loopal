use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod common;
mod edit_op;
mod kv_ops;
mod recipients_ops;
mod rekey_op;
mod vaults_ops;

pub use common::{DEFAULT_VAULT_NAME, validate_vault_name};

// ── `loopal vault[@name] <op>` — single-vault operations ────────────────

#[derive(Parser)]
#[command(name = "loopal vault", about = "Operate on a single vault")]
struct VaultCli {
    #[command(subcommand)]
    op: VaultCmd,
}

#[derive(Subcommand)]
enum VaultCmd {
    /// Write a secret (stdin recommended; --value for scripts)
    Set {
        name: String,
        #[arg(long)]
        value: Option<String>,
    },
    /// Print a secret's plaintext to stdout (use only in scripts)
    Get { name: String },
    /// List secret names in this vault (never shows values)
    List,
    /// Open vault YAML in $EDITOR (decrypted tempfile, re-encrypted on save)
    Edit,
    /// Re-encrypt vault with current recipients
    Rekey,
    /// SSH recipient management for this vault
    Recipients {
        #[command(subcommand)]
        op: RecipientsCmd,
    },
}

#[derive(Subcommand)]
pub(crate) enum RecipientsCmd {
    /// Add an SSH public key file as recipient + rekey
    Add { pubkey_path: PathBuf },
    /// Remove a recipient by label/comment + force rotation confirmation
    Remove { label: String },
    /// List recipients (one per line, with labels)
    List,
}

// ── `loopal vaults <op>` — set-wide operations ──────────────────────────

#[derive(Parser)]
#[command(name = "loopal vaults", about = "Manage the set of vaults")]
struct VaultsCli {
    #[command(subcommand)]
    op: VaultsCmd,
}

#[derive(Subcommand)]
enum VaultsCmd {
    /// Create a new vault (default name if omitted)
    Init { name: Option<String> },
    /// List all vaults
    List,
    /// Remove a vault (forces 'rotated' confirmation)
    Remove { name: String },
}

// ── Dispatch entry points ───────────────────────────────────────────────

/// Single-vault op. `vault_name` chosen by the caller (default vs `vault@X`).
pub async fn dispatch_vault(argv: Vec<String>, cwd: &std::path::Path, vault_name: &str) -> i32 {
    let cli = match VaultCli::try_parse_from(argv) {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() { 2 } else { 0 };
        }
    };
    match cli.op {
        VaultCmd::Set { name, value } => kv_ops::set(cwd, vault_name, &name, value).await,
        VaultCmd::Get { name } => kv_ops::get(cwd, vault_name, &name).await,
        VaultCmd::List => kv_ops::list(cwd, vault_name).await,
        VaultCmd::Edit => edit_op::run(cwd, vault_name).await,
        VaultCmd::Rekey => rekey_op::run(cwd, vault_name).await,
        VaultCmd::Recipients { op } => recipients_ops::run(cwd, vault_name, op).await,
    }
}

/// Vault-set op (init / list / remove across vaults).
pub async fn dispatch_vaults(argv: Vec<String>, cwd: &std::path::Path) -> i32 {
    let cli = match VaultsCli::try_parse_from(argv) {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() { 2 } else { 0 };
        }
    };
    match cli.op {
        VaultsCmd::Init { name } => {
            let n = name.as_deref().unwrap_or(DEFAULT_VAULT_NAME);
            vaults_ops::init(cwd, n).await
        }
        VaultsCmd::List => vaults_ops::list(cwd),
        VaultsCmd::Remove { name } => vaults_ops::remove(cwd, &name).await,
    }
}
