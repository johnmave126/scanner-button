use std::{
    fmt::Display,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    num::Wrapping,
    sync::Arc,
};

use anyhow::{ensure, Context};
use bjnp::{
    serdes::{Deserialize, Serialize},
    Packet, PacketBuilder, PacketHeaderOnly, PayloadType,
};
use log::{debug, trace};
use pretty_hex::PrettyHex;
use tokio::net::UdpSocket;

#[derive(Debug)]
pub struct Channel {
    socket: Arc<UdpSocket>,
    sequence: Wrapping<u16>,
}

impl Channel {
    pub async fn new(addr: SocketAddr) -> anyhow::Result<Self> {
        const IPV4_ANY: IpAddr = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
        const IPV6_ANY: IpAddr = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0));

        let local_ip = if addr.is_ipv4() { IPV4_ANY } else { IPV6_ANY };
        let local = SocketAddr::new(local_ip.into(), 0);
        let socket = UdpSocket::bind(local)
            .await
            .with_context(|| format!("couldn't bind to {local}"))?;
        debug!("binded socket to {local}");

        socket
            .connect(addr)
            .await
            .with_context(|| format!("couldn't connect to remote socket {addr}"))?;
        debug!("connected socket to {addr}");

        Ok(Self {
            socket: Arc::new(socket),
            sequence: Wrapping(0),
        })
    }

    pub async fn send<T: Serialize + Display>(
        &mut self,
        payload_type: PayloadType,
        payload: T,
    ) -> anyhow::Result<()> {
        let peer = self.socket.peer_addr().unwrap();

        let command = PacketBuilder::new(bjnp::PacketType::ScannerCommand, payload_type)
            .sequence(self.sequence.0)
            .build(payload);
        debug!("sending {payload_type} command to {peer}: {command:-}",);

        let buffer = command.serialize_to_vec();
        trace!(
            "outbound packet to {peer}: {buffer:?}",
            buffer = buffer.hex_dump()
        );

        self.socket
            .send(buffer.as_slice())
            .await
            .with_context(|| format!("couldn't send packet `[{payload_type}]` to {peer}"))?;

        self.sequence += 1;
        trace!("sequence to {peer}: {sequence}", sequence = self.sequence);

        Ok(())
    }

    pub async fn recv<T: Deserialize + Display>(&self) -> anyhow::Result<T> {
        let peer = self.socket.peer_addr().unwrap();

        let mut buffer = [0; 65536];
        let size = self.socket.recv(&mut buffer).await?;
        let buffer = &buffer[..size];
        trace!(
            "inbound packet from {peer}: {buffer:?}",
            buffer = buffer.hex_dump()
        );
        let packet = PacketHeaderOnly::parse(buffer)?;
        trace!("inbound packet {packet}");
        ensure!(
            packet.error() == 0 || packet.payload_size() > 0,
            "remote peer {peer} returns error code `{err:#02x}`",
            err = packet.error()
        );

        let packet = Packet::<T>::try_from(packet)?;
        debug!(
            "decoded {payload_type} response: {packet:-}",
            payload_type = packet.payload_type()
        );
        Ok(packet.payload())
    }

    pub fn reset_sequence(&mut self) {
        trace!("sequence reset to 0");
        self.sequence = Wrapping(0);
    }
}
