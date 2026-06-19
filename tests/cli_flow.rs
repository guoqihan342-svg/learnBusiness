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
        .stdout(predicates::str::contains("process.txt"))
        .stdout(predicates::str::contains("chunk="))
        .stdout(predicates::str::contains("score="));
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
        .stdout(predicates::str::contains("provider=mock"))
        .stdout(predicates::str::contains("model=business-vision"))
        .stdout(predicates::str::contains("purpose=describe_image"))
        .stdout(predicates::str::contains("input_hash="))
        .stdout(predicates::str::contains("sha256="))
        .stdout(predicates::str::contains("redaction="))
        .stdout(predicates::str::contains("token_estimate="))
        .stdout(predicates::str::contains("local_provider=false"));
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
        .stdout(predicates::str::contains("dry_run"))
        .stdout(predicates::str::contains("trace_id="))
        .stdout(predicates::str::contains("output_hash=-"))
        .stdout(predicates::str::contains("error_category=-"));
}

#[test]
fn inspect_ai_filters_by_trace_id() {
    let workspace = tempfile::tempdir().unwrap();
    let image_dir = tempfile::tempdir().unwrap();
    let first = image_dir.path().join("first.png");
    let second = image_dir.path().join("second.png");
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args(["init", workspace.path().to_str().unwrap()])
        .assert()
        .success();
    for image in [&first, &second] {
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
    }

    let output = Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "inspect-ai",
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let trace_id = stdout
        .lines()
        .find_map(|line| {
            line.split_whitespace()
                .find_map(|part| part.strip_prefix("trace_id="))
        })
        .unwrap()
        .to_string();

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "inspect-ai",
            "--workspace",
            workspace.path().to_str().unwrap(),
            "--trace",
            &trace_id,
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(format!("trace_id={trace_id}")))
        .stdout(predicates::str::contains("describe_image"));
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
provider = \"http\"
base_url = \"http://127.0.0.1:8000/v1\"
chat_model = \"business-chat\"
vision_model = \"business-vision\"
embedding_model = \"business-embedding\"
api_key_env = \"\"

[ai.headers]
Authorization = \"Bearer ${LEARNBUSINESS_TEST_KEY}\"
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
        .stdout(predicates::str::contains("provider=http"))
        .stdout(predicates::str::contains("model=business-vision"));
}

#[test]
fn describe_image_dry_run_rejects_invalid_http_base_url() {
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
provider = \"http\"
base_url = \"file:///tmp/model\"
chat_model = \"business-chat\"
vision_model = \"business-vision\"
embedding_model = \"business-embedding\"
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
        .failure()
        .stderr(predicates::str::contains("http or https"));

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "inspect-ai",
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("没有 AI 调用记录"));
}

#[test]
fn inspect_ai_lists_failed_provider_error_category() {
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
    std::fs::write(
        workspace
            .path()
            .join(".learnBusiness")
            .join("config")
            .join("app.toml"),
        "\
[ai]
provider = \"http\"
base_url = \"https://gateway.example.com/v1\"
chat_model = \"gpt-4o-mini\"
vision_model = \"gpt-4o-mini\"
embedding_model = \"text-embedding-3-small\"
api_key_env = \"LEARNBUSINESS_MISSING_API_KEY\"
",
    )
    .unwrap();

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "ask",
            "--workspace",
            workspace.path().to_str().unwrap(),
            "核心流程是什么？",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("API key"));

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "inspect-ai",
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("status=failed"))
        .stdout(predicates::str::contains("error_category=api_key_missing"))
        .stdout(predicates::str::contains("output_hash=-"));
}
