use serde::{Deserialize, Serialize};
use serde_json as json;

pub use settings_schema_derive::*;

#[derive(Serialize, Deserialize, Clone)]
pub enum Switch<T> {
    Enabled(T),
    Disabled,
}

impl<T> Switch<T> {
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Enabled(t) => Some(t),
            Self::Disabled => None,
        }
    }
}

#[derive(Clone)]
pub struct SwitchDefault<C> {
    pub enabled: bool,
    pub content: C,
}

#[derive(Clone)]
pub struct OptionalDefault<C> {
    pub set: bool,
    pub content: C,
}

#[derive(Clone)]
pub struct VectorDefault<C, D> {
    pub element: C,
    pub default: Vec<D>,
}

#[derive(Clone)]
pub struct DictionaryDefault<V, D> {
    pub key: String,
    pub value: V,
    pub default: Vec<(String, D)>,
}

#[derive(Serialize, Deserialize)]
pub enum NumericGuiType {
    TextBox,
    UpDown,
    Slider,
}

#[derive(Serialize, Deserialize)]
pub enum SchemaNodeType {
    Section {
        entries: Vec<(String, SchemaNode)>,
    },
    Choice {
        variants: Vec<(String, Option<SchemaNode>)>,
        default: String,
    },
    Optional {
        default_set: bool,
        content: Box<SchemaNode>,
    },
    Switch {
        default_enabled: bool,
        content: Box<SchemaNode>,
    },
    Boolean {
        default: bool,
    },
    Integer {
        default: i128,
        min: i128,
        max: i128,
        step: i128,
        gui: Option<NumericGuiType>,
    },
    Float {
        default: f64,
        min: Option<f64>,
        max: Option<f64>,
        step: Option<f64>,
        gui: Option<NumericGuiType>,
    },
    Text {
        default: String,
    },
    Array(Vec<SchemaNode>),
    Vector {
        default_element: Box<SchemaNode>,
        default: json::Value,
    },
    Dictionary {
        default_key: String,
        default_value: Box<SchemaNode>,
        default: json::Value,
    },
}

#[derive(Serialize, Deserialize)]
pub struct SchemaNode {
    pub advanced: bool,
    pub node_type: SchemaNodeType,
}
