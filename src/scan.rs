use std::{
    io::{self, Write},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use anyhow::{ensure, Context};
use bjnp::{
    discover, identity,
    serdes::{Empty, Serialize},
    Packet, PacketBuilder, PacketHeaderOnly, PacketType, PayloadType,
};
use log::{debug, error, info, trace};
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use owo_colors::{OwoColorize, Style};
use pretty_hex::PrettyHex;
use stream::wrappers::UnboundedReceiverStream;
use tokio::{
    net::UdpSocket,
    sync::mpsc::unbounded_channel,
    task::JoinSet,
    time::{sleep_until, Instant},
};
use tokio_stream::{self as stream, StreamExt, StreamMap};

use crate::{channel::Channel, utils::BJNP_PORT};

pub async fn scan(max_waiting: u64) -> anyhow::Result<()> {
    // binding to 0.0.0.0 relies on system routing table, so it is
    // more robust to get all the local IP and bind to them.
    let interfaces =
        NetworkInterface::show().context("couldn't obtain the list of network interfaces")?;
    let mut task_set = JoinSet::new();
    let mut map = interfaces
        .into_iter()
        .filter(|interface| interface.addr.is_some())
        .map(|interface| {
            let receiver = broadcast_scan(&mut task_set, &interface);
            ((interface.name, interface.addr.unwrap().ip()), receiver)
        })
        .collect::<StreamMap<_, _>>();

    let deadline = Instant::now() + Duration::from_secs(max_waiting);
    let sleep = sleep_until(deadline);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            Some(((name, addr), maybe_resp)) = map.next() => {
                // received response
                match maybe_resp {
                    Ok(resp) => {
                        info!("detected device at {addr}");
                        task_set.spawn(inquire_device(resp));
                    },
                    Err(e) => {
                        error!("socket at {addr} on {name}: {e:?}");
                    },
                }
            },
            Some(join_result) = task_set.join_next() => {
                // some job terminateed early
                if let Err(e) = join_result
                    .context("failed to join task")
                    .and_then(std::convert::identity)
                {
                    error!("socket error: {e:?}");
                }
            },
            _ = &mut sleep => {
                break;
            }
        }
    }
    // Clear tasks
    task_set.shutdown().await;
    Ok(())
}

fn broadcast_scan(
    set: &mut JoinSet<anyhow::Result<()>>,
    interface: &NetworkInterface,
) -> UnboundedReceiverStream<anyhow::Result<discover::Response>> {
    const IPV4_BROADCAST: Ipv4Addr = Ipv4Addr::new(255, 255, 255, 255);
    const IPV6_LINKLOCAL_MULTICAST: Ipv6Addr = Ipv6Addr::new(0xFF02, 0, 0, 0, 0, 0, 0, 1);

    let (sender, receiver) = unbounded_channel();

    set.spawn({
        let name = interface.name.clone();
        let ifaddr = interface.addr.unwrap().clone();

        async move {
            // create socket
            let (socket, broadcast) = match ifaddr {
                network_interface::Addr::V4(addr) => {
                    let socket = UdpSocket::bind(SocketAddr::new(addr.ip.into(), 0))
                        .await
                        .with_context(|| {
                            format!("couldn't bind to {ip} on {name}", ip = addr.ip)
                        })?;
                    socket.set_broadcast(true).with_context(|| {
                        format!(
                            "couldn't set socket for {ip} on {name} to broadcast",
                            ip = addr.ip
                        )
                    })?;
                    let broadcast = addr.broadcast.unwrap_or(IPV4_BROADCAST);
                    let broadcast = SocketAddr::new(broadcast.into(), BJNP_PORT);
                    (socket, broadcast)
                }
                network_interface::Addr::V6(addr) => {
                    let socket = UdpSocket::bind(SocketAddr::new(addr.ip.into(), 0))
                        .await
                        .with_context(|| {
                            format!("couldn't bind to {ip} on {name}", ip = addr.ip)
                        })?;
                    let broadcast = SocketAddr::new(IPV6_LINKLOCAL_MULTICAST.into(), BJNP_PORT);
                    (socket, broadcast)
                }
            };
            let local = socket
                .local_addr()
                .with_context(|| format!("couldn't obtain socket local address on {name}"))?;
            debug!("binded socket to {local} on {name}");

            // create command
            let command =
                PacketBuilder::new(PacketType::ScannerCommand, PayloadType::Discover).build(Empty);

            debug!("broadcast discover command to {broadcast} on {name}: {command:-}",);

            // broadcast command
            let buffer = command.serialize_to_vec();
            socket
                .send_to(buffer.as_slice(), broadcast)
                .await
                .with_context(|| {
                    format!("failed to broadcast to {broadcast} from {local} on {name}")
                })?;

            // receiving command
            let mut buffer = [0; 65536];
            loop {
                let resp = socket
                    .recv_from(&mut buffer)
                    .await
                    .with_context(|| format!("error receiving packet at {local} on {name}",))
                    .and_then(|(size, remote)| {
                        // parsing

                        let buffer = &buffer[..size];
                        trace!(
                            "inbound packet from {remote}: {buffer:?}",
                            buffer = buffer.hex_dump()
                        );

                        let packet = PacketHeaderOnly::parse(buffer)?;
                        ensure!(
                            packet.error() == 0 || packet.payload_size() > 0,
                            "remote peer {remote} returns error code `{err:#02x}`",
                            err = packet.error()
                        );

                        let packet: Packet<discover::Response> = packet.try_into()?;
                        Ok(packet.payload())
                    });

                if sender.send(resp).is_err() {
                    trace!("receiving end of {local} on {name} closed");
                    break;
                }
            }

            Ok::<(), anyhow::Error>(())
        }
    });

    receiver.into()
}

async fn inquire_device(device: discover::Response) -> anyhow::Result<()> {
    let mut channel = Channel::new(SocketAddr::new(*device.ip_addr(), BJNP_PORT)).await?;
    channel.send(PayloadType::GetId, Empty).await?;
    let id: identity::Response = channel.recv().await?;
    let mut id: Vec<_> = id.iter().collect();
    id.sort();

    let key_style = Style::new().bright_blue();
    let value_style = Style::new().bright_yellow();

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    writeln!(
        handle,
        "Scanner {IP}={ip} {MAC}={mac}",
        IP = "IP".if_supports_color(owo_colors::Stream::Stdout, |v| v.style(key_style)),
        MAC = "MAC".if_supports_color(owo_colors::Stream::Stdout, |v| v.style(key_style)),
        ip = format!("{addr}:{BJNP_PORT}", addr = device.ip_addr())
            .if_supports_color(owo_colors::Stream::Stdout, |v| v.style(value_style)),
        mac = device
            .mac_addr()
            .if_supports_color(owo_colors::Stream::Stdout, |v| v.style(value_style)),
    )
    .context("failed to write to stdout")?;
    for (key, value) in id.iter() {
        writeln!(
            handle,
            "  {key}: {value}",
            key = key.if_supports_color(owo_colors::Stream::Stdout, |v| v.style(key_style)),
            value = value.if_supports_color(owo_colors::Stream::Stdout, |v| v.style(value_style))
        )
        .context("failed to write to stdout")?;
    }

    Ok(())
}
