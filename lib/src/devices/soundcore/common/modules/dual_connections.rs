use std::sync::Arc;

use openscq30_lib_has::Has;
use strum::{EnumIter, EnumString};

use crate::{
    device,
    devices::soundcore::common::{
        modules::ModuleCollection,
        packet::{self, PacketIOController, inbound::TryToPacket},
        structures::{DualConnections, DualConnectionsDevice},
    },
    macros::enum_subset,
    settings::{CategoryId, SettingId},
};

mod packet_handler;
mod setting_handler;
mod state_modifier;

enum_subset! {
    SettingId,
    #[derive(EnumIter, EnumString)]
    enum DualConnectionsSetting {
        DualConnections,
        DualConnectionsDevices,
    }
}

impl<T> ModuleCollection<T>
where
    T: Has<DualConnections> + Clone + Send + Sync,
{
    pub fn add_dual_connections(&mut self, packet_io: Arc<PacketIOController>) {
        self.packet_handlers.set_handler(
            packet_handler::DualConnectionsDevicePacketHandler::COMMAND,
            Box::new(packet_handler::DualConnectionsDevicePacketHandler),
        );
        self.setting_manager.add_handler(
            CategoryId::DualConnections,
            setting_handler::DualConnectionsSettingHandler,
        );
        self.state_modifiers
            .push(Box::new(state_modifier::DualConnectionsStateModifier::new(
                packet_io,
            )));
    }
}

pub async fn take_dual_connection_devices(
    packet_io: &PacketIOController,
) -> device::Result<Vec<DualConnectionsDevice>> {
    // The A3959 reports the connected-device list in one or more `[0x0b, 0x02]`
    // packets, each a full snapshot, terminated by a `[0]` packet with no
    // devices. Merge snapshots (dedup by MAC) and stop on the terminator.
    let mut devices: Vec<DualConnectionsDevice> = Vec::new();
    packet_io
        .send_with_multi_response(
            &packet::outbound::request_dual_connections_devices(),
            |packet| {
                let packet: packet::inbound::DualConnectionsDevicePacket = match packet
                    .try_to_packet()
                {
                    Ok(packet) => packet,
                    Err(err) => {
                        tracing::warn!("failed to parse dual connection device packet: {err:?}");
                        return false;
                    }
                };

                let is_end = packet.devices.is_empty();
                for device in packet.devices {
                    if !devices
                        .iter()
                        .any(|existing| existing.mac_address == device.mac_address)
                    {
                        devices.push(device);
                    }
                }

                !is_end
            },
            20,
        )
        .await?;

    Ok(devices)
}
