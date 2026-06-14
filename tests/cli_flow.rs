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

#[test]
fn init_creates_agent_index_layout() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("biz-agent")
        .unwrap()
        .arg("init")
        .arg(temp.path())
        .assert()
        .success();

    assert!(temp.path().join(".agent-index/config.toml").exists());
    assert!(temp.path().join(".agent-index/artifacts/images").exists());
    assert!(temp.path().join(".agent-index/cache/ai").exists());
}
