use a3s_acl::Block;
use a3s_test_agent::{
    ContractConflictResolution, ContractGenerationReview, ContractReviewAction,
    ContractReviewDecision,
};
use anyhow::{Context, Result};

use super::config::{ensure_attributes, no_nested_blocks, one_label, required_string};

pub(super) fn parse_review(source: &str) -> Result<ContractGenerationReview> {
    let document = a3s_acl::parse(source).context("invalid contract review ACL")?;
    if document.blocks.len() != 1 || document.blocks[0].name != "contract_review" {
        anyhow::bail!("contract review must contain exactly one contract_review block");
    }
    let root = &document.blocks[0];
    if !root.labels.is_empty() {
        anyhow::bail!("contract_review does not accept labels");
    }
    ensure_attributes(root, &["reviewer"], "contract_review")?;
    let reviewer = required_string(root, "reviewer", "contract_review")?.to_string();
    let mut decisions = Vec::new();
    let mut conflict_resolutions = Vec::new();
    for block in &root.blocks {
        match block.name.as_str() {
            "candidate" => decisions.push(parse_candidate(block)?),
            "conflict" => conflict_resolutions.push(parse_conflict(block)?),
            name => anyhow::bail!("unsupported contract_review block '{name}'"),
        }
    }
    Ok(ContractGenerationReview {
        reviewer,
        decisions,
        conflict_resolutions,
    })
}

fn parse_candidate(block: &Block) -> Result<ContractReviewDecision> {
    let path = "contract_review.candidate";
    let candidate_id = one_label(block, path)?.to_string();
    no_nested_blocks(block, path)?;
    ensure_attributes(block, &["action"], path)?;
    let action = match required_string(block, "action", path)? {
        "approve" => ContractReviewAction::Approve,
        "reject" => ContractReviewAction::Reject,
        value => anyhow::bail!("unsupported candidate review action '{value}'"),
    };
    Ok(ContractReviewDecision {
        candidate_id,
        action,
    })
}

fn parse_conflict(block: &Block) -> Result<ContractConflictResolution> {
    let path = "contract_review.conflict";
    let conflict_id = one_label(block, path)?.to_string();
    no_nested_blocks(block, path)?;
    ensure_attributes(block, &["select", "rationale"], path)?;
    Ok(ContractConflictResolution {
        conflict_id,
        selected_candidate_id: required_string(block, "select", path)?.to_string(),
        rationale: required_string(block, "rationale", path)?.to_string(),
    })
}
