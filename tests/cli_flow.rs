use assert_cmd::Command;

#[test]
fn prints_help_with_core_commands() {
    let mut cmd = Command::cargo_bin("learnBusiness").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("ingest"))
        .stdout(predicates::str::contains("ask"))
        .stdout(predicates::str::contains("report"));
}

#[test]
fn init_creates_learn_business_layout_with_config_folder() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("learnBusiness")
        .unwrap()
        .arg("init")
        .arg(temp.path())
        .assert()
        .success();

    assert!(temp.path().join(".learnBusiness/config/app.toml").exists());
    assert!(!temp.path().join(".learnBusiness/config.toml").exists());
    assert!(temp.path().join(".learnBusiness/artifacts/images").exists());
    assert!(temp.path().join(".learnBusiness/cache/ai").exists());
}

#[test]
fn ingest_indexes_text_document() {
    let workspace = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    std::fs::write(docs.path().join("policy.txt"), "客户准入规则").unwrap();

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args(["init", workspace.path().to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("learnBusiness")
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
            .join(".learnBusiness/metadata.sqlite")
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

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args(["init", workspace.path().to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "ingest",
            docs.path().to_str().unwrap(),
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("learnBusiness")
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

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args(["init", workspace.path().to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("learnBusiness")
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

#[test]
fn inspect_ai_lists_dry_run_image_audit_record() {
    let workspace = tempfile::tempdir().unwrap();
    let image_dir = tempfile::tempdir().unwrap();
    let image = image_dir.path().join("diagram.png");
    std::fs::write(&image, b"not a real png but enough for hashing").unwrap();

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args(["init", workspace.path().to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "describe-image",
            image.to_str().unwrap(),
            "--workspace",
            workspace.path().to_str().unwrap(),
            "--dry-run-ai",
        ])
        .assert()
        .success();

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "inspect-ai",
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("describe_image"))
        .stdout(predicates::str::contains("dry_run"));
}

#[test]
fn describe_image_dry_run_uses_configured_provider_metadata() {
    let workspace = tempfile::tempdir().unwrap();
    let image_dir = tempfile::tempdir().unwrap();
    let image = image_dir.path().join("diagram.png");
    std::fs::write(&image, b"not a real png but enough for hashing").unwrap();

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args(["init", workspace.path().to_str().unwrap()])
        .assert()
        .success();
    std::fs::write(
        workspace
            .path()
            .join(".learnBusiness")
            .join("config")
            .join("app.toml"),
        "\
[ai]
provider = \"ollama\"
base_url = \"http://127.0.0.1:11434\"
chat_model = \"qwen2.5\"
vision_model = \"llava\"
embedding_model = \"nomic-embed-text\"
api_key_env = \"\"
",
    )
    .unwrap();

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "describe-image",
            image.to_str().unwrap(),
            "--workspace",
            workspace.path().to_str().unwrap(),
            "--dry-run-ai",
        ])
        .assert()
        .success();

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "inspect-ai",
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("provider=ollama"))
        .stdout(predicates::str::contains("model=llava"));
}
