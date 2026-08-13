use a3s_test_core::Action;
use serde_json::Value;

pub(crate) fn interactive_action_schema() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(Action))
        .unwrap_or_else(|_| serde_json::json!({ "type": "object" }));
    if let Some(variants) = schema.get_mut("oneOf").and_then(Value::as_array_mut) {
        variants.retain(|variant| {
            variant
                .pointer("/properties/type/const")
                .and_then(Value::as_str)
                != Some("verify_contract")
        });
    }
    schema
}
