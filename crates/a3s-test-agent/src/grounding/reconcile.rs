use a3s_test_core::{PageContextLocator, PageContextNode, PageContextSnapshot, Target};

use super::{
    GroundingCandidate, GroundingCandidateGeometry, GroundingCoordinateSpace,
    GroundingImageCandidate, GroundingPoint, GroundingSemanticMatch,
};

pub(super) enum CandidateResolution {
    Semantic(GroundingSemanticMatch),
    ImageBound(GroundingImageCandidate),
}

pub(super) fn reconcile_candidate(
    index: usize,
    candidate: &GroundingCandidate,
    coordinate_space: GroundingCoordinateSpace,
    screenshot_width: u32,
    screenshot_height: u32,
    snapshot: Option<&PageContextSnapshot>,
) -> CandidateResolution {
    let screenshot_point = screenshot_point(
        candidate.geometry,
        coordinate_space,
        screenshot_width,
        screenshot_height,
    );
    let viewport_point = snapshot.and_then(|snapshot| {
        map_to_viewport(
            screenshot_point,
            screenshot_width,
            screenshot_height,
            snapshot,
        )
    });
    let matches = snapshot
        .filter(|snapshot| !snapshot.truncated)
        .zip(viewport_point)
        .map(|(snapshot, point)| hit_test(snapshot, point))
        .unwrap_or_default();
    if matches.len() == 1 {
        let (node, target) = matches[0].clone();
        return CandidateResolution::Semantic(GroundingSemanticMatch {
            candidate_index: u32::try_from(index).unwrap_or(u32::MAX),
            node_id: node_identity(node),
            reference: node.r#ref.clone(),
            target,
            screenshot_point,
            viewport_point: viewport_point.expect("one hit requires a viewport point"),
            confidence: candidate.confidence,
            label: candidate.label.clone(),
        });
    }

    let mut semantic_node_ids = matches
        .iter()
        .map(|(node, _)| node_identity(node))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    semantic_node_ids.sort();
    semantic_node_ids.dedup();
    CandidateResolution::ImageBound(GroundingImageCandidate {
        candidate_index: u32::try_from(index).unwrap_or(u32::MAX),
        geometry: candidate.geometry,
        screenshot_point,
        viewport_point,
        confidence: candidate.confidence,
        label: candidate.label.clone(),
        semantic_node_ids,
    })
}

fn screenshot_point(
    geometry: GroundingCandidateGeometry,
    coordinate_space: GroundingCoordinateSpace,
    screenshot_width: u32,
    screenshot_height: u32,
) -> GroundingPoint {
    let (mut x, mut y) = match geometry {
        GroundingCandidateGeometry::Point { x, y } => (x, y),
        GroundingCandidateGeometry::Box {
            x,
            y,
            width,
            height,
        } => (x + width / 2.0, y + height / 2.0),
    };
    if coordinate_space == GroundingCoordinateSpace::Normalized {
        x *= f64::from(screenshot_width);
        y *= f64::from(screenshot_height);
    }
    GroundingPoint { x, y }
}

fn map_to_viewport(
    point: GroundingPoint,
    screenshot_width: u32,
    screenshot_height: u32,
    snapshot: &PageContextSnapshot,
) -> Option<GroundingPoint> {
    let page = snapshot.page.as_ref()?;
    let (origin_x, origin_y, width, height) = page.viewport.visual.as_ref().map_or(
        (0.0, 0.0, page.viewport.width, page.viewport.height),
        |visual| (visual.x, visual.y, visual.width, visual.height),
    );
    let mapped = GroundingPoint {
        x: origin_x + point.x * width / f64::from(screenshot_width),
        y: origin_y + point.y * height / f64::from(screenshot_height),
    };
    (mapped.x.is_finite() && mapped.y.is_finite()).then_some(mapped)
}

fn hit_test(
    snapshot: &PageContextSnapshot,
    point: GroundingPoint,
) -> Vec<(&PageContextNode, Target)> {
    snapshot
        .nodes
        .iter()
        .filter(|node| node.state.visible)
        .filter_map(|node| {
            let geometry = node.geometry.as_ref()?;
            if !geometry.visible_ratio.is_finite()
                || geometry.visible_ratio <= 0.0
                || geometry.occluded
            {
                return None;
            }
            let rect = &geometry.viewport;
            let valid_rect = rect.x.is_finite()
                && rect.y.is_finite()
                && rect.width.is_finite()
                && rect.height.is_finite()
                && rect.width > 0.0
                && rect.height > 0.0;
            if !valid_rect
                || point.x < rect.x
                || point.y < rect.y
                || point.x >= rect.x + rect.width
                || point.y >= rect.y + rect.height
            {
                return None;
            }
            preferred_target(node).map(|target| (node, target))
        })
        .collect()
}

fn node_identity(node: &PageContextNode) -> String {
    if node.id.is_empty() {
        node.r#ref.clone().unwrap_or_default()
    } else {
        node.id.clone()
    }
}

fn preferred_target(node: &PageContextNode) -> Option<Target> {
    if let Some(reference) = &node.r#ref {
        return Some(Target::Ref {
            value: reference.clone(),
        });
    }
    let preferred = ["test_id", "role", "label", "placeholder", "text", "css"];
    if let Some(target) = preferred.into_iter().find_map(|kind| {
        node.locators
            .iter()
            .find_map(|locator| target_from_locator(kind, locator))
    }) {
        return Some(target);
    }
    if let Some(value) = &node.test_id {
        return Some(Target::TestId {
            value: value.clone(),
        });
    }
    node.role.as_ref().map(|role| Target::Role {
        role: role.clone(),
        name: node.name.clone().unwrap_or_default(),
    })
}

fn target_from_locator(kind: &str, locator: &PageContextLocator) -> Option<Target> {
    match (kind, locator) {
        ("test_id", PageContextLocator::TestId { value }) => Some(Target::TestId {
            value: value.clone(),
        }),
        ("role", PageContextLocator::Role { role, name }) => Some(Target::Role {
            role: role.clone(),
            name: name.clone(),
        }),
        ("label", PageContextLocator::Label { value }) => Some(Target::Label {
            value: value.clone(),
        }),
        ("placeholder", PageContextLocator::Placeholder { value }) => Some(Target::Placeholder {
            value: value.clone(),
        }),
        ("text", PageContextLocator::Text { value, exact }) => Some(Target::Text {
            value: value.clone(),
            exact: *exact,
        }),
        ("css", PageContextLocator::Css { value }) => Some(Target::Css {
            selector: value.clone(),
        }),
        _ => None,
    }
}
