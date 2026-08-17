use nom::{
    IResult, Parser,
    error::{ContextError, ParseError, context},
};

use crate::devices::soundcore::common::{
    self,
    packet::{self, Command, outbound::ToPacket},
    structures::DualConnectionsDevice,
};

use super::FromPacketBody;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DualConnectionsDevicePacket {
    pub total_packets: u8,
    /// Index starts from 1
    pub current_packet_index: u8,
    pub devices: Vec<common::structures::DualConnectionsDevice>,
}

impl DualConnectionsDevicePacket {
    pub const COMMAND: Command = Command([0x0b, 0x02]);
}

impl ToPacket for DualConnectionsDevicePacket {
    type DirectionMarker = packet::InboundMarker;

    fn command(&self) -> packet::Command {
        Self::COMMAND
    }

    fn body(&self) -> Vec<u8> {
        self.devices
            .iter()
            .flat_map(|device| device.bytes())
            .collect()
    }
}

impl FromPacketBody for DualConnectionsDevicePacket {
    type DirectionMarker = packet::InboundMarker;

    fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        context("dual connection device", |input: &'a [u8]| {
            // A3959 format: the body is a snapshot of the connected-device list.
            // Each connected device is `[0x01, mac(6)]`. A single `[0x00]` byte
            // marks the end of the list (no name, count header, or length prefix,
            // unlike the a3936).
            let mut devices = Vec::new();
            let mut rest = input;
            while !rest.is_empty() {
                // Determine whether this record is a device (flag == 1) or the
                // end marker (flag == 0).
                let flag = rest[0];
                if flag == 0 {
                    break;
                }
                let (remaining, device) = DualConnectionsDevice::take(rest)?;
                devices.push(device);
                rest = remaining;
            }

            Ok((
                &[] as &[u8],
                Self {
                    total_packets: devices.len() as u8,
                    current_packet_index: devices.len() as u8,
                    devices,
                },
            ))
        })
        .parse_complete(input)
    }
}

#[cfg(test)]
mod tests {
    use nom_language::error::VerboseError;

    use super::*;

    #[test]
    fn empty_list() {
        // `[0]` marks an empty device list / end of list.
        let initial = [0x00];
        let (remaining, parsed) =
            DualConnectionsDevicePacket::take::<VerboseError<_>>(&initial).unwrap();
        assert_eq!(remaining.len(), 0);
        assert!(parsed.devices.is_empty());
    }

    #[test]
    fn single_device() {
        // `[1, mac(6)]` — one connected device, no name field.
        let initial = [0x01, 0x27, 0x65, 0xF4, 0x18, 0xB3, 0xE4];
        let (remaining, parsed) =
            DualConnectionsDevicePacket::take::<VerboseError<_>>(&initial).unwrap();
        assert_eq!(remaining.len(), 0);
        assert_eq!(parsed.devices.len(), 1);
        assert!(parsed.devices[0].is_connected);
        assert_eq!(
            parsed.devices[0].mac_address.to_string(),
            "27:65:F4:18:B3:E4"
        );
        assert_eq!(parsed.devices[0].name, "");
    }

    #[test]
    fn multiple_devices_then_terminator() {
        // Two connected devices followed by a `[0]` terminator.
        let initial = [
            0x01, 0x27, 0x65, 0xF4, 0x18, 0xB3, 0xE4, // phone
            0x01, 0x01, 0x00, 0x00, 0x46, 0x00, 0x00, // placeholder
            0x00, // end of list
        ];
        let (remaining, parsed) =
            DualConnectionsDevicePacket::take::<VerboseError<_>>(&initial).unwrap();
        assert_eq!(remaining.len(), 0);
        assert_eq!(parsed.devices.len(), 2);
        assert_eq!(
            parsed.devices[0].mac_address.to_string(),
            "27:65:F4:18:B3:E4"
        );
        assert_eq!(
            parsed.devices[1].mac_address.to_string(),
            "01:00:00:46:00:00"
        );
    }
}
