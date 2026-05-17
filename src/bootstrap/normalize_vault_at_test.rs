use super::normalize_vault_at_syntax;

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

#[test]
fn no_args_returns_unchanged() {
    let out = normalize_vault_at_syntax(argv(&["loopal"])).unwrap();
    assert_eq!(out, vec!["loopal"]);
}

#[test]
fn non_vault_argv_returns_unchanged() {
    let input = argv(&["loopal", "hello", "world"]);
    let out = normalize_vault_at_syntax(input.clone()).unwrap();
    assert_eq!(out, input);
}

#[test]
fn plain_vault_returns_unchanged() {
    let input = argv(&["loopal", "vault", "list"]);
    let out = normalize_vault_at_syntax(input.clone()).unwrap();
    assert_eq!(out, input);
}

#[test]
fn vault_at_name_is_rewritten_to_name_flag() {
    let out = normalize_vault_at_syntax(argv(&["loopal", "vault@production", "set", "k"])).unwrap();
    assert_eq!(
        out,
        vec!["loopal", "vault", "--name", "production", "set", "k"]
    );
}

#[test]
fn vault_at_default_is_rewritten_too() {
    let out = normalize_vault_at_syntax(argv(&["loopal", "vault@default", "list"])).unwrap();
    assert_eq!(out, vec!["loopal", "vault", "--name", "default", "list"]);
}

#[test]
fn vault_at_invalid_name_returns_err() {
    let res = normalize_vault_at_syntax(argv(&["loopal", "vault@../etc/passwd", "list"]));
    assert!(res.is_err(), "path-traversal name must be rejected");
}

#[test]
fn vault_at_empty_name_returns_err() {
    let res = normalize_vault_at_syntax(argv(&["loopal", "vault@", "list"]));
    assert!(res.is_err(), "empty name must be rejected");
}

#[test]
fn vaults_plural_is_not_normalized() {
    let input = argv(&["loopal", "vaults", "init"]);
    let out = normalize_vault_at_syntax(input.clone()).unwrap();
    assert_eq!(out, input, "`vaults` (set ops) must not be rewritten");
}

#[test]
fn normalized_args_parse_through_build_cli_with_correct_name() {
    let args = normalize_vault_at_syntax(argv(&["loopal", "vault@personal", "list"])).unwrap();
    let m = crate::cli::build_cli()
        .try_get_matches_from(args)
        .expect("clap must parse normalized args");
    let (sub_name, sub) = m.subcommand().expect("vault subcommand chosen");
    assert_eq!(sub_name, "vault");
    assert_eq!(
        sub.get_one::<String>("name").map(String::as_str),
        Some("personal"),
        "normalized --name must reach build_cli matches"
    );
    let (op, _) = sub.subcommand().expect("vault subcommand has op");
    assert_eq!(op, "list");
}

#[test]
fn plain_vault_parses_with_default_name_via_build_cli() {
    let args = normalize_vault_at_syntax(argv(&["loopal", "vault", "list"])).unwrap();
    let m = crate::cli::build_cli()
        .try_get_matches_from(args)
        .expect("clap must parse plain `vault list`");
    let (_, sub) = m.subcommand().expect("vault subcommand");
    assert_eq!(
        sub.get_one::<String>("name").map(String::as_str),
        Some("default"),
        "missing --name must fall back to default via clap default_value"
    );
}
