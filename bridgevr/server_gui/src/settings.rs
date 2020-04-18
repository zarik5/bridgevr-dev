use serde_json as json;
use settings_schema::{SchemaNode, SchemaNodeType};

const SETTINGS_SCHEMA: &str = env!("SETTINGS_SCHEMA");

fn get_default(schema: &SchemaNode) -> json::Value {
    match &schema.node_type {
        SchemaNodeType::Section { entries } => json::Value::Object(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), get_default(value)))
                .collect(),
        ),
        SchemaNodeType::Choice { variants, default } => {
            let maybe_entry = variants.iter().find(|(key, _)| key == default);
            if let Some((_, maybe_value)) = maybe_entry {
                match maybe_value {
                    Some(value) => json::json!({ default: get_default(value) }),
                    None => json::json!(default),
                }
            } else {
                println!("Error: {}", default);
                panic!()
            }
        }
        SchemaNodeType::Optional {
            default_set,
            content,
        } => {
            if *default_set {
                get_default(&*content)
            } else {
                json::json!(null)
            }
        }
        SchemaNodeType::Switch {
            default_enabled,
            content,
        } => {
            if *default_enabled {
                json::json!({ "Enabled": get_default(&*content) })
            } else {
                json::json!("Disabled")
            }
        }
        SchemaNodeType::Boolean { default } => json::json!(default),
        &SchemaNodeType::Integer { default, .. } => {
            // json!() does not support i128
            if default.is_negative() {
                json::json!(default as i64)
            } else {
                json::json!(default as u64)
            }
        }
        SchemaNodeType::Float { default, .. } => json::json!(default),
        SchemaNodeType::Text { default } => json::json!(default),
        SchemaNodeType::Array(array) => {
            let array = array.iter().map(get_default).collect::<Vec<_>>();
            json::json!(array)
        }
        SchemaNodeType::Vector { default, .. } => default.clone(),
        SchemaNodeType::Dictionary { default, .. } => default.clone(),
    }
}

pub fn generate_default_settings() -> String {
    let schema = json::from_str(SETTINGS_SCHEMA).unwrap();
    json::to_string_pretty(&get_default(&schema)).unwrap()
}
