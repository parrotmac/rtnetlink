// SPDX-License-Identifier: MIT

use std::net::{IpAddr, Ipv4Addr};

use futures::stream::StreamExt;

use crate::{
    packet::{
        nlas::address::Nla,
        AddressMessage, NetlinkMessage, RtnlMessage, NLM_F_ACK, NLM_F_REQUEST,
        AF_INET, AF_INET6,
    },
    try_nl, Error, Handle,
};

pub struct AddressDelRequest {
    handle: Handle,
    message: AddressMessage,
}

impl AddressDelRequest {
    pub(crate) fn new(handle: Handle, index: u32, address: IpAddr, prefix_len: u8) -> Self {
        let mut message = AddressMessage::default();

        message.header.prefix_len = prefix_len;
        message.header.index = index;

        let address_vec = match address {
            IpAddr::V4(ipv4) => {
                message.header.family = AF_INET as u8;
                ipv4.octets().to_vec()
            }
            IpAddr::V6(ipv6) => {
                message.header.family = AF_INET6 as u8;
                ipv6.octets().to_vec()
            }
        };

        if address.is_multicast() {
            message.nlas.push(Nla::Multicast(address_vec));
        } else if address.is_unspecified() {
            message.nlas.push(Nla::Unspec(address_vec));
        } else if address.is_ipv6() {
            message.nlas.push(Nla::Address(address_vec));
        } else {
            message.nlas.push(Nla::Address(address_vec.clone()));

            // for IPv4 the IFA_LOCAL address can be set to the same value as
            // IFA_ADDRESS
            message.nlas.push(Nla::Local(address_vec.clone()));

            // set the IFA_BROADCAST address as well (IPv6 does not support
            // broadcast)
            if prefix_len == 32 {
                message.nlas.push(Nla::Broadcast(address_vec));
            } else {
                let ip_addr: u32 = u32::from(Ipv4Addr::new(
                    address_vec[0],
                    address_vec[1],
                    address_vec[2],
                    address_vec[3],
                ));
                let brd = Ipv4Addr::from(
                    (0xffff_ffff_u32) >> u32::from(prefix_len) | ip_addr,
                );
                message.nlas.push(Nla::Broadcast(brd.octets().to_vec()));
            };
        }

        AddressDelRequest { handle, message }
    }

    pub(crate) fn flush(handle: Handle, address: AddressMessage) -> Self {
        AddressDelRequest {
            handle,
            message: address,
        }
    }

    /// Execute the request
    pub async fn execute(self) -> Result<(), Error> {
        let AddressDelRequest {
            mut handle,
            message,
        } = self;

        let mut req = NetlinkMessage::from(RtnlMessage::DelAddress(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK;
        let mut response = handle.request(req)?;
        while let Some(msg) = response.next().await {
            try_nl!(msg);
        }
        Ok(())
    }

    pub fn message_mut(&mut self) -> &mut AddressMessage {
        &mut self.message
    }
}
