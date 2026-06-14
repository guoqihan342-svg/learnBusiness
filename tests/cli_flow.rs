use assert_cmd::Command;

#[test]
fn prints_help_with_core_commands() {
    let mut cmd = Command::cargo_bin("biz-agent").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("ingest"))
        .stdout(predicates::str::contains("ask"))
        .stdout(predicates::str::contains("report"));
}
