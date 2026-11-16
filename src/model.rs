use std::fmt;
use std::net::IpAddr;

use bitflags::bitflags;
use clap::ValueEnum;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lower")]
pub enum Proto {
    Tcp,
    Udp,
}

impl Proto {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }

    pub fn from_loose(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "tcp" => Some(Self::Tcp),
            "udp" => Some(Self::Udp),
            _ => None,
        }
    }
}

impl fmt::Display for Proto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ProtoSet: u8 {
        const TCP = 1 << 0;
        const UDP = 1 << 1;
    }
}

impl ProtoSet {
    pub fn from_proto(proto: Proto) -> Self {
        match proto {
            Proto::Tcp => Self::TCP,
            Proto::Udp => Self::UDP,
        }
    }

    pub fn to_vec(self) -> Vec<Proto> {
        let mut out = Vec::with_capacity(2);
        if self.contains(Self::TCP) {
            out.push(Proto::Tcp);
        }
        if self.contains(Self::UDP) {
            out.push(Proto::Udp);
        }
        out
    }

    pub fn from_raw(raw: &[String]) -> Option<Self> {
        let mut set = Self::empty();
        for value in raw {
            set |= Self::from_proto(Proto::from_loose(value)?);
        }
        (!set.is_empty()).then_some(set)
    }
}

impl Serialize for ProtoSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let protos = self.to_vec();
        let mut seq = serializer.serialize_seq(Some(protos.len()))?;
        for proto in protos {
            seq.serialize_element(&proto)?;
        }
        seq.end()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Closed,
    Listen,
    SynSent,
    SynRecv,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
    DeleteTcb,
    Unconn,
    Unknown,
}

impl State {
    pub fn as_table(self) -> &'static str {
        match self {
            Self::Closed => "CLOSED",
            Self::Listen => "LISTEN",
            Self::SynSent => "SYN_SENT",
            Self::SynRecv => "SYN_RCVD",
            Self::Established => "ESTABLISHED",
            Self::FinWait1 => "FIN_WAIT_1",
            Self::FinWait2 => "FIN_WAIT_2",
            Self::CloseWait => "CLOSE_WAIT",
            Self::Closing => "CLOSING",
            Self::LastAck => "LAST_ACK",
            Self::TimeWait => "TIME_WAIT",
            Self::DeleteTcb => "DELETE_TCB",
            Self::Unconn => "UNCONN",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    User,
    Curated,
    Iana,
}

impl Tier {
    pub fn rank(self) -> u8 {
        match self {
            Self::User => 0,
            Self::Curated => 1,
            Self::Iana => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Curated => "curated",
            Self::Iana => "iana",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "user" => Some(Self::User),
            "curated" => Some(Self::Curated),
            "iana" => Some(Self::Iana),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
pub enum Category {
    Web,
    Database,
    Cache,
    Queue,
    Monitoring,
    Devtool,
    Container,
    Remote,
    Mail,
    Dns,
    Vpn,
    File,
    Infra,
    Other,
}

impl Category {
    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "web" => Some(Self::Web),
            "database" | "db" => Some(Self::Database),
            "cache" => Some(Self::Cache),
            "queue" | "message-queue" => Some(Self::Queue),
            "monitoring" | "observability" => Some(Self::Monitoring),
            "devtool" | "dev-tool" | "development" => Some(Self::Devtool),
            "container" | "containers" => Some(Self::Container),
            "remote" | "remote-access" => Some(Self::Remote),
            "mail" => Some(Self::Mail),
            "dns" => Some(Self::Dns),
            "vpn" => Some(Self::Vpn),
            "file" | "file-sharing" => Some(Self::File),
            "infra" | "infrastructure" => Some(Self::Infra),
            "other" | "misc" => Some(Self::Other),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Database => "database",
            Self::Cache => "cache",
            Self::Queue => "queue",
            Self::Monitoring => "monitoring",
            Self::Devtool => "devtool",
            Self::Container => "container",
            Self::Remote => "remote",
            Self::Mail => "mail",
            Self::Dns => "dns",
            Self::Vpn => "vpn",
            Self::File => "file",
            Self::Infra => "infra",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceEntry {
    pub port: u16,
    pub protos: ProtoSet,
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub category: Option<Category>,
    pub homepage: Option<String>,
    pub notes: Option<String>,
    pub tier: Tier,
}

#[derive(Debug, Clone, Serialize)]
pub struct Owner {
    pub pid: u32,
    pub cmdline: String,
    pub exe: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SocketRecord {
    pub proto: Proto,
    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: IpAddr,
    pub remote_port: u16,
    pub state: State,
    pub inode: u32,
    pub uid: u32,
    pub owners: Vec<Owner>,
}

impl SocketRecord {
    /// Listening semantics used by `list --listening`.
    ///
    /// UDP sockets have no connection state in netstat2 0.11, so every UDP
    /// socket is treated as unconnected/listening.
    pub fn is_listening(&self) -> bool {
        matches!(self.state, State::Listen | State::Unconn)
    }
}

#[derive(Debug, Clone)]
pub struct SocketView {
    pub socket: SocketRecord,
    pub hints: Vec<ServiceEntry>,
}
