use crate::data::*;

pub enum InputValue {
    Boolean(bool),
    NormalizedOneSided(f32),
    NormalizedTwoSided(f32),
    Skeletal(),
}

pub fn input_device_data_to_str_value_map(
    input_device_data: &InputDeviceData,
) -> Vec<(&str, InputValue)> {
    match input_device_data {
        InputDeviceData::Gamepad {
            thumbstick_left_horizontal,
            thumbstick_left_vertical,
            thumbstick_right_horizontal,
            thumbstick_right_vertical,
            trigger_left,
            trigger_right,
            digital_input,
        } => vec![
            (
                "/gamepad/left/joystick/x",
                InputValue::NormalizedTwoSided(*thumbstick_left_horizontal),
            ),
            (
                "/gamepad/left/joystick/y",
                InputValue::NormalizedTwoSided(*thumbstick_left_vertical),
            ),
            (
                "/gamepad/right/joystick/x",
                InputValue::NormalizedTwoSided(*thumbstick_right_horizontal),
            ),
            (
                "/gamepad/right/joystick/y",
                InputValue::NormalizedTwoSided(*thumbstick_right_vertical),
            ),
            (
                "/gamepad/left/trigger/value",
                InputValue::NormalizedOneSided(*trigger_left),
            ),
            (
                "/gamepad/right/trigger/value",
                InputValue::NormalizedOneSided(*trigger_right),
            ),
            (
                "/gamepad/a/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::A)),
            ),
            (
                "/gamepad/b/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::B)),
            ),
            (
                "/gamepad/x/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::X)),
            ),
            (
                "/gamepad/y/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::Y)),
            ),
            (
                "/gamepad/dpad/left/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::DPAD_LEFT)),
            ),
            (
                "/gamepad/dpad/right/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::DPAD_RIGHT)),
            ),
            (
                "/gamepad/dpad/up/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::DPAD_UP)),
            ),
            (
                "/gamepad/dpad/down/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::DPAD_DOWN)),
            ),
            (
                "/gamepad/left/joystick/click",
                InputValue::Boolean(
                    digital_input.contains(GamepadDigitalInput::JOYSTICK_LEFT_CLICK),
                ),
            ),
            (
                "/gamepad/right/joystick/click",
                InputValue::Boolean(
                    digital_input.contains(GamepadDigitalInput::JOYSTICK_RIGHT_CLICK),
                ),
            ),
            (
                "/gamepad/left/shoulder/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::SHOULDER_LEFT)),
            ),
            (
                "/gamepad/right/shoulder/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::SHOULDER_RIGHT)),
            ),
            (
                "/gamepad/menu/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::MENU)),
            ),
            (
                "/gamepad/view/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::VIEW)),
            ),
            (
                "/gamepad/home/click",
                InputValue::Boolean(digital_input.contains(GamepadDigitalInput::HOME)),
            ),
        ],
        InputDeviceData::OculusTouchPair {
            thumbstick_left_horizontal,
            thumbstick_left_vertical,
            thumbstick_right_horizontal,
            thumbstick_right_vertical,
            trigger_left,
            trigger_right,
            grip_left,
            grip_right,
            digital_input,
        } => vec![
            (
                "/oculus_touch/left/joystick/x",
                InputValue::NormalizedTwoSided(*thumbstick_left_horizontal),
            ),
            (
                "/oculus_touch/left/joystick/y",
                InputValue::NormalizedTwoSided(*thumbstick_left_vertical),
            ),
            (
                "/oculus_touch/right/joystick/x",
                InputValue::NormalizedTwoSided(*thumbstick_right_horizontal),
            ),
            (
                "/oculus_touch/right/joystick/y",
                InputValue::NormalizedTwoSided(*thumbstick_right_vertical),
            ),
            (
                "/oculus_touch/left/trigger/value",
                InputValue::NormalizedOneSided(*trigger_left),
            ),
            (
                "/oculus_touch/right/trigger/value",
                InputValue::NormalizedOneSided(*trigger_right),
            ),
            (
                "/oculus_touch/left/grip/value",
                InputValue::NormalizedOneSided(*grip_left),
            ),
            (
                "/oculus_touch/right/grip/value",
                InputValue::NormalizedOneSided(*grip_right),
            ),
            (
                "/oculus_touch/a/click",
                InputValue::Boolean(digital_input.contains(OculusTouchDigitalInput::A_CLICK)),
            ),
            (
                "/oculus_touch/a/touch",
                InputValue::Boolean(digital_input.contains(OculusTouchDigitalInput::A_TOUCH)),
            ),
            (
                "/oculus_touch/b/click",
                InputValue::Boolean(digital_input.contains(OculusTouchDigitalInput::B_CLICK)),
            ),
            (
                "/oculus_touch/b/touch",
                InputValue::Boolean(digital_input.contains(OculusTouchDigitalInput::B_TOUCH)),
            ),
            (
                "/oculus_touch/x/click",
                InputValue::Boolean(digital_input.contains(OculusTouchDigitalInput::X_CLICK)),
            ),
            (
                "/oculus_touch/x/touch",
                InputValue::Boolean(digital_input.contains(OculusTouchDigitalInput::X_TOUCH)),
            ),
            (
                "/oculus_touch/y/click",
                InputValue::Boolean(digital_input.contains(OculusTouchDigitalInput::Y_CLICK)),
            ),
            (
                "/oculus_touch/y/touch",
                InputValue::Boolean(digital_input.contains(OculusTouchDigitalInput::Y_TOUCH)),
            ),
            (
                "/oculus_touch/left/joystick/click",
                InputValue::Boolean(
                    digital_input.contains(OculusTouchDigitalInput::THUMBSTICK_LEFT_CLICK),
                ),
            ),
            (
                "/oculus_touch/left/joystick/touch",
                InputValue::Boolean(
                    digital_input.contains(OculusTouchDigitalInput::THUMBSTICK_LEFT_TOUCH),
                ),
            ),
            (
                "/oculus_touch/right/joystick/click",
                InputValue::Boolean(
                    digital_input.contains(OculusTouchDigitalInput::THUMBSTICK_RIGHT_CLICK),
                ),
            ),
            (
                "/oculus_touch/right/joystick/touch",
                InputValue::Boolean(
                    digital_input.contains(OculusTouchDigitalInput::THUMBSTICK_RIGHT_TOUCH),
                ),
            ),
            (
                "/oculus_touch/left/trigger/touch",
                InputValue::Boolean(
                    digital_input.contains(OculusTouchDigitalInput::TRIGGER_LEFT_TOUCH),
                ),
            ),
            (
                "/oculus_touch/right/trigger/touch",
                InputValue::Boolean(
                    digital_input.contains(OculusTouchDigitalInput::TRIGGER_RIGHT_TOUCH),
                ),
            ),
            (
                "/oculus_touch/left/grip/touch",
                InputValue::Boolean(
                    digital_input.contains(OculusTouchDigitalInput::GRIP_LEFT_TOUCH),
                ),
            ),
            (
                "/oculus_touch/right/grip/touch",
                InputValue::Boolean(
                    digital_input.contains(OculusTouchDigitalInput::GRIP_RIGHT_TOUCH),
                ),
            ),
            (
                "/oculus_touch/menu/click",
                InputValue::Boolean(digital_input.contains(OculusTouchDigitalInput::MENU)),
            ),
            (
                "/oculus_touch/home/click",
                InputValue::Boolean(digital_input.contains(OculusTouchDigitalInput::HOME)),
            ),
        ],
        InputDeviceData::OculusGoController {
            trigger,
            touchpad_horizontal,
            touchpad_vertical,
            digital_input,
        } => vec![
            (
                "/oculus_go/trigger/value",
                InputValue::NormalizedOneSided(*trigger),
            ),
            (
                "/oculus_go/touchpad/x",
                InputValue::NormalizedTwoSided(*touchpad_horizontal),
            ),
            (
                "/oculus_go/touchpad/y",
                InputValue::NormalizedTwoSided(*touchpad_vertical),
            ),
            (
                "/oculus_go/touchpad/click",
                InputValue::Boolean(digital_input.contains(OculusGoDigitalInput::TOUCHPAD_CLICK)),
            ),
            (
                "/oculus_go/touchpad/touch",
                InputValue::Boolean(digital_input.contains(OculusGoDigitalInput::TOUCHPAD_TOUCH)),
            ),
            (
                "/oculus_go/back/click",
                InputValue::Boolean(digital_input.contains(OculusGoDigitalInput::BACK)),
            ),
            (
                "/oculus_go/home/click",
                InputValue::Boolean(digital_input.contains(OculusGoDigitalInput::HOME)),
            ),
        ],
        _ => todo!(),
    }
}