//! Protocol 17 admission and pixel comparison.
//!
//! Size is decided before masks. Caps use the smaller limit. A missing
//! baseline stays missing unless update is set.

use a3s_test_core::{
    compare_pixels, decode_png_bounded, pixel_baseline_decision, verified_session_candidate,
    Action, DecodedPng, Expectation, PixelBaselineDecision, PixelCompareFailure, PixelDecodeError,
    PixelExpectation, PixelLimits, PixelMask, RecordedStep, SecretPurpose, Target, TestSuite,
    MAX_PIXEL_RATIO_MICROS, MAX_PIXEL_THRESHOLD_MILLIS,
};

fn image(width: u32, height: u32, rgba: &[u8]) -> DecodedPng {
    DecodedPng {
        width,
        height,
        rgba: rgba.to_vec(),
    }
}

fn solid(width: u32, height: u32, pixel: [u8; 4]) -> DecodedPng {
    let mut rgba = Vec::new();
    for _ in 0..(width * height) {
        rgba.extend_from_slice(&pixel);
    }
    image(width, height, &rgba)
}

fn limits(
    max_diff_pixels: Option<u32>,
    max_diff_pixel_ratio_micros: Option<u32>,
    masks: Vec<PixelMask>,
) -> PixelLimits {
    PixelLimits {
        threshold_millis: 0,
        max_diff_pixels,
        max_diff_pixel_ratio_micros,
        masks,
    }
}

#[test]
fn pixel_size_mismatch_is_reported_before_masks_or_caps() {
    let actual = solid(1, 1, [255, 0, 0, 255]);
    let baseline = solid(2, 1, [0, 0, 255, 255]);
    let error = compare_pixels(
        &actual,
        &baseline,
        &limits(
            Some(100),
            Some(1_000_000),
            vec![PixelMask {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            }],
        ),
    )
    .expect_err("size");
    assert_eq!(
        error,
        PixelCompareFailure::Size {
            actual_width: 1,
            actual_height: 1,
            baseline_width: 2,
            baseline_height: 1,
        }
    );
}

#[test]
fn pixel_caps_use_zero_when_unset_and_the_smaller_limit_when_both_are_set() {
    let baseline = solid(2, 2, [255, 0, 0, 255]);
    let mut actual = baseline.clone();
    actual.rgba[..4].copy_from_slice(&[0, 0, 255, 255]);
    let one =
        compare_pixels(&actual, &baseline, &limits(None, None, Vec::new())).expect_err("zero");
    assert_eq!(
        one,
        PixelCompareFailure::Diff {
            different_pixels: 1,
            allowed_pixels: 0,
        }
    );
    assert_eq!(
        compare_pixels(&actual, &baseline, &limits(Some(1), None, Vec::new())).expect("pixel cap"),
        1
    );

    actual.rgba[4..8].copy_from_slice(&[0, 0, 255, 255]);
    let smaller = compare_pixels(
        &actual,
        &baseline,
        &limits(Some(10), Some(250_000), Vec::new()),
    )
    .expect_err("ratio cap");
    assert_eq!(
        smaller,
        PixelCompareFailure::Diff {
            different_pixels: 2,
            allowed_pixels: 1,
        }
    );
    assert_eq!(
        compare_pixels(&actual, &baseline, &limits(Some(2), None, Vec::new())).expect("pixel cap"),
        2
    );
}

#[test]
fn pixel_masks_ignore_covered_differences() {
    let baseline = solid(2, 2, [255, 0, 0, 255]);
    let mut actual = baseline.clone();
    actual.rgba[..8].copy_from_slice(&[0, 0, 255, 255, 0, 255, 0, 255]);
    let masked = compare_pixels(
        &actual,
        &baseline,
        &limits(
            None,
            None,
            vec![PixelMask {
                x: 0,
                y: 0,
                width: 2,
                height: 1,
            }],
        ),
    )
    .expect("masked");
    assert_eq!(masked, 0);
}

#[test]
fn pixel_baseline_policy_writes_only_when_update_is_set() {
    assert_eq!(
        pixel_baseline_decision(true, false, true),
        PixelBaselineDecision::Update
    );
    assert_eq!(
        pixel_baseline_decision(false, false, true),
        PixelBaselineDecision::Missing
    );
    assert_eq!(
        pixel_baseline_decision(false, false, false),
        PixelBaselineDecision::Missing
    );
    assert_eq!(
        pixel_baseline_decision(false, true, true),
        PixelBaselineDecision::Compare
    );
}

#[test]
fn bounded_png_decode_rejects_illegal_images_before_a_full_decode() {
    assert_eq!(
        decode_png_bounded(b"not a png").expect_err("signature"),
        PixelDecodeError::Invalid
    );
    assert_eq!(
        decode_png_bounded(&ihdr(0, 1)).expect_err("zero"),
        PixelDecodeError::TooLarge
    );
    assert_eq!(
        decode_png_bounded(&ihdr(4_097, 1)).expect_err("dimension"),
        PixelDecodeError::TooLarge
    );
    let mut huge = vec![0_u8; 32 * 1_024 * 1_024 + 1];
    huge[..8].copy_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
    assert_eq!(
        decode_png_bounded(&huge).expect_err("byte cap"),
        PixelDecodeError::TooLarge
    );

    let encoded = encode_png(1, 1, &[10, 20, 30, 255]);
    let decoded = decode_png_bounded(&encoded).expect("roundtrip");
    assert_eq!(
        decoded,
        DecodedPng {
            width: 1,
            height: 1,
            rgba: vec![10, 20, 30, 255],
        }
    );
}

fn ihdr(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
    bytes.extend_from_slice(&13_u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut encoder = png::Encoder::new(&mut output, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgba).expect("png data");
    writer.finish().expect("png finish");
    output
}

#[test]
fn viewport_and_pixel_acl_fail_closed() {
    let suite = TestSuite::from_acl(
        r#"
suite "pixels" {
    version = 1
    target "desktop" {
        width = 1280
        height = 720
    }
    target "mobile" {
        width = 390
        height = 844
        scale = 2
    }
    scenario "home" {
        name = "Home"
        surface = "web"
        timeout_ms = 1000
        expect "shot" {
            pixels = "baseline.png"
            threshold_millis = 200
            max_diff_pixels = 4
            max_diff_pixel_ratio_micros = 250000
            masks = ["0,0,2,1"]
            update = false
        }
    }
}
"#,
    )
    .expect("suite");
    assert_eq!(suite.targets.len(), 2);
    assert_eq!(suite.targets[0].name, "desktop");
    assert_eq!(suite.targets[1].scale, Some(2));
    match &suite.scenarios[0].steps[0].action {
        Action::Assert {
            expectation: Expectation::Pixels(pixels),
        } => {
            assert_eq!(
                pixels,
                &PixelExpectation {
                    baseline: "baseline.png".to_string(),
                    threshold_millis: 200,
                    max_diff_pixels: Some(4),
                    max_diff_pixel_ratio_micros: Some(250_000),
                    masks: vec![PixelMask {
                        x: 0,
                        y: 0,
                        width: 2,
                        height: 1,
                    }],
                    update: false,
                }
            );
        }
        other => panic!("expected pixels, got {other:?}"),
    }

    let rejected = [
        (
            r#"
suite "wide" {
    version = 1
    target "desktop" { width = 4097 height = 10 }
    scenario "home" { name = "Home" surface = "web" timeout_ms = 1000 snapshot "page" { interactive = true } }
}
"#,
            "test.spec.viewport_size",
        ),
        (
            r#"
suite "dots" {
    version = 1
    target ".." { width = 10 height = 10 }
    scenario "home" { name = "Home" surface = "web" timeout_ms = 1000 snapshot "page" { interactive = true } }
}
"#,
            "test.spec.viewport_name",
        ),
        (
            r#"
suite "same" {
    version = 1
    target "desktop" { width = 10 height = 10 }
    target "desktop" { width = 20 height = 20 }
    scenario "home" { name = "Home" surface = "web" timeout_ms = 1000 snapshot "page" { interactive = true } }
}
"#,
            "test.spec.viewport_duplicate",
        ),
        (
            r#"
suite "threshold" {
    version = 1
    scenario "home" {
        name = "Home"
        surface = "web"
        timeout_ms = 1000
        expect "shot" { pixels = "baseline.png" threshold_millis = 1001 }
    }
}
"#,
            "test.spec.pixel_threshold",
        ),
        (
            r#"
suite "ratio" {
    version = 1
    scenario "home" {
        name = "Home"
        surface = "web"
        timeout_ms = 1000
        expect "shot" { pixels = "baseline.png" max_diff_pixel_ratio_micros = 1000001 }
    }
}
"#,
            "test.spec.pixel_ratio",
        ),
        (
            r#"
suite "mask" {
    version = 1
    scenario "home" {
        name = "Home"
        surface = "web"
        timeout_ms = 1000
        expect "shot" { pixels = "baseline.png" masks = ["1,2,3"] }
    }
}
"#,
            "test.spec.pixel_mask",
        ),
        (
            r#"
suite "mixed" {
    version = 1
    scenario "home" {
        name = "Home"
        surface = "web"
        timeout_ms = 1000
        expect "title" { text = "Ready" update = true }
    }
}
"#,
            "test.spec.attribute_unexpected",
        ),
    ];
    const _: () = assert!(MAX_PIXEL_THRESHOLD_MILLIS < 1001);
    const _: () = assert!(MAX_PIXEL_RATIO_MICROS < 1_000_001);
    for (source, code) in rejected {
        let error = TestSuite::from_acl(source).expect_err(code);
        assert_eq!(error.code(), code, "{}", error.message());
    }
}

#[test]
fn secret_acl_stores_the_name_and_not_a_literal() {
    let suite = TestSuite::from_acl(
        r##"
suite "secret" {
    version = 1
    scenario "login" {
        name = "Login"
        surface = "web"
        timeout_ms = 1000
        fill_secret "password" {
            target = css("#password")
            name = "APP_PASSWORD"
            purpose = "password"
        }
    }
}
"##,
    )
    .expect("secret suite");
    match &suite.scenarios[0].steps[0].action {
        Action::FillSecret {
            target: Target::Css { selector },
            name,
            purpose,
        } => {
            assert_eq!(selector, "#password");
            assert_eq!(name, "APP_PASSWORD");
            assert_eq!(*purpose, SecretPurpose::Password);
        }
        other => panic!("expected fill_secret, got {other:?}"),
    }
    let json = serde_json::to_string(&suite).expect("json");
    assert!(!json.contains("s3cret"));
    assert!(json.contains("APP_PASSWORD"));
}

fn json_strings_contain(value: &serde_json::Value, secret: &str) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains(secret),
        serde_json::Value::Array(items) => {
            items.iter().any(|item| json_strings_contain(item, secret))
        }
        serde_json::Value::Object(fields) => fields
            .values()
            .any(|item| json_strings_contain(item, secret)),
        _ => false,
    }
}

#[test]
fn verified_session_candidate_keeps_only_steps_a_later_assertion_confirmed() {
    let steps = vec![
        RecordedStep {
            index: 0,
            action: Action::Navigate {
                url: "https://example.test/login".to_string(),
            },
            passed: true,
        },
        RecordedStep {
            index: 1,
            action: Action::Fill {
                target: Target::Css {
                    selector: "#password".to_string(),
                },
                value: "s3cret-literal".to_string(),
            },
            passed: true,
        },
        RecordedStep {
            index: 2,
            action: Action::Click {
                target: Target::Ref {
                    value: "@e9".to_string(),
                },
            },
            passed: true,
        },
        RecordedStep {
            index: 3,
            action: Action::Click {
                target: Target::VisualPoint {
                    snapshot: "shot".to_string(),
                    x: 1,
                    y: 2,
                },
            },
            passed: true,
        },
        RecordedStep {
            index: 4,
            action: Action::Click {
                target: Target::Css {
                    selector: "#failed".to_string(),
                },
            },
            passed: false,
        },
        RecordedStep {
            index: 5,
            action: Action::Assert {
                expectation: Expectation::TextVisible("Welcome".to_string()),
            },
            passed: true,
        },
        RecordedStep {
            index: 6,
            action: Action::Click {
                target: Target::Css {
                    selector: "#after".to_string(),
                },
            },
            passed: true,
        },
    ];
    let acl = verified_session_candidate(&steps, &["s3cret-literal"]).expect("candidate");
    assert!(acl.contains("https://example.test/login"));
    assert!(acl.contains("Welcome"));
    assert!(!acl.contains("s3cret-literal"));
    assert!(!acl.contains("@e9"));
    assert!(!acl.contains("visual"));
    assert!(!acl.contains("#failed"));
    assert!(!acl.contains("#after"));
    assert!(TestSuite::from_acl(&acl).is_ok());
    let quoted = "p\"ass";
    let quoted_steps = vec![
        RecordedStep {
            index: 0,
            action: Action::Navigate {
                url: "https://example.test/login".to_string(),
            },
            passed: true,
        },
        RecordedStep {
            index: 1,
            action: Action::Fill {
                target: Target::Css {
                    selector: "#echo".to_string(),
                },
                value: quoted.to_string(),
            },
            passed: true,
        },
        RecordedStep {
            index: 2,
            action: Action::Assert {
                expectation: Expectation::TextVisible("Welcome".to_string()),
            },
            passed: true,
        },
    ];
    let quoted_acl =
        verified_session_candidate(&quoted_steps, &[quoted]).expect("quoted candidate");
    let quoted_suite = TestSuite::from_acl(&quoted_acl).expect("quoted acl");
    assert!(
        !json_strings_contain(
            &serde_json::to_value(&quoted_suite).expect("suite json"),
            quoted
        ),
        "{quoted_acl}"
    );
    assert!(quoted_acl.contains("Welcome"));
    assert!(!quoted_acl.contains("#echo"));
    assert!(!quoted_acl.contains("p\\\"ass"));

    let prose = vec![RecordedStep {
        index: 0,
        action: Action::Navigate {
            url: "https://example.test/".to_string(),
        },
        passed: true,
    }];
    assert!(verified_session_candidate(&prose, &[]).is_none());
}
