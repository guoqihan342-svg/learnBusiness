use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

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
fn ingest_describe_images_indexes_mock_description_for_search() {
    let workspace = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    std::fs::write(docs.path().join("flow.png"), b"image bytes").unwrap();

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
            "--describe-images",
        ])
        .assert()
        .success();

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "search",
            "--workspace",
            workspace.path().to_str().unwrap(),
            "mock description",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("flow.png"))
        .stdout(predicates::str::contains("kind=image"))
        .stdout(predicates::str::contains("ai_generated=true"));
}

#[test]
fn ingest_describe_images_dry_run_does_not_index_description() {
    let workspace = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    std::fs::write(docs.path().join("flow.png"), b"image bytes").unwrap();

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
            "--describe-images",
            "--dry-run-ai",
        ])
        .assert()
        .success();

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "search",
            "--workspace",
            workspace.path().to_str().unwrap(),
            "mock description",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("没有检索命中"));

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "inspect-ai",
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("status=dry_run"));
}

#[test]
fn search_returns_local_results_without_ai_call() {
    let workspace = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    std::fs::write(docs.path().join("process.txt"), "客户申请后运营审核。").unwrap();

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
            "search",
            "--workspace",
            workspace.path().to_str().unwrap(),
            "--limit",
            "1",
            "运营审核",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("process.txt"))
        .stdout(predicates::str::contains("chunk="))
        .stdout(predicates::str::contains("score="))
        .stdout(predicates::str::contains("snippet="));

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

#[test]
fn ask_outputs_reasoning_process_and_operation_trace() {
    let workspace = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    std::fs::write(
        docs.path().join("approval.txt"),
        "approval workflow requires reviewer confirmation",
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

    let output = Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "ask",
            "--workspace",
            workspace.path().to_str().unwrap(),
            "approval workflow",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("推算过程"));
    assert!(stdout.contains("local_search"));
    assert!(stdout.contains("context_selection"));
    assert!(stdout.contains("ai_call"));
    assert!(stdout.contains("citation_binding"));
    assert!(stdout.contains("trace_id="));

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
            "inspect-trace",
            "--workspace",
            workspace.path().to_str().unwrap(),
            "--trace",
            &trace_id,
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(format!("trace_id={trace_id}")))
        .stdout(predicates::str::contains("operation=ask"))
        .stdout(predicates::str::contains("step=local_search"));
}

#[test]
fn search_writes_operation_trace_without_ai_call() {
    let workspace = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    std::fs::write(docs.path().join("policy.txt"), "approval policy").unwrap();

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
            "search",
            "--workspace",
            workspace.path().to_str().unwrap(),
            "approval",
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
        .stdout(predicates::str::contains("没有 AI 调用记录"));

    Command::cargo_bin("learnBusiness")
        .unwrap()
        .args([
            "inspect-trace",
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("operation=search"))
        .stdout(predicates::str::contains("step=search_text"))
        .stdout(predicates::str::contains("result_count=1"));
}

#[test]
fn ingest_writes_operation_trace_without_raw_document_text() {
    let workspace = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    std::fs::write(
        docs.path().join("sensitive.txt"),
        "approval secret@example.com 13800138000 sk-live-secret",
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
            "inspect-trace",
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("operation=ingest"))
        .stdout(predicates::str::contains("step=discover_documents"))
        .stdout(predicates::str::contains("step=extract_document"))
        .stdout(predicates::str::contains("step=write_index"))
        .stdout(predicates::str::contains("result_count=1"))
        .stdout(predicates::str::contains("secret@example.com").not())
        .stdout(predicates::str::contains("13800138000").not())
        .stdout(predicates::str::contains("sk-live-secret").not());
}

#[test]
fn describe_image_dry_run_writes_operation_trace() {
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
            "inspect-trace",
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("operation=describe-image"))
        .stdout(predicates::str::contains("step=ai_call"))
        .stdout(predicates::str::contains("status=dry_run"));
}
