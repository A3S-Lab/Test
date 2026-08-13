use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::merge::{detect_conflicts, merge_decisions};
use super::validation::{
    read_verified_sources, validate_context, validate_identifier, validate_options,
    validate_response, validate_sources,
};
use super::{
    ContractConflictStatus, ContractGenerationOptions, ContractGenerationProviderResponse,
    ContractGenerationReview, ContractSource, GeneratedContractDraft, ReviewedContractDraft,
};
use crate::ContractGenerationError;

pub const CONTRACT_WORKFLOW_PROTOCOL: &str = "a3s.test.contract-workflow/1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractWorkflowStage {
    Generated,
    Reviewed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractWorkflowAdmission {
    pub sources: Vec<ContractSource>,
    pub max_cost_microusd: u64,
    pub timeout_ms: u64,
    pub max_sources: usize,
    pub max_source_bytes: usize,
    pub max_candidates: usize,
    pub max_elements: usize,
    pub max_string_bytes: usize,
}

impl ContractWorkflowAdmission {
    pub fn new(
        sources: Vec<ContractSource>,
        max_cost_microusd: u64,
        options: &ContractGenerationOptions,
    ) -> Result<Self, ContractGenerationError> {
        validate_options(options)?;
        validate_sources(&sources, options)?;
        let timeout_ms = u64::try_from(options.timeout.as_millis()).map_err(|_| {
            workflow_error("contract workflow timeout cannot be represented in milliseconds")
        })?;
        Ok(Self {
            sources,
            max_cost_microusd,
            timeout_ms,
            max_sources: options.max_sources,
            max_source_bytes: options.max_source_bytes,
            max_candidates: options.max_candidates,
            max_elements: options.max_elements,
            max_string_bytes: options.max_string_bytes,
        })
    }

    fn options(&self) -> Result<ContractGenerationOptions, ContractGenerationError> {
        let options = ContractGenerationOptions {
            timeout: std::time::Duration::from_millis(self.timeout_ms),
            max_sources: self.max_sources,
            max_source_bytes: self.max_source_bytes,
            max_candidates: self.max_candidates,
            max_elements: self.max_elements,
            max_string_bytes: self.max_string_bytes,
        };
        validate_options(&options)?;
        validate_sources(&self.sources, &options)?;
        Ok(options)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractWorkflowArtifact {
    pub protocol: String,
    pub stage: ContractWorkflowStage,
    pub integrity_sha256: String,
    pub admission: ContractWorkflowAdmission,
    pub generated: GeneratedContractDraft,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ContractGenerationReview>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_acl: Option<String>,
}

impl ContractWorkflowArtifact {
    pub fn generated(
        draft: GeneratedContractDraft,
        admission: ContractWorkflowAdmission,
    ) -> Result<Self, ContractGenerationError> {
        let mut artifact = Self {
            protocol: CONTRACT_WORKFLOW_PROTOCOL.to_string(),
            stage: ContractWorkflowStage::Generated,
            integrity_sha256: String::new(),
            admission,
            generated: draft,
            review: None,
            contract_acl: None,
        };
        artifact.integrity_sha256 = artifact.calculate_integrity()?;
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn reviewed(
        reviewed: ReviewedContractDraft,
        admission: ContractWorkflowAdmission,
    ) -> Result<Self, ContractGenerationError> {
        let mut artifact = Self {
            protocol: CONTRACT_WORKFLOW_PROTOCOL.to_string(),
            stage: ContractWorkflowStage::Reviewed,
            integrity_sha256: String::new(),
            admission,
            contract_acl: Some(reviewed.contract.to_acl()),
            generated: reviewed.generated,
            review: Some(reviewed.review),
        };
        artifact.integrity_sha256 = artifact.calculate_integrity()?;
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), ContractGenerationError> {
        if self.protocol != CONTRACT_WORKFLOW_PROTOCOL {
            return Err(workflow_error("contract workflow protocol is unsupported"));
        }
        if self.integrity_sha256 != self.calculate_integrity()? {
            return Err(workflow_error(
                "contract workflow integrity digest does not match its payload",
            ));
        }
        let options = self.admission.options()?;
        validate_identifier(&self.generated.name, "contract name")?;
        validate_context(&self.generated.context, options.max_string_bytes)?;
        if self.generated.version != 1 {
            return Err(workflow_error(
                "contract workflow generated draft version is unsupported",
            ));
        }
        match self.stage {
            ContractWorkflowStage::Generated
                if self.review.is_none() && self.contract_acl.is_none() => {}
            ContractWorkflowStage::Reviewed
                if self.review.is_some() && self.contract_acl.is_some() => {}
            _ => {
                return Err(workflow_error(
                    "contract workflow stage and payload are inconsistent",
                ));
            }
        }
        self.validate_derived_fields()?;
        if let Some(contract_acl) = &self.contract_acl {
            self.validate_reviewed_acl(contract_acl)?;
        }
        Ok(())
    }

    pub async fn validate_evidence(&self) -> Result<(), ContractGenerationError> {
        self.validate()?;
        let options = self.admission.options()?;
        let verified =
            read_verified_sources(&self.admission.sources, options.max_source_bytes).await?;
        let response = ContractGenerationProviderResponse {
            identity: self.generated.provider.clone(),
            source_digests: self.generated.provenance.clone(),
            candidates: self.generated.candidates.clone(),
            usage: self.generated.usage,
            request_id: self.generated.request_id.clone(),
        };
        validate_response(
            &self.generated.context,
            &self.admission.sources,
            &verified,
            self.admission.max_cost_microusd,
            &response,
            &self.generated.provider,
            &options,
        )?;
        Ok(())
    }

    pub fn apply_review(
        self,
        review: ContractGenerationReview,
    ) -> Result<Self, ContractGenerationError> {
        self.validate()?;
        if self.stage != ContractWorkflowStage::Generated {
            return Err(workflow_error(
                "only a generated contract workflow can be reviewed",
            ));
        }
        let reviewed =
            super::review::review(self.generated, review, self.admission.max_string_bytes)?;
        Self::reviewed(reviewed, self.admission)
    }

    fn validate_reviewed_acl(&self, contract_acl: &str) -> Result<(), ContractGenerationError> {
        let contract =
            a3s_test_core::SurfaceContractDraft::from_acl(contract_acl).map_err(|error| {
                workflow_error(format!("contract workflow ACL is invalid: {error}"))
            })?;
        if contract.name() != self.generated.name
            || contract.context() != &self.generated.context
            || contract.to_acl() != contract_acl
        {
            return Err(workflow_error(
                "contract workflow ACL does not match its generated draft",
            ));
        }
        let expected_review = self
            .review
            .clone()
            .ok_or_else(|| workflow_error("reviewed contract workflow has no review"))?;
        let regenerated = super::review::review(
            self.generated.clone(),
            expected_review,
            self.admission.max_string_bytes,
        )
        .map_err(|error| {
            workflow_error(format!(
                "contract workflow review cannot be reproduced: {}",
                error.message()
            ))
        })?;
        if regenerated.contract.to_acl() != contract_acl
            || regenerated.generated != self.generated
            || self.review.as_ref() != Some(&regenerated.review)
        {
            return Err(workflow_error(
                "contract workflow ACL is not the deterministic result of its review",
            ));
        }
        Ok(())
    }

    fn validate_derived_fields(&self) -> Result<(), ContractGenerationError> {
        let expected_conflicts = detect_conflicts(&self.generated.candidates);
        if expected_conflicts.len() != self.generated.conflicts.len()
            || expected_conflicts
                .iter()
                .zip(&self.generated.conflicts)
                .any(|(expected, actual)| {
                    expected.id != actual.id
                        || expected.variant_id != actual.variant_id
                        || expected.element_id != actual.element_id
                        || expected.field != actual.field
                        || expected.candidate_ids != actual.candidate_ids
                        || expected.values != actual.values
                })
        {
            return Err(workflow_error(
                "contract workflow conflict set does not match its candidates",
            ));
        }
        if self.stage == ContractWorkflowStage::Generated
            && self.generated.conflicts.iter().any(|conflict| {
                conflict.status != ContractConflictStatus::Unresolved
                    || conflict.resolution.is_some()
            })
        {
            return Err(workflow_error(
                "generated contract workflow contains pre-resolved conflicts",
            ));
        }
        if merge_decisions(&self.generated.candidates)? != self.generated.unresolved_decisions {
            return Err(workflow_error(
                "contract workflow decision set does not match its candidates",
            ));
        }
        Ok(())
    }

    fn calculate_integrity(&self) -> Result<String, ContractGenerationError> {
        #[derive(Serialize)]
        struct IntegrityPayload<'a> {
            protocol: &'a str,
            stage: ContractWorkflowStage,
            admission: &'a ContractWorkflowAdmission,
            generated: &'a GeneratedContractDraft,
            review: &'a Option<ContractGenerationReview>,
            contract_acl: &'a Option<String>,
        }

        let payload = IntegrityPayload {
            protocol: &self.protocol,
            stage: self.stage,
            admission: &self.admission,
            generated: &self.generated,
            review: &self.review,
            contract_acl: &self.contract_acl,
        };
        let bytes = serde_json::to_vec(&payload).map_err(|error| {
            workflow_error(format!(
                "contract workflow integrity payload cannot be encoded: {error}"
            ))
        })?;
        Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
    }
}

fn workflow_error(message: impl Into<String>) -> ContractGenerationError {
    ContractGenerationError::new(
        "test.agent.contract_generation.workflow_invalid",
        message,
        false,
    )
}
