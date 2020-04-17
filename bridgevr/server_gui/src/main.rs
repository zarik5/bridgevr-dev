use iced::{Element, Sandbox, Settings, Text};

const BVR_SERVER_VERSION: &str = env!("BVR_SERVER_VERSION");
const SETTINGS_SCHEMA: &str = env!("SETTINGS_SCHEMA");

struct Gui;
impl Sandbox for Gui {
    type Message = ();

    fn new() -> Self {
        Self
    }

    fn title(&self) -> String {
        format!("BridgeVR v{}", BVR_SERVER_VERSION)
    }

    fn update(&mut self, message: Self::Message) {
        // iced::sty
    }

    fn view(&mut self) -> Element<Self::Message> {
        let fdjsfhs = (0..3).map(|i| {
            serde_json::json!(7)
        }).collect::<Vec<_>>();
        let fkdlsfjklsd = serde_json::json!({
            "hello": fdjsfhs
        });
        Text::new(serde_json::to_string_pretty(&fkdlsfjklsd).unwrap()).into()
    }
}

fn main() {
    let json_value: serde_json::Value = serde_json::from_str(SETTINGS_SCHEMA).unwrap();
    std::fs::write("settings.json", serde_json::to_string_pretty(&json_value).unwrap()).unwrap();

    Gui::run(Settings::default());
}
