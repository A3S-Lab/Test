use serde::{Deserialize, Serialize};

use crate::{ContractReport, PageContextTheme, SpecError, SurfaceObservation};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractMode {
    Persuade,
    Operate,
    Read,
    Experience,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContractContext {
    pub mode: ContractMode,
    pub audience: Vec<String>,
    pub primary_outcome: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractProvenanceKind {
    Prd,
    Design,
    Manual,
    OfficialDocs,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractProvenanceStatus {
    Draft,
    Reviewed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdmittedProvenance {
    pub id: String,
    pub kind: ContractProvenanceKind,
    pub uri: String,
    pub digest: String,
    pub status: ContractProvenanceStatus,
    pub confidence: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractSeverity {
    Blocking,
    Important,
    Suggestion,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContractCitation {
    pub id: String,
    pub provenance_id: String,
    pub quote: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContractElement {
    pub id: String,
    pub test_id: Option<String>,
    pub component_id: Option<String>,
    pub role: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub required: bool,
    pub visible: Option<bool>,
    pub enabled: Option<bool>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    pub readonly: Option<bool>,
    pub form_required: Option<bool>,
    pub invalid: Option<bool>,
    pub parent: Option<String>,
    pub severity: ContractSeverity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<ContractCitation>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContractVariant {
    pub id: String,
    pub state: String,
    pub min_width: Option<u32>,
    pub max_width: Option<u32>,
    pub theme: Option<PageContextTheme>,
    pub language: Option<String>,
    pub elements: Vec<ContractElement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceContractDraft {
    pub(crate) name: String,
    pub(crate) version: u32,
    pub(crate) context: ContractContext,
    pub(crate) provenance: Vec<AdmittedProvenance>,
    pub(crate) variants: Vec<ContractVariant>,
}

impl SurfaceContractDraft {
    pub fn new(
        name: impl Into<String>,
        version: u32,
        context: ContractContext,
        provenance: Vec<AdmittedProvenance>,
        variants: Vec<ContractVariant>,
    ) -> Result<Self, SpecError> {
        let draft = Self {
            name: name.into(),
            version,
            context,
            provenance,
            variants,
        };
        super::parser::validate_draft_structure(&draft)?;
        Ok(draft)
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn provenance(&self) -> &[AdmittedProvenance] {
        &self.provenance
    }

    #[must_use]
    pub fn variants(&self) -> &[ContractVariant] {
        &self.variants
    }

    #[must_use]
    pub fn context(&self) -> &ContractContext {
        &self.context
    }

    #[must_use]
    pub fn to_acl(&self) -> String {
        super::parser::generate_contract(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceContract {
    pub name: String,
    pub version: u32,
    pub context: ContractContext,
    pub provenance: Vec<AdmittedProvenance>,
    pub variants: Vec<ContractVariant>,
}

impl SurfaceContract {
    #[must_use]
    pub fn variant(&self, id: &str) -> Option<&ContractVariant> {
        self.variants.iter().find(|variant| variant.id == id)
    }

    pub fn reconcile(
        &self,
        variant: &str,
        state: &str,
        observation: &SurfaceObservation,
    ) -> Result<ContractReport, SpecError> {
        crate::reconcile::reconcile(self, variant, state, observation)
    }
}
