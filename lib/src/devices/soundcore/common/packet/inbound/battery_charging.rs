use nom::{
    IResult, Parser,
    combinator::{all_consuming, map},
    error::{ContextError, ParseError, context},
    sequence::pair,
};

use crate::devices::soundcore::common::{
    packet::{self, Command},
    structures::IsBatteryCharging,
};

use super::FromPacketBody;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DualBatteryCharging {
    pub left: IsBatteryCharging,
    pub right: IsBatteryCharging,
}

impl DualBatteryCharging {
    pub const COMMAND: Command = Command([0x01, 0x04]);
}

impl FromPacketBody for DualBatteryCharging {
    type DirectionMarker = packet::InboundMarker;

    fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        context(
            "DualBatteryChargingUpdatePacket",
            all_consuming(map(
                pair(IsBatteryCharging::take, IsBatteryCharging::take),
                |(left, right)| Self { left, right },
            )),
        )
        .parse_complete(input)
    }
}

#[cfg(test)]
mod tests {
    use nom_language::error::VerboseError;

    use crate::devices::soundcore::common::packet;

    use super::*;

    #[test]
    fn it_parses_a_manually_crafted_packet() {
        let input: &[u8] = &[
            0x09, 0xff, 0x00, 0x00, 0x01, 0x01, 0x04, 0x0c, 0x00, 0x01, 0x00, 0x1b,
        ];
        let (_, packet) = packet::Inbound::take_with_checksum::<VerboseError<_>>(input).unwrap();
        let packet = DualBatteryCharging::take::<VerboseError<_>>(&packet.body)
            .unwrap()
            .1;

        assert_eq!(IsBatteryCharging::Yes, packet.left);
        assert_eq!(IsBatteryCharging::No, packet.right);
    }
}
