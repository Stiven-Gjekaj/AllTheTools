//! `att wol`: wake a computer on the local network.
//!
//! No operating system ships a tool that sends a Wake-on-LAN packet. This one
//! sends the magic packet as a UDP broadcast, which every network card that
//! supports Wake-on-LAN reads.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use anyhow::{Context, Result};

pub mod parse;

/// Wake a computer on the local network with a magic packet.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The hardware address of the computer to wake, as aa:bb:cc:dd:ee:ff.
    #[arg(required = true)]
    pub mac: Vec<String>,

    /// The address to send to. The default reaches the local network.
    #[arg(short, long, default_value = "255.255.255.255", value_name = "ADDRESS")]
    pub broadcast: Ipv4Addr,

    /// The UDP port. Most cards read port 9, and some read port 7.
    #[arg(short, long, default_value_t = 9)]
    pub port: u16,

    /// The SecureOn password of the card, when it has one.
    #[arg(long, value_name = "PASSWORD")]
    pub password: Option<String>,
}

pub fn run(args: Args) -> Result<i32> {
    let password = args.password.as_deref().map(parse::password).transpose()?;
    let targets = args
        .mac
        .iter()
        .map(|m| parse::mac(m))
        .collect::<Result<Vec<_>>>()?;

    let socket =
        UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).context("could not open a UDP socket")?;
    socket
        .set_broadcast(true)
        .context("the host refused a broadcast on the UDP socket")?;
    let to = SocketAddr::from((args.broadcast, args.port));
    for mac in targets {
        send(&socket, to, mac, password)?;
        println!("Sent the magic packet for {} to {to}.", parse::show(mac));
    }
    Ok(0)
}

fn send(socket: &UdpSocket, to: SocketAddr, mac: [u8; 6], password: Option<[u8; 6]>) -> Result<()> {
    let packet = parse::packet(mac, password);
    let sent = socket
        .send_to(&packet, to)
        .with_context(|| format!("could not send the magic packet to {to}"))?;
    anyhow::ensure!(
        sent == packet.len(),
        "the host sent {sent} of {} bytes",
        packet.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::send;
    use std::net::{Ipv4Addr, UdpSocket};
    use std::time::Duration;

    // The packet goes to a socket of this test on the loopback address, so
    // nothing leaves the computer.
    #[test]
    fn the_packet_arrives_whole() {
        let listener = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mac = [1, 2, 3, 4, 5, 6];

        send(&sender, listener.local_addr().unwrap(), mac, None).unwrap();

        let mut buffer = [0u8; 256];
        let (size, _) = listener.recv_from(&mut buffer).unwrap();
        assert_eq!(&buffer[..size], super::parse::packet(mac, None).as_slice());
    }
}
