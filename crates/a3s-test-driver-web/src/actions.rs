use std::ffi::OsString;

use a3s_test_core::{
    DialogOperation, DriverError, FrameTarget, ModifierKey, NetworkRoute, TabOperation, Target,
};

use crate::protocol::direct_selector;

pub(crate) fn tab_args(operation: &TabOperation) -> Vec<OsString> {
    match operation {
        TabOperation::List => vec!["tab".into(), "list".into()],
        TabOperation::New { url, label } => {
            let mut args = vec![OsString::from("tab"), OsString::from("new")];
            if let Some(label) = label {
                args.extend([OsString::from("--label"), OsString::from(label)]);
            }
            if let Some(url) = url {
                args.push(OsString::from(url));
            }
            args
        }
        TabOperation::Switch { tab } => vec!["tab".into(), tab.into()],
        TabOperation::Close { tab } => {
            let mut args = vec![OsString::from("tab"), OsString::from("close")];
            if let Some(tab) = tab {
                args.push(OsString::from(tab));
            }
            args
        }
    }
}

pub(crate) fn frame_args(target: &FrameTarget) -> Vec<OsString> {
    vec![
        OsString::from("frame"),
        match target {
            FrameTarget::Main => OsString::from("main"),
            FrameTarget::Selector(selector) => OsString::from(selector),
        },
    ]
}

pub(crate) fn dialog_args(operation: &DialogOperation) -> Vec<OsString> {
    match operation {
        DialogOperation::Status => vec!["dialog".into(), "status".into()],
        DialogOperation::Accept { text } => {
            let mut args = vec![OsString::from("dialog"), OsString::from("accept")];
            if let Some(text) = text {
                args.push(OsString::from(text));
            }
            args
        }
        DialogOperation::Dismiss => vec!["dialog".into(), "dismiss".into()],
    }
}

pub(crate) fn upload_args(target: &Target, paths: &[String]) -> Result<Vec<OsString>, DriverError> {
    let mut args = vec![
        OsString::from("upload"),
        OsString::from(direct_selector(target)?),
    ];
    args.extend(paths.iter().map(OsString::from));
    Ok(args)
}

pub(crate) fn select_args(
    target: &Target,
    values: &[String],
) -> Result<Vec<OsString>, DriverError> {
    if values.is_empty() {
        return Err(DriverError::new(
            "test.driver.web.select_values_required",
            "select requires at least one value",
        ));
    }
    let mut args = vec![
        OsString::from("select"),
        OsString::from(direct_selector(target)?),
    ];
    args.extend(values.iter().map(OsString::from));
    Ok(args)
}

pub(crate) fn drag_args(source: &Target, target: &Target) -> Result<Vec<OsString>, DriverError> {
    Ok(vec![
        OsString::from("drag"),
        OsString::from(direct_selector(source)?),
        OsString::from(direct_selector(target)?),
    ])
}

pub(crate) fn viewport_args(width: u32, height: u32, scale: Option<u32>) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("set"),
        OsString::from("viewport"),
        OsString::from(width.to_string()),
        OsString::from(height.to_string()),
    ];
    if let Some(scale) = scale {
        args.push(OsString::from(scale.to_string()));
    }
    args
}

pub(crate) const fn modifier_name(modifier: ModifierKey) -> &'static str {
    match modifier {
        ModifierKey::Alt => "Alt",
        ModifierKey::Control => "Control",
        ModifierKey::Meta => "Meta",
        ModifierKey::Shift => "Shift",
    }
}

pub(crate) fn network_route_args(pattern: &str, route: &NetworkRoute) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("network"),
        OsString::from("route"),
        OsString::from(pattern),
    ];
    match route {
        NetworkRoute::Abort => args.push(OsString::from("--abort")),
        NetworkRoute::Body(body) => {
            args.extend([OsString::from("--body"), OsString::from(body)]);
        }
    }
    args
}

pub(crate) fn network_unroute_args(pattern: Option<&str>) -> Vec<OsString> {
    let mut args = vec![OsString::from("network"), OsString::from("unroute")];
    if let Some(pattern) = pattern {
        args.push(OsString::from(pattern));
    }
    args
}
