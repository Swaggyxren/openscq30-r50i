mod dual_connections;
mod outbound_packet;
mod request_battery_charging;
mod request_battery_level;
mod request_state;
mod set_ambient_sound_mode_cycle;
mod set_auto_power_off;
mod set_button_configuration;
mod set_equalizer;
mod set_equalizer_with_drc;
mod set_sound_modes;

pub use dual_connections::*;
pub use outbound_packet::*;
#[allow(
    unused_imports,
    reason = "TODO consider polling with one of these every once in a while if it doesn't push this information to us"
)]
pub use request_battery_charging::*;
#[allow(
    unused_imports,
    reason = "TODO consider polling with one of these every once in a while if it doesn't push this information to us"
)]
pub use request_battery_level::*;
pub use request_state::*;
pub use set_ambient_sound_mode_cycle::*;
pub use set_auto_power_off::*;
pub use set_button_configuration::*;
pub use set_equalizer::*;
pub use set_equalizer_with_drc::*;
#[allow(
    unused_imports,
    reason = "SetSoundModes is only used by the packet_io_controller tests"
)]
pub use set_sound_modes::*;

use crate::devices::soundcore::common::packet;

pub const SET_GAMING_MODE_COMMAND: packet::Command = packet::Command([0x01, 0x87]);
pub const SET_TOUCH_TONE_COMMAND: packet::Command = packet::Command([0x01, 0x83]);
pub const SET_LOW_BATTERY_PROMPT_COMMAND: packet::Command = packet::Command([0x10, 0x82]);
