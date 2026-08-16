use crate::devices::soundcore::common::macros::sound_mode_enum;

// Only used by the SetSoundModes packet in packet_io_controller tests; A3959 uses sound_modes_v2.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Default)]
pub struct SoundModes {
    pub ambient_sound_mode: AmbientSoundMode,
    pub noise_canceling_mode: NoiseCancelingMode,
    pub transparency_mode: TransparencyMode,
    pub custom_noise_canceling: CustomNoiseCanceling,
}

sound_mode_enum!(
    pub enum AmbientSoundMode {
        NoiseCanceling = 0,
        Transparency = 1,
        Normal = 2,
    }
);

sound_mode_enum!(
    pub enum NoiseCancelingMode {
        Transport = 0,
        Outdoor = 1,
        Indoor = 2,
        Custom = 3,
    }
);

sound_mode_enum!(
    pub enum TransparencyMode {
        FullyTransparent = 0,
        VocalMode = 1,
    }
);

// Only used by the SetSoundModes packet in packet_io_controller tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct CustomNoiseCanceling {
    value: u8,
}

#[allow(dead_code)]
impl CustomNoiseCanceling {
    pub fn new(value: u8) -> Self {
        // Not sure what 255 means here, but it is allowed in addition to 0-10
        let clamped_value = if value == 255 {
            value
        } else {
            value.clamp(0, 10)
        };
        Self {
            value: clamped_value,
        }
    }

    pub fn value(&self) -> u8 {
        self.value
    }
}
