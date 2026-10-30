use clap::{CommandFactory, Parser};
use os_cli::{args::Cli, main_entry};

// spec: OUT-250 [M3]
#[test]
fn update_hook_is_explicit_and_requires_a_public_key() {
    assert!(
        Cli::try_parse_from(["openspider", "--check-update"])
            .unwrap()
            .run
            .check_update
    );
    assert_eq!(main_entry(["openspider", "--check-update"]), 2);
    assert_eq!(main_entry(["openspider", "--uninstall-cleanup"]), 2);
    let help = Cli::command().render_long_help().to_string();
    assert!(help.contains("--check-update"));
    assert!(!help.contains("--uninstall-cleanup"));
    assert!(Cli::try_parse_from(["openspider", "--check-update", "--run-task", "test"]).is_err());
}
