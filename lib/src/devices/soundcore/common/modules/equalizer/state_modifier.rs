use async_trait::async_trait;
use openscq30_lib_has::Has;
use std::sync::Arc;
use tokio::sync::watch;

use crate::{
    api::device,
    devices::soundcore::common::{
        packet::{self, PacketIOController},
        state_modifier::StateModifier,
        structures::EqualizerConfiguration,
    },
};

pub struct EqualizerStateModifier<
    const CHANNELS: usize,
    const BANDS: usize,
    const MIN_VOLUME: i16,
    const MAX_VOLUME: i16,
    const FRACTION_DIGITS: u8,
> {
    packet_io: Arc<PacketIOController>,
    options: EqualizerStateModifierOptions,
}

pub struct EqualizerStateModifierOptions {
    pub has_drc: bool,
}

impl<
    const CHANNELS: usize,
    const BANDS: usize,
    const MIN_VOLUME: i16,
    const MAX_VOLUME: i16,
    const FRACTION_DIGITS: u8,
> EqualizerStateModifier<CHANNELS, BANDS, MIN_VOLUME, MAX_VOLUME, FRACTION_DIGITS>
{
    pub fn new(packet_io: Arc<PacketIOController>, options: EqualizerStateModifierOptions) -> Self {
        Self { packet_io, options }
    }
}

#[async_trait]
impl<
    T,
    const CHANNELS: usize,
    const BANDS: usize,
    const MIN_VOLUME: i16,
    const MAX_VOLUME: i16,
    const FRACTION_DIGITS: u8,
> StateModifier<T>
    for EqualizerStateModifier<CHANNELS, BANDS, MIN_VOLUME, MAX_VOLUME, FRACTION_DIGITS>
where
    T: Has<EqualizerConfiguration<CHANNELS, BANDS, MIN_VOLUME, MAX_VOLUME, FRACTION_DIGITS>>
        + Clone
        + Send
        + Sync,
{
    async fn move_to_state(
        &self,
        state_sender: &watch::Sender<T>,
        target_state: &T,
    ) -> device::Result<()> {
        let target_equalizer_configuration = target_state.get();
        {
            let state = state_sender.borrow();
            let equalizer_configuration = state.get();
            if equalizer_configuration == target_equalizer_configuration {
                return Ok(());
            }
        }

        self.packet_io
            .send_with_response(&if self.options.has_drc {
                packet::outbound::set_equalizer_with_drc(target_equalizer_configuration)
            } else {
                packet::outbound::set_equalizer(target_equalizer_configuration)
            })
            .await?;
        state_sender.send_modify(|state| *state.get_mut() = *target_equalizer_configuration);
        Ok(())
    }
}
