mod settings;

use iced::{
    button, checkbox, scrollable, Align, Button, Column, Container, Element, Length, Row, Sandbox,
    Scrollable, Settings, Space, Text, TextInput,
};

const BVR_SERVER_VERSION: &str = env!("BVR_SERVER_VERSION");

enum MonitorMode {
    Events,
    Log,
}

enum SettingsViewMode {
    Basic,
    Advanced,
    Text { save_button_state: button::State },
}

#[derive(Debug, Clone)]
enum Tab {
    Monitor,
    Settings,
    About,
}

#[derive(Debug, Clone)]
enum Action {}

enum MessageBoxIconType {
    Info,
    Warning,
}

struct MessageBox {
    icon_type: MessageBoxIconType,
    message: String,
    ok_button_state: button::State,
    cancel_button_state: Option<button::State>,
    do_not_show_again_checkbox_checked: Option<bool>,
    ok_action: Option<Action>,
}

#[derive(Debug, Clone)]
enum Event {
    TabSelected(Tab),
    Request(Action),
    MessageBoxOk,
    MessageBoxCancel,
}

struct Gui {
    selected_tab: Tab,
    monitor_mode: MonitorMode,
    settings_view_mode: SettingsViewMode,
    message_box: Option<MessageBox>,
}

impl Sandbox for Gui {
    type Message = Event;

    fn new() -> Self {
        // Self {
        //     // scroll_state: <_>::default(),
        //     // json_string: serde_json::to_string_pretty(
        //     //     &serde_json::from_str::<serde_json::Value>(SETTINGS_SCHEMA).unwrap(),
        //     // )
        //     // .unwrap(),
        // }
        // todo!()
        Self {
            selected_tab: Tab::Monitor,
            monitor_mode: MonitorMode::Events,
            settings_view_mode: SettingsViewMode::Basic,
            message_box: Some(MessageBox {
                icon_type: MessageBoxIconType::Info,
                message: "Generate settings?".into(),
                ok_button_state: <_>::default(),
                cancel_button_state: Some(<_>::default()),
                do_not_show_again_checkbox_checked: None,
                ok_action: None,
            }),
        }
    }

    fn title(&self) -> String {
        format!("BridgeVR v{}", BVR_SERVER_VERSION)
    }

    fn update(&mut self, event: Event) {
        match event {
            Event::MessageBoxOk => {
                settings::generate_default_settings();
                std::fs::write(
                    "./settings.json",
                    settings::generate_default_settings()
                )
                .unwrap();
            }
            Event::MessageBoxCancel => (),
            _ => (),
        }
    }

    fn view(&mut self) -> Element<Event> {
        if let Some(message_box) = &mut self.message_box {
            let mut buttons = Row::new().spacing(10);
            if let Some(state) = &mut message_box.cancel_button_state {
                buttons = buttons
                    .push(Button::new(state, Text::new("Cancel")).on_press(Event::MessageBoxOk));
            }
            buttons = buttons.push(
                Button::new(&mut message_box.ok_button_state, Text::new("Ok"))
                    .on_press(Event::MessageBoxOk),
            );
            Container::new(
                Column::new()
                    .spacing(10)
                    .align_items(Align::Center)
                    .push(Text::new(&message_box.message))
                    .push(buttons),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
        } else {
            todo!()
        }
    }
}

fn main() {
    std::fs::write(
        "./settings_schema.json",
        serde_json::to_string_pretty(
            &serde_json::from_str::<serde_json::Value>(env!("SETTINGS_SCHEMA")).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    Gui::run(Settings::default());
}
