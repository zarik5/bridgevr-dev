use bridgevr_common::{data::*, *};
use std::{fs, path::PathBuf};

const TRACE_CONTEXT: &str = "Settings schema exporter";

#[derive(serde::Serialize)]
enum TestEnum {
    Variant1,
    Variant2(u32),
    Variant3 { field: String },
    Variant4,
}

#[derive(serde::Serialize)]
struct TestStruct {
    field1: bridgevr_common::data::Switch<Vec<TestEnum>>,
    field2: Vec<(String, u32)>,
    field3: Option<f32>,
    field4: Option<f32>,
}
// Save settings schema
fn main() -> StrResult {

    // let fdjkslfjl = semver::VersionReq::

    let test_value = TestStruct {
        field1: bridgevr_common::data::Switch::Enabled(vec![
            TestEnum::Variant1,
            TestEnum::Variant2(42),
            TestEnum::Variant3 {
                field: "hello world".into(),
            },
            TestEnum::Variant4,
        ]),
        field2: vec![("hello".into(), 1), ("world".into(), 2)],
        field3: Some(123.4),
        field4: None,
    };

    trace_err!(fs::write(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/settings_schema.json"),
        // trace_err!(serde_json::to_string_pretty(&data::settings_schema()))?,
        trace_err!(serde_json::to_string_pretty(&test_value))?,
    ))
}
