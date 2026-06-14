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

#[test]
fn ingest_indexes_text_document() {
    let workspace = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    std::fs::write(docs.path().join("policy.txt"), "客户准入规则").unwrap();

    Command::cargo_bin("biz-agent")
        .unwrap()
        .args(["init", workspace.path().to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("biz-agent")
        .unwrap()
        .args([
            "ingest",
            docs.path().to_str().unwrap(),
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(workspace.path().join(".agent-index/metadata.sqlite").exists());
}
