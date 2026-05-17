use super::build_cli;

#[test]
fn vault_subcommand_is_registered() {
    let cmd = build_cli();
    let vault = cmd
        .get_subcommands()
        .find(|c| c.get_name() == "vault")
        .expect("vault subcommand must be attached");

    let names: Vec<&str> = vault.get_subcommands().map(|c| c.get_name()).collect();
    for expected in ["set", "get", "list", "edit", "rekey", "recipients"] {
        assert!(
            names.contains(&expected),
            "vault must expose {expected:?}, got {names:?}"
        );
    }
}

#[test]
fn vaults_subcommand_is_registered() {
    let cmd = build_cli();
    let vaults = cmd
        .get_subcommands()
        .find(|c| c.get_name() == "vaults")
        .expect("vaults subcommand must be attached");

    let names: Vec<&str> = vaults.get_subcommands().map(|c| c.get_name()).collect();
    for expected in ["init", "list", "remove"] {
        assert!(
            names.contains(&expected),
            "vaults must expose {expected:?}, got {names:?}"
        );
    }
}

#[test]
fn vault_has_name_flag_with_default() {
    let cmd = build_cli();
    let vault = cmd
        .get_subcommands()
        .find(|c| c.get_name() == "vault")
        .expect("vault subcommand");

    let name_arg = vault
        .get_arguments()
        .find(|a| a.get_id() == "name")
        .expect("vault must have --name arg");
    let default: Vec<_> = name_arg.get_default_values().iter().collect();
    assert_eq!(
        default,
        vec!["default"],
        "default vault name must be 'default'"
    );
}

#[test]
fn permission_accepts_yolo_alias() {
    let res = build_cli().try_get_matches_from(["loopal", "--permission", "yolo"]);
    assert!(res.is_ok(), "yolo must be a valid --permission value");
}

#[test]
fn permission_rejects_unknown_value() {
    let res = build_cli().try_get_matches_from(["loopal", "--permission", "wat"]);
    assert!(
        res.is_err(),
        "unknown --permission must be rejected by clap"
    );
}

#[test]
fn decision_lists_three_canonical_modes() {
    for mode in ["manual", "classifier", "agent"] {
        let res = build_cli().try_get_matches_from(["loopal", "--decision", mode]);
        assert!(res.is_ok(), "--decision {mode} must parse");
    }
}

#[test]
fn vault_dispatch_via_name_flag_parses() {
    let m = build_cli()
        .try_get_matches_from(["loopal", "vault", "--name", "personal", "list"])
        .expect("parse");
    let (sub_name, sub) = m.subcommand().expect("vault subcommand chosen");
    assert_eq!(sub_name, "vault");
    assert_eq!(
        sub.get_one::<String>("name").map(String::as_str),
        Some("personal")
    );
}
