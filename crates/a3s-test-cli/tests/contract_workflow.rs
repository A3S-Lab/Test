use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use a3s_test_agent::{
    ContractCandidate, ContractCandidateElement, ContractCandidateVariant,
    ContractGenerationProviderIdentity, ContractGenerationProviderResponse,
    ContractGenerationUsage, ContractSourceSpan, ContractWorkflowAdmission,
    ContractWorkflowArtifact, GeneratedContractProvenance, CONTRACT_GENERATION_PROVIDER_PROTOCOL,
};
use a3s_test_core::{ContractContext, ContractElement, ContractMode, ContractSeverity};
use sha2::{Digest, Sha256};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn contract_review_requires_explicit_human_decisions_and_publishes_acl_plus_audit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let prd = "Customers can place an order using the Place order button.";
    fs::write(temp.path().join("requirements.md"), prd).expect("PRD source");
    let generated = generated_contract_workflow(temp.path(), prd);
    let draft = temp.path().join("checkout.draft.json");
    fs::write(
        &draft,
        serde_json::to_vec_pretty(&generated).expect("generated workflow JSON"),
    )
    .expect("generated workflow");
    let review = temp.path().join("checkout.review.acl");
    fs::write(
        &review,
        r#"
contract_review {
    reviewer = "product-owner@example.test"

    candidate "prd:desktop:place-order" {
        action = "approve"
    }
}
"#,
    )
    .expect("review ACL");
    let contract = temp.path().join("checkout.acl");
    let audit = temp.path().join("checkout.reviewed.json");

    let output = Command::new(binary())
        .args([
            "contract",
            "review",
            "--draft",
            draft.to_str().unwrap(),
            "--review",
            review.to_str().unwrap(),
            "--output",
            contract.to_str().unwrap(),
            "--audit",
            audit.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run contract review");

    assert!(output.status.success(), "{output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("command JSON");
    assert_eq!(result["stage"], "reviewed");
    let acl = fs::read_to_string(&contract).expect("reviewed contract");
    let admitted = a3s_test_core::SurfaceContractDraft::from_acl(&acl)
        .expect("reviewed ACL")
        .admit()
        .expect("admitted reviewed contract");
    assert_eq!(admitted.name, "checkout");
    assert_eq!(
        admitted.variants[0].elements[0].name.as_deref(),
        Some("Place order")
    );
    let audit_value: serde_json::Value =
        serde_json::from_slice(&fs::read(&audit).expect("review audit")).expect("audit JSON");
    assert_eq!(audit_value["protocol"], "a3s.test.contract-workflow/1");
    assert_eq!(audit_value["stage"], "reviewed");
    assert!(audit_value["integrity_sha256"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:") && value.len() == 71));
    assert_eq!(
        audit_value["review"]["reviewer"],
        "product-owner@example.test"
    );
    assert!(audit_value["contract_acl"]
        .as_str()
        .is_some_and(|value| value == acl));
}

#[test]
fn contract_review_fails_closed_without_decisions_and_does_not_publish_outputs() {
    let temp = tempfile::tempdir().expect("tempdir");
    let prd = "Customers can place an order using the Place order button.";
    fs::write(temp.path().join("requirements.md"), prd).expect("PRD source");
    let draft = temp.path().join("checkout.draft.json");
    fs::write(
        &draft,
        serde_json::to_vec_pretty(&generated_contract_workflow(temp.path(), prd))
            .expect("generated workflow JSON"),
    )
    .expect("generated workflow");
    let review = temp.path().join("checkout.review.acl");
    fs::write(
        &review,
        "contract_review { reviewer = \"product-owner@example.test\" }",
    )
    .expect("empty review");
    let contract = temp.path().join("checkout.acl");
    let audit = temp.path().join("checkout.reviewed.json");

    let output = Command::new(binary())
        .args([
            "contract",
            "review",
            "--draft",
            draft.to_str().unwrap(),
            "--review",
            review.to_str().unwrap(),
            "--output",
            contract.to_str().unwrap(),
            "--audit",
            audit.to_str().unwrap(),
        ])
        .output()
        .expect("run invalid contract review");

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("at least one explicit candidate review decision"));
    assert!(!contract.exists());
    assert!(!audit.exists());
}

#[test]
fn contract_review_rehashes_source_evidence_before_accepting_a_saved_draft() {
    let temp = tempfile::tempdir().expect("tempdir");
    let prd = "Customers can place an order using the Place order button.";
    let source_path = temp.path().join("requirements.md");
    fs::write(&source_path, prd).expect("PRD source");
    let draft = temp.path().join("checkout.draft.json");
    fs::write(
        &draft,
        serde_json::to_vec_pretty(&generated_contract_workflow(temp.path(), prd))
            .expect("generated workflow JSON"),
    )
    .expect("generated workflow");
    fs::write(&source_path, "requirements changed after generation").expect("changed PRD");
    let review = temp.path().join("checkout.review.acl");
    fs::write(
        &review,
        r#"
contract_review {
    reviewer = "product-owner@example.test"
    candidate "prd:desktop:place-order" { action = "approve" }
}
"#,
    )
    .expect("review ACL");
    let contract = temp.path().join("checkout.acl");
    let audit = temp.path().join("checkout.reviewed.json");

    let output = Command::new(binary())
        .args([
            "contract",
            "review",
            "--draft",
            draft.to_str().unwrap(),
            "--review",
            review.to_str().unwrap(),
            "--output",
            contract.to_str().unwrap(),
            "--audit",
            audit.to_str().unwrap(),
        ])
        .output()
        .expect("run stale contract review");

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("source_mismatch"));
    assert!(!contract.exists());
    assert!(!audit.exists());
}

#[test]
fn contract_review_rejects_tampered_candidate_content_before_publishing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let prd = "Customers can place an order using the Place order button.";
    fs::write(temp.path().join("requirements.md"), prd).expect("PRD source");
    let mut workflow = generated_contract_workflow(temp.path(), prd);
    workflow["generated"]["candidates"][0]["variants"][0]["elements"][0]["element"]["name"] =
        serde_json::json!("Tampered action");
    let draft = temp.path().join("checkout.draft.json");
    fs::write(
        &draft,
        serde_json::to_vec_pretty(&workflow).expect("tampered workflow JSON"),
    )
    .expect("tampered workflow");
    let review = temp.path().join("checkout.review.acl");
    fs::write(
        &review,
        r#"
contract_review {
    reviewer = "product-owner@example.test"
    candidate "prd:desktop:place-order" { action = "approve" }
}
"#,
    )
    .expect("review ACL");
    let contract = temp.path().join("checkout.acl");
    let audit = temp.path().join("checkout.reviewed.json");

    let output = Command::new(binary())
        .args([
            "contract",
            "review",
            "--draft",
            draft.to_str().unwrap(),
            "--review",
            review.to_str().unwrap(),
            "--output",
            contract.to_str().unwrap(),
            "--audit",
            audit.to_str().unwrap(),
        ])
        .output()
        .expect("run tampered contract review");

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("integrity digest does not match its payload"));
    assert!(!contract.exists());
    assert!(!audit.exists());
}

#[cfg(unix)]
#[test]
fn contract_generate_calls_http_provider_with_digest_binding_and_env_authorization() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    let temp = tempfile::tempdir().expect("tempdir");
    let prd = "Customers can place an order using the Place order button.";
    fs::write(temp.path().join("requirements.md"), prd).expect("PRD source");
    let listener = TcpListener::bind("127.0.0.1:0").expect("provider listener");
    let address = listener.local_addr().expect("provider address");
    let response = provider_response(temp.path(), prd);
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("provider request");
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).expect("request line");
        let mut content_length = 0usize;
        let mut authorization = None;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("request header");
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.trim_end().split_once(':').expect("header shape");
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().expect("content length");
            }
            if name.eq_ignore_ascii_case("authorization") {
                authorization = Some(value.trim().to_string());
            }
        }
        let mut body = vec![0; content_length];
        reader.read_exact(&mut body).expect("provider body");
        let envelope: serde_json::Value = serde_json::from_slice(&body).expect("provider JSON");
        assert_eq!(request_line, "POST /contracts HTTP/1.1\r\n");
        assert_eq!(authorization.as_deref(), Some("Bearer workflow-secret"));
        assert_eq!(envelope["protocol"], CONTRACT_GENERATION_PROVIDER_PROTOCOL);
        assert_eq!(
            envelope["request"]["sources"][0]["sha256"],
            format!("sha256:{:x}", Sha256::digest(prd.as_bytes()))
        );
        let body = serde_json::to_vec(&serde_json::json!({
            "status": "success",
            "protocol": CONTRACT_GENERATION_PROVIDER_PROTOCOL,
            "response": response,
        }))
        .expect("provider response");
        let stream = reader.get_mut();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        )
        .expect("response head");
        stream.write_all(&body).expect("response body");
    });
    let config = temp.path().join("contract-generation.acl");
    fs::write(
        &config,
        format!(
            r#"
contract_generation "checkout" {{
    max_cost_microusd = 1000

    context {{
        mode = "operate"
        audience = ["customer"]
        primary_outcome = "place_order"
    }}

    provider {{
        name = "fixture-provider"
        model = "fixture-model"
        endpoint = "http://{address}/contracts"
        authorization_env = "A3S_TEST_PROVIDER_AUTHORIZATION_FIXTURE"
    }}

    source "prd" {{
        kind = "prd"
        path = "requirements.md"
        uri = "./requirements.md"
    }}
}}
"#
        ),
    )
    .expect("generation config");
    let output_path = temp.path().join("checkout.draft.json");
    let output = Command::new(binary())
        .env(
            "A3S_TEST_PROVIDER_AUTHORIZATION_FIXTURE",
            "Bearer workflow-secret",
        )
        .args([
            "contract",
            "generate",
            "--config",
            config.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run contract generation");
    server.join().expect("provider server");

    assert!(output.status.success(), "{output:?}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("workflow-secret"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("workflow-secret"));
    let workflow: serde_json::Value =
        serde_json::from_slice(&fs::read(output_path).expect("generated workflow artifact"))
            .expect("workflow JSON");
    assert_eq!(workflow["stage"], "generated");
    assert_eq!(workflow["generated"]["provider"]["model"], "fixture-model");
    assert!(workflow.get("review").is_none());
    assert!(workflow.get("contract_acl").is_none());
}

#[test]
fn contract_generate_rejects_source_escape_before_calling_a_provider() {
    let temp = tempfile::tempdir().expect("tempdir");
    let config = temp.path().join("contract-generation.acl");
    fs::write(
        &config,
        r#"
contract_generation "checkout" {
    max_cost_microusd = 1000
    context {
        mode = "operate"
        audience = ["customer"]
        primary_outcome = "place_order"
    }
    provider {
        name = "fixture-provider"
        model = "fixture-model"
        endpoint = "http://127.0.0.1:9/contracts"
    }
    source "prd" {
        kind = "prd"
        path = "../outside.md"
        uri = "./outside.md"
    }
}
"#,
    )
    .expect("generation config");
    let output_path = temp.path().join("checkout.draft.json");
    let output = Command::new(binary())
        .args([
            "contract",
            "generate",
            "--config",
            config.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("run invalid contract generation");

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("contract source path must stay inside the config directory"));
    assert!(!output_path.exists());
}

#[test]
fn contract_generate_rejects_provenance_uri_escape_before_calling_a_provider() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("requirements.md"), "requirements").expect("PRD source");
    let config = temp.path().join("contract-generation.acl");
    fs::write(
        &config,
        r#"
contract_generation "checkout" {
    max_cost_microusd = 1000
    context {
        mode = "operate"
        audience = ["customer"]
        primary_outcome = "place_order"
    }
    provider {
        name = "fixture-provider"
        model = "fixture-model"
        endpoint = "http://127.0.0.1:9/contracts"
    }
    source "prd" {
        kind = "prd"
        path = "requirements.md"
        uri = "../requirements.md"
    }
}
"#,
    )
    .expect("generation config");
    let output_path = temp.path().join("checkout.draft.json");
    let output = Command::new(binary())
        .args([
            "contract",
            "generate",
            "--config",
            config.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("run invalid contract generation");

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("contract source uri must be a contained relative path"));
    assert!(!output_path.exists());
}

#[cfg(unix)]
#[test]
fn contract_generate_rejects_existing_output_before_provider_dispatch() {
    use std::net::TcpListener;
    use std::sync::mpsc;

    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("requirements.md"), "requirements").expect("PRD source");
    let listener = TcpListener::bind("127.0.0.1:0").expect("provider listener");
    listener
        .set_nonblocking(true)
        .expect("nonblocking provider listener");
    let address = listener.local_addr().expect("provider address");
    let (sender, receiver) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        while std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok(_) => {
                    sender.send(true).expect("dispatch result");
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => panic!("provider accept failed: {error}"),
            }
        }
        sender.send(false).expect("dispatch result");
    });
    let config = temp.path().join("contract-generation.acl");
    fs::write(
        &config,
        format!(
            r#"
contract_generation "checkout" {{
    max_cost_microusd = 1000
    context {{
        mode = "operate"
        audience = ["customer"]
        primary_outcome = "place_order"
    }}
    provider {{
        name = "fixture-provider"
        model = "fixture-model"
        endpoint = "http://{address}/contracts"
    }}
    source "prd" {{
        kind = "prd"
        path = "requirements.md"
        uri = "./requirements.md"
    }}
}}
"#
        ),
    )
    .expect("generation config");
    let output_path = temp.path().join("checkout.draft.json");
    fs::write(&output_path, "keep existing draft").expect("existing output");

    let output = Command::new(binary())
        .args([
            "contract",
            "generate",
            "--config",
            config.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("run contract generation with existing output");
    server.join().expect("provider observer");

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert_eq!(
        fs::read_to_string(output_path).unwrap(),
        "keep existing draft"
    );
    assert!(!receiver.recv().expect("provider dispatch observation"));
}

fn generated_contract_workflow(root: &std::path::Path, prd: &str) -> serde_json::Value {
    let source = a3s_test_agent::ContractSource {
        id: "prd".to_string(),
        kind: a3s_test_agent::ContractSourceKind::Prd,
        uri: "./requirements.md".to_string(),
        path: root.join("requirements.md").to_string_lossy().into_owned(),
        sha256: format!("sha256:{:x}", Sha256::digest(prd.as_bytes())),
        media_type: None,
        width: None,
        height: None,
    };
    let options = a3s_test_agent::ContractGenerationOptions {
        timeout: std::time::Duration::from_secs(30),
        max_sources: 8,
        max_source_bytes: 8 * 1024 * 1024,
        max_candidates: 64,
        max_elements: 1024,
        max_string_bytes: 16 * 1024,
    };
    let admission =
        ContractWorkflowAdmission::new(vec![source], 1_000, &options).expect("workflow admission");
    serde_json::to_value(
        ContractWorkflowArtifact::generated(generated_contract_draft(root, prd), admission)
            .expect("generated workflow"),
    )
    .expect("workflow JSON")
}

fn provider_response(root: &std::path::Path, prd: &str) -> ContractGenerationProviderResponse {
    let generated = generated_contract_draft(root, prd);
    ContractGenerationProviderResponse {
        identity: generated.provider,
        source_digests: generated.provenance,
        candidates: generated.candidates,
        usage: generated.usage,
        request_id: generated.request_id,
    }
}

fn generated_contract_draft(
    _root: &std::path::Path,
    prd: &str,
) -> a3s_test_agent::GeneratedContractDraft {
    let source = GeneratedContractProvenance {
        source_id: "prd".to_string(),
        kind: a3s_test_agent::ContractSourceKind::Prd,
        uri: "./requirements.md".to_string(),
        sha256: format!("sha256:{:x}", Sha256::digest(prd.as_bytes())),
    };
    let quote = prd.to_string();
    a3s_test_agent::GeneratedContractDraft {
        name: "checkout".to_string(),
        version: 1,
        context: ContractContext {
            mode: ContractMode::Operate,
            audience: vec!["customer".to_string()],
            primary_outcome: "place_order".to_string(),
        },
        provenance: vec![source.clone()],
        candidates: vec![ContractCandidate {
            source_id: "prd".to_string(),
            context: ContractContext {
                mode: ContractMode::Operate,
                audience: vec!["customer".to_string()],
                primary_outcome: "place_order".to_string(),
            },
            variants: vec![ContractCandidateVariant {
                id: "desktop".to_string(),
                state: "ready".to_string(),
                min_width: None,
                max_width: None,
                theme: None,
                language: Some("en".to_string()),
                elements: vec![ContractCandidateElement {
                    element: ContractElement {
                        id: "place-order".to_string(),
                        test_id: Some("place-order".to_string()),
                        component_id: None,
                        role: Some("button".to_string()),
                        name: Some("Place order".to_string()),
                        description: None,
                        required: true,
                        visible: Some(true),
                        enabled: Some(true),
                        checked: None,
                        selected: None,
                        expanded: None,
                        readonly: None,
                        form_required: None,
                        invalid: None,
                        parent: None,
                        severity: ContractSeverity::Blocking,
                        citations: Vec::new(),
                    },
                    confidence: 90,
                    source_spans: vec![ContractSourceSpan {
                        source_id: "prd".to_string(),
                        quote,
                        start: 0,
                        end: prd.len() as u32,
                    }],
                    design_region: None,
                    unresolved_decision_ids: Vec::new(),
                }],
            }],
            unresolved_decisions: Vec::new(),
        }],
        conflicts: Vec::new(),
        unresolved_decisions: Vec::new(),
        usage: ContractGenerationUsage {
            input_tokens: 10,
            output_tokens: 10,
            cost_microusd: 100,
        },
        provider: ContractGenerationProviderIdentity {
            provider: "fixture-provider".to_string(),
            model: "fixture-model".to_string(),
        },
        request_id: Some(format!(
            "workflow-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Unix time")
                .as_nanos()
        )),
    }
}
