use std::collections::{HashMap, HashSet};

use super::{
    ContractProvenanceStatus, ContractSeverity, ContractVariant, SurfaceContract,
    SurfaceContractDraft,
};
use crate::SpecError;

impl SurfaceContractDraft {
    pub fn admit(self) -> Result<SurfaceContract, SpecError> {
        let path = format!("surface_contract.{}", self.name);
        if self.version != 1 {
            return Err(SpecError::new(
                "test.contract.version_unsupported",
                format!("{path}.version"),
                "only surface contract version 1 is supported",
            ));
        }
        if self.provenance.is_empty() {
            return Err(SpecError::new(
                "test.contract.provenance_required",
                format!("{path}.provenance"),
                "a surface contract requires at least one provenance entry",
            ));
        }
        for entry in &self.provenance {
            let digest = entry.digest.strip_prefix("sha256:").ok_or_else(|| {
                SpecError::new(
                    "test.contract.provenance_digest_invalid",
                    format!("{path}.provenance.{}.digest", entry.id),
                    "provenance digest must use sha256:<64 lowercase hexadecimal characters>",
                )
            })?;
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(SpecError::new(
                    "test.contract.provenance_digest_invalid",
                    format!("{path}.provenance.{}.digest", entry.id),
                    "provenance digest must use sha256:<64 lowercase hexadecimal characters>",
                ));
            }
        }
        if self.variants.is_empty() {
            return Err(SpecError::new(
                "test.contract.variant_required",
                format!("{path}.variant"),
                "a surface contract requires at least one variant",
            ));
        }
        let has_blocking = self.variants.iter().any(|variant| {
            variant
                .elements
                .iter()
                .any(|element| element.severity == ContractSeverity::Blocking)
        });
        let has_authoritative_review = self.provenance.iter().any(|entry| {
            entry.status == ContractProvenanceStatus::Reviewed && entry.confidence == 100
        });
        if has_blocking && !has_authoritative_review {
            return Err(SpecError::new(
                "test.contract.provenance_unreviewed",
                format!("{path}.provenance"),
                "blocking contract checks require at least one reviewed provenance entry with 100 confidence",
            ));
        }
        for variant in &self.variants {
            admit_variant(variant, &path)?;
        }
        Ok(SurfaceContract {
            name: self.name,
            version: self.version,
            context: self.context,
            provenance: self.provenance,
            variants: self.variants,
        })
    }
}

fn admit_variant(variant: &ContractVariant, parent: &str) -> Result<(), SpecError> {
    let path = format!("{parent}.variant.{}", variant.id);
    if variant.elements.is_empty() {
        return Err(SpecError::new(
            "test.contract.element_required",
            &path,
            "a contract variant requires at least one element",
        ));
    }
    if variant
        .min_width
        .zip(variant.max_width)
        .is_some_and(|(min, max)| min > max)
    {
        return Err(SpecError::new(
            "test.contract.viewport_range_invalid",
            &path,
            "variant min_width must not exceed max_width",
        ));
    }
    let by_id = variant
        .elements
        .iter()
        .map(|element| (element.id.as_str(), element))
        .collect::<HashMap<_, _>>();
    for element in &variant.elements {
        let element_path = format!("{path}.element.{}", element.id);
        if element.test_id.is_none() && element.role.is_none() && element.component_id.is_none() {
            return Err(SpecError::new(
                "test.contract.element_identity_required",
                &element_path,
                "an element requires test_id, component_id, or role identity",
            ));
        }
        if let Some(parent_id) = &element.parent {
            if parent_id == &element.id || !by_id.contains_key(parent_id.as_str()) {
                return Err(SpecError::new(
                    "test.contract.element_reference_unknown",
                    format!("{element_path}.parent"),
                    "element parent must reference another element in the same variant",
                ));
            }
        }
    }
    for element in &variant.elements {
        let mut seen = HashSet::new();
        let mut current = Some(element.id.as_str());
        while let Some(id) = current {
            if !seen.insert(id) {
                return Err(SpecError::new(
                    "test.contract.element_reference_cycle",
                    format!("{path}.element.{}.parent", element.id),
                    "element parent relationships must not contain cycles",
                ));
            }
            current = by_id.get(id).and_then(|value| value.parent.as_deref());
        }
    }
    Ok(())
}
