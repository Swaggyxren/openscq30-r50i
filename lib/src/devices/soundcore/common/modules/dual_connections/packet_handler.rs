use async_trait::async_trait;
use openscq30_lib_has::Has;
use tokio::sync::watch;

use crate::{
    api::device,
    devices::soundcore::common::{
        packet::{self, Command, inbound::TryToPacket},
        packet_manager::PacketHandler,
        structures::DualConnections,
    },
};

#[derive(Default)]
pub struct DualConnectionsDevicePacketHandler;

impl DualConnectionsDevicePacketHandler {
    pub const COMMAND: Command = packet::inbound::DualConnectionsDevicePacket::COMMAND;
}

#[async_trait]
impl<T> PacketHandler<T> for DualConnectionsDevicePacketHandler
where
    T: Has<DualConnections> + Send + Sync,
{
    async fn handle_packet(
        &self,
        state: &watch::Sender<T>,
        packet: &packet::Inbound,
    ) -> device::Result<()> {
        let packet: packet::inbound::DualConnectionsDevicePacket = packet.try_to_packet()?;
        state.send_modify(|state| {
            let dual_connections = state.get_mut();
            modify_state(dual_connections, packet);
        });
        Ok(())
    }
}

#[inline(never)]
fn modify_state(
    dual_connections: &mut DualConnections,
    packet: packet::inbound::DualConnectionsDevicePacket,
) {
    tracing::debug!(
        "got dual connections devices packet {}/{}",
        packet.current_packet_index,
        packet.total_packets
    );
    // Each A3959 packet is a full snapshot of the connected-device list, so
    // replace the previous list rather than appending to it.
    dual_connections.devices = packet.devices;
}

#[cfg(test)]
mod tests {
    use macaddr::MacAddr6;
    use openscq30_lib_macros::Has;

    use crate::devices::soundcore::common::structures::DualConnectionsDevice;

    use super::*;

    #[derive(Has)]
    struct TestState {
        dual_connections: DualConnections,
    }

    #[tokio::test(start_paused = true)]
    async fn replaces_list_with_latest_snapshot() {
        let handler = DualConnectionsDevicePacketHandler;
        let (state_sender, state_receiver) = watch::channel(TestState {
            dual_connections: DualConnections {
                is_enabled: true,
                devices: Vec::new(),
            },
        });

        // First snapshot lists device A.
        handler
            .handle_packet(
                &state_sender,
                &packet::Inbound::new(
                    packet::inbound::DualConnectionsDevicePacket::COMMAND,
                    vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
                ),
            )
            .await
            .unwrap();

        // A later snapshot lists devices B and C; it must replace the list.
        handler
            .handle_packet(
                &state_sender,
                &packet::Inbound::new(
                    packet::inbound::DualConnectionsDevicePacket::COMMAND,
                    vec![
                        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // B
                        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, // C
                    ],
                ),
            )
            .await
            .unwrap();

        let state = state_receiver.borrow();
        assert_eq!(
            state.dual_connections.devices,
            vec![
                DualConnectionsDevice {
                    is_connected: true,
                    mac_address: MacAddr6::new(0, 0, 0, 0, 0, 2),
                    name: String::new(),
                },
                DualConnectionsDevice {
                    is_connected: true,
                    mac_address: MacAddr6::new(0, 0, 0, 0, 0, 3),
                    name: String::new(),
                },
            ]
        )
    }

    #[tokio::test(start_paused = true)]
    async fn empty_snapshot_clears_list() {
        let handler = DualConnectionsDevicePacketHandler;
        let (state_sender, state_receiver) = watch::channel(TestState {
            dual_connections: DualConnections {
                is_enabled: true,
                devices: vec![DualConnectionsDevice {
                    is_connected: true,
                    mac_address: MacAddr6::new(0, 0, 0, 0, 0, 1),
                    name: String::new(),
                }],
            },
        });

        // A `[0]` packet reports an empty list.
        handler
            .handle_packet(
                &state_sender,
                &packet::Inbound::new(
                    packet::inbound::DualConnectionsDevicePacket::COMMAND,
                    vec![0x00],
                ),
            )
            .await
            .unwrap();

        let state = state_receiver.borrow();
        assert!(state.dual_connections.devices.is_empty())
    }
}
