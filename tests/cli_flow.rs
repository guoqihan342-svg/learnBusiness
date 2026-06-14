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

    assert!(
        workspace
            .path()
            .join(".agent-index/metadata.sqlite")
            .exists()
    );
}

#[test]
fn ask_returns_answer_with_source() {
    let workspace = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    std::fs::write(
        docs.path().join("process.txt"),
        "核心流程是申请、审核、归档。",
    )
    .unwrap();

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

    Command::cargo_bin("biz-agent")
        .unwrap()
        .args([
            "ask",
            "--workspace",
            workspace.path().to_str().unwrap(),
            "核心流程是什么？",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("process.txt"));
}

#[test]
fn describe_image_dry_run_shows_hash_without_ai_call() {
    let workspace = tempfile::tempdir().unwrap();
    let image_dir = tempfile::tempdir().unwrap();
    let image = image_dir.path().join("diagram.png");
    std::fs::write(&image, b"not a real png but enough for hashing").unwrap();

    Command::cargo_bin("biz-agent")
        .unwrap()
        .args(["init", workspace.path().to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("biz-agent")
        .unwrap()
        .args([
            "describe-image",
            image.to_str().unwrap(),
            "--workspace",
            workspace.path().to_str().unwrap(),
            "--dry-run-ai",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("dry-run"))
        .stdout(predicates::str::contains("sha256="));
}
