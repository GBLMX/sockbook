use std::io::IsTerminal;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::model::{Category, Proto};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

impl ColorChoice {
    pub fn enabled(self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                std::io::stdout().is_terminal()
                    && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
            }
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "sockbook",
    version,
    about = "Linux socket inspector with an offline port knowledge base",
    long_about = "Linux socket inspector with an offline port knowledge base.

Socket rows come from the kernel via NETLINK_SOCK_DIAG. Service names are hints
from IANA and a hand-curated overlay; a hint does not assert that the owning
process is that service. Owner attribution is best-effort and depends on
access to /proc/<pid>/fd."
)]
pub struct Cli {
    #[arg(long, global = true, help = "Emit JSON instead of a table")]
    pub json: bool,

    #[arg(
        long,
        global = true,
        value_enum,
        default_value = "auto",
        help = "Colorize table output"
    )]
    pub color: ColorChoice,

    #[arg(long, global = true, help = "Print extra diagnostics to stderr")]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List live sockets
    List(ListArgs),
    /// Look up a port in the knowledge base
    Lookup(LookupArgs),
    /// Search the knowledge base by name, alias, description, or port
    Search(SearchArgs),
    /// Browse knowledge-base entries
    Browse(BrowseArgs),
    /// Manage the downloaded knowledge-base cache
    Db(DbArgs),
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Include TCP sockets
    #[arg(short = 't', long)]
    pub tcp: bool,

    /// Include UDP sockets
    #[arg(short = 'u', long)]
    pub udp: bool,

    /// Only listening sockets (TCP LISTEN; unconnected UDP)
    #[arg(short = 'l', long)]
    pub listening: bool,

    /// Filter by local port
    #[arg(long)]
    pub port: Option<u16>,

    /// Filter by owning PID
    #[arg(long)]
    pub pid: Option<u32>,
}

impl ListArgs {
    pub fn protocols(&self) -> (bool, bool) {
        if !self.tcp && !self.udp {
            (true, true)
        } else {
            (self.tcp, self.udp)
        }
    }

    pub fn allows(&self, proto: Proto) -> bool {
        let (tcp, udp) = self.protocols();
        match proto {
            Proto::Tcp => tcp,
            Proto::Udp => udp,
        }
    }
}

#[derive(Debug, Args)]
pub struct LookupArgs {
    /// Port number
    pub port: u16,

    /// Restrict to one transport protocol (default: both)
    #[arg(value_enum)]
    pub proto: Option<Proto>,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Search term
    pub term: String,

    /// Maximum results; 0 means unlimited
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct BrowseArgs {
    /// Restrict to one category
    #[arg(long, value_enum)]
    pub category: Option<Category>,

    /// Include all IANA entries, not only curated/user entries
    #[arg(long)]
    pub all: bool,

    /// Maximum results; 0 means unlimited
    #[arg(long, default_value_t = 50)]
    pub limit: usize,

    /// Skip this many entries
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
}

#[derive(Debug, Args)]
pub struct DbArgs {
    #[command(subcommand)]
    pub command: DbCommand,
}

#[derive(Debug, Subcommand)]
pub enum DbCommand {
    /// Validate and install a knowledge-base snapshot
    Update(UpdateArgs),
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Read a local file instead of the network
    #[arg(long, conflicts_with = "url")]
    pub file: Option<PathBuf>,

    /// Fetch from an explicit HTTPS URL
    #[arg(long)]
    pub url: Option<String>,
}
