use std::process::ExitCode;

use a3s_test_core::ACTION_PROTOCOL_REVISION;
use anyhow::Result;
use serde_json::json;

use super::args::SchemaArgs;
use crate::action_schema::interactive_action_schema;

pub(super) fn execute(args: SchemaArgs) -> Result<ExitCode> {
    let schema = json!({
        "protocol_revision": ACTION_PROTOCOL_REVISION,
        "planner": "external_coding_agent",
        "turns": [
            "start",
            "observe",
            "act",
            "observe",
            "finish"
        ],
        "invariants": {
            "typed_actions": true,
            "ref_targets_require_latest_observation": true,
            "explicit_navigation_is_origin_scoped": true,
            "browser_network_is_domain_scoped": true,
            "sessions_are_workspace_local": true,
            "evidence_is_session_scoped": true,
            "one_workspace_mutation_at_a_time": true,
            "human_repair_acceptance_is_default": true,
            "automatic_repair_resolution_is_session_scoped": true,
            "automatic_repair_resolution_requires_all_gates": true
        },
        "repair_resolution": {
            "default": "human_review",
            "session_option": "--auto-resolve-repairs",
            "verified_transition_order": ["review_ready", "resolved"]
        },
        "action_ownership": {
            "interactive": "actions listed in action_schema",
            "deterministic_runner": ["verify_contract"]
        },
        "action_schema": interactive_action_schema(),
    });
    if args.compact {
        println!("{}", serde_json::to_string(&schema)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&schema)?);
    }
    Ok(ExitCode::SUCCESS)
}
