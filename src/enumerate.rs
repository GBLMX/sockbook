use std::fs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};

use crate::error::{Error, Result};
use crate::model::{Proto, SocketRecord, State};
use crate::owners::OwnerResolver;

pub fn sockets() -> Result<Vec<SocketRecord>> {
    // netstat2 panics via expect() if /proc cannot be read. Check first so the
    // common failure gets a useful message instead of a panic.
    if fs::read_dir("/proc").is_err() {
        return Err(Error::NoProc);
    }

    let infos = get_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP | ProtocolFlags::UDP,
    )?;

    let mut owner_resolver = OwnerResolver::new();
    let mut sockets = Vec::with_capacity(infos.len());

    for info in infos {
        let owners = owner_resolver.resolve(&info.associated_pids);
        let (proto, local_addr, local_port, remote_addr, remote_port, state) =
            match info.protocol_socket_info {
                ProtocolSocketInfo::Tcp(tcp) => (
                    Proto::Tcp,
                    tcp.local_addr,
                    tcp.local_port,
                    tcp.remote_addr,
                    tcp.remote_port,
                    state_from_tcp(tcp.state),
                ),
                ProtocolSocketInfo::Udp(udp) => (
                    Proto::Udp,
                    udp.local_addr,
                    udp.local_port,
                    unspecified_for(udp.local_addr),
                    0,
                    State::Unconn,
                ),
            };

        sockets.push(SocketRecord {
            proto,
            local_addr,
            local_port,
            remote_addr,
            remote_port,
            state,
            inode: info.inode,
            uid: info.uid,
            owners,
        });
    }

    Ok(sockets)
}

fn unspecified_for(local_addr: IpAddr) -> IpAddr {
    match local_addr {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    }
}

fn state_from_tcp(state: TcpState) -> State {
    match state {
        TcpState::Closed => State::Closed,
        TcpState::Listen => State::Listen,
        TcpState::SynSent => State::SynSent,
        TcpState::SynReceived => State::SynRecv,
        TcpState::Established => State::Established,
        TcpState::FinWait1 => State::FinWait1,
        TcpState::FinWait2 => State::FinWait2,
        TcpState::CloseWait => State::CloseWait,
        TcpState::Closing => State::Closing,
        TcpState::LastAck => State::LastAck,
        TcpState::TimeWait => State::TimeWait,
        TcpState::DeleteTcb => State::DeleteTcb,
        TcpState::Unknown => State::Unknown,
    }
}
