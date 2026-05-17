use std::path::PathBuf;

use clap::{Arg, ArgMatches, Command};

mod common;
mod edit_op;
mod kv_ops;
mod recipients_ops;
mod rekey_op;
mod vaults_ops;

pub use common::{DEFAULT_VAULT_NAME, validate_vault_name};

pub(crate) enum RecipientsCmd {
    Add { pubkey_path: PathBuf },
    Remove { label: String },
    List,
}

pub fn vault_command() -> Command {
    let name_arg = Arg::new("name")
        .long("name")
        .value_name("NAME")
        .default_value(DEFAULT_VAULT_NAME)
        .help("Vault name (defaults to 'default'; equivalent to legacy 'vault@<name>' syntax)");

    Command::new("vault")
        .about("Operate on a single vault")
        .arg(name_arg)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("set")
                .about("Write a secret (stdin recommended; --value for scripts)")
                .arg(
                    Arg::new("key")
                        .required(true)
                        .value_name("KEY")
                        .help("Secret name"),
                )
                .arg(
                    Arg::new("value")
                        .long("value")
                        .value_name("VALUE")
                        .help("Plaintext value (omit to read from stdin)"),
                ),
        )
        .subcommand(
            Command::new("get")
                .about("Print a secret's plaintext to stdout (use only in scripts)")
                .arg(
                    Arg::new("key")
                        .required(true)
                        .value_name("KEY")
                        .help("Secret name to retrieve"),
                ),
        )
        .subcommand(Command::new("list").about("List secret names in this vault (never values)"))
        .subcommand(
            Command::new("edit")
                .about("Open vault YAML in $EDITOR (decrypted tempfile, re-encrypted on save)"),
        )
        .subcommand(Command::new("rekey").about("Re-encrypt vault with current recipients"))
        .subcommand(
            Command::new("recipients")
                .about("SSH recipient management for this vault")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("add")
                        .about("Add an SSH public key file as recipient + rekey")
                        .arg(
                            Arg::new("pubkey_path")
                                .required(true)
                                .value_name("PUBKEY_PATH")
                                .value_parser(clap::value_parser!(PathBuf))
                                .help(
                                    "Path to an SSH public key file (e.g. ~/.ssh/id_ed25519.pub)",
                                ),
                        ),
                )
                .subcommand(
                    Command::new("remove")
                        .about("Remove a recipient by label/comment + force rotation confirmation")
                        .arg(Arg::new("label").required(true).value_name("LABEL").help(
                            "Recipient label/comment (suffix after the key in `recipients` file)",
                        )),
                )
                .subcommand(
                    Command::new("list").about("List recipients (one per line, with labels)"),
                ),
        )
}

pub fn vaults_command() -> Command {
    Command::new("vaults")
        .about("Manage the set of vaults")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("init")
                .about("Create a new vault (default name if omitted)")
                .arg(
                    Arg::new("name")
                        .value_name("NAME")
                        .help("Vault name to create (defaults to 'default')"),
                ),
        )
        .subcommand(Command::new("list").about("List all vaults"))
        .subcommand(
            Command::new("remove")
                .about("Remove a vault (forces 'rotated' confirmation)")
                .arg(
                    Arg::new("name")
                        .required(true)
                        .value_name("NAME")
                        .help("Vault name to remove"),
                ),
        )
}

pub async fn dispatch_vault(matches: &ArgMatches, cwd: &std::path::Path) -> i32 {
    let vault_name = matches
        .get_one::<String>("name")
        .expect("clap default_value on vault --name guarantees Some")
        .as_str();
    if let Err(e) = validate_vault_name(vault_name) {
        eprintln!("{e}");
        return 2;
    }

    match matches.subcommand() {
        Some(("set", sub)) => {
            let key = sub
                .get_one::<String>("key")
                .expect("clap requires <KEY> for `vault set`");
            let value = sub.get_one::<String>("value").cloned();
            kv_ops::set(cwd, vault_name, key, value).await
        }
        Some(("get", sub)) => {
            let key = sub
                .get_one::<String>("key")
                .expect("clap requires <KEY> for `vault get`");
            kv_ops::get(cwd, vault_name, key).await
        }
        Some(("list", _)) => kv_ops::list(cwd, vault_name).await,
        Some(("edit", _)) => edit_op::run(cwd, vault_name).await,
        Some(("rekey", _)) => rekey_op::run(cwd, vault_name).await,
        Some(("recipients", sub)) => {
            let op = match sub.subcommand() {
                Some(("add", a)) => RecipientsCmd::Add {
                    pubkey_path: a
                        .get_one::<PathBuf>("pubkey_path")
                        .cloned()
                        .expect("clap requires <PUBKEY_PATH> for `vault recipients add`"),
                },
                Some(("remove", a)) => RecipientsCmd::Remove {
                    label: a
                        .get_one::<String>("label")
                        .cloned()
                        .expect("clap requires <LABEL> for `vault recipients remove`"),
                },
                Some(("list", _)) => RecipientsCmd::List,
                _ => unreachable!("`vault recipients` subcommand_required guarantees a match"),
            };
            recipients_ops::run(cwd, vault_name, op).await
        }
        _ => unreachable!("`vault` subcommand_required guarantees a match"),
    }
}

pub async fn dispatch_vaults(matches: &ArgMatches, cwd: &std::path::Path) -> i32 {
    match matches.subcommand() {
        Some(("init", sub)) => {
            let name = sub
                .get_one::<String>("name")
                .map(String::as_str)
                .unwrap_or(DEFAULT_VAULT_NAME);
            vaults_ops::init(cwd, name).await
        }
        Some(("list", _)) => vaults_ops::list(cwd),
        Some(("remove", sub)) => {
            let name = sub
                .get_one::<String>("name")
                .expect("clap requires <NAME> for `vaults remove`");
            vaults_ops::remove(cwd, name).await
        }
        _ => unreachable!("`vaults` subcommand_required guarantees a match"),
    }
}
