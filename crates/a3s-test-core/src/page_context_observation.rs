use crate::model::{PageContextObservation, PageContextSnapshot};

impl PageContextObservation {
    #[must_use]
    pub fn absent() -> Self {
        Self {
            present: false,
            protocol: None,
            sdk_version: None,
            revision: None,
            snapshot: None,
        }
    }

    #[must_use]
    pub fn from_snapshot(snapshot: PageContextSnapshot) -> Self {
        Self {
            present: true,
            protocol: snapshot.protocol.clone(),
            sdk_version: snapshot.sdk_version.clone(),
            revision: snapshot.revision,
            snapshot: Some(snapshot),
        }
    }
}
