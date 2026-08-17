use macaddr::MacAddr6;
use nom::{
    IResult, Parser,
    error::{ContextError, ParseError, context},
    number::complete::le_u8,
};

use crate::devices::soundcore::common::packet::parsing::take_bool;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DualConnections {
    pub is_enabled: bool,
    pub devices: Vec<DualConnectionsDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DualConnectionsDevice {
    pub is_connected: bool,
    pub mac_address: MacAddr6,
    pub name: String,
}

impl DualConnectionsDevice {
    pub fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        context("dual connection device", |input| {
            // A3959 format: `[is_connected(bool), mac_address(6)]`. No name and
            // no length prefix (unlike the a3936, which interleaved a name).
            let (input, (is_connected, mac_address_bytes)) =
                (take_bool, (le_u8, le_u8, le_u8, le_u8, le_u8, le_u8)).parse_complete(input)?;

            let mac_address = MacAddr6::from(<[u8; 6]>::from(mac_address_bytes));
            let name = String::new();

            Ok((
                input,
                Self {
                    is_connected,
                    mac_address,
                    name,
                },
            ))
        })
        .parse_complete(input)
    }

    pub fn bytes(&self) -> impl Iterator<Item = u8> {
        [self.is_connected as u8]
            .into_iter()
            .chain(self.mac_address.into_array())
    }
}
