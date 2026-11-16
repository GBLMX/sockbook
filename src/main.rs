mod cli;
mod enumerate;
mod error;
mod kb;
mod model;
mod owners;
mod paths;
mod render;

#[cfg(not(target_os = "linux"))]
compile_error!("sockbook currently supports Linux only");

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{BrowseArgs, Cli, Command, DbArgs, DbCommand, ListArgs, LookupArgs, SearchArgs};
use crate::error::{Error, Result};
use crate::model::{Proto, SocketView};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("sockbook: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let Cli {
        json,
        color,
        verbose,
        command,
    } = cli;
    let color = color.enabled();

    match command {
        Command::List(args) => cmd_list(args, json, color, verbose),
        Command::Lookup(args) => cmd_lookup(args, json, color),
        Command::Search(args) => cmd_search(args, json, color),
        Command::Browse(args) => cmd_browse(args, json, color),
        Command::Db(args) => cmd_db(args, json),
    }
}

fn cmd_list(args: ListArgs, json: bool, color: bool, verbose: bool) -> Result<ExitCode> {
    let mut sockets = enumerate::sockets()?;
    let enumerated = sockets.len();

    sockets.retain(|socket| args.allows(socket.proto));
    if args.listening {
        sockets.retain(|socket| socket.is_listening());
    }
    if let Some(port) = args.port {
        sockets.retain(|socket| socket.local_port == port);
    }
    if let Some(pid) = args.pid {
        sockets.retain(|socket| socket.owners.iter().any(|owner| owner.pid == pid));
    }

    sockets.sort_by(|left, right| {
        (
            left.local_port,
            proto_rank(left.proto),
            left.local_addr,
            left.remote_port,
        )
            .cmp(&(
                right.local_port,
                proto_rank(right.proto),
                right.local_addr,
                right.remote_port,
            ))
    });

    let views: Vec<SocketView> = if sockets.is_empty() {
        Vec::new()
    } else {
        let (kb, warnings) = kb::Kb::load()?;
        print_warnings(&warnings);
        sockets
            .into_iter()
            .map(|socket| SocketView {
                hints: kb.hints_for(socket.local_port, socket.proto),
                socket,
            })
            .collect()
    };

    if json {
        println!("{}", render::json::sockets(&views)?);
    } else {
        println!("{}", render::table::sockets(&views, color));
    }

    if verbose {
        let owners = views
            .iter()
            .filter(|view| !view.socket.owners.is_empty())
            .count();
        let hints = views.iter().filter(|view| !view.hints.is_empty()).count();
        eprintln!(
            "sockbook: {enumerated} sockets enumerated, {} shown, {owners} with owners, {hints} with hints",
            views.len()
        );
    }

    Ok(ExitCode::SUCCESS)
}

fn cmd_lookup(args: LookupArgs, json: bool, color: bool) -> Result<ExitCode> {
    let (kb, warnings) = kb::Kb::load()?;
    print_warnings(&warnings);

    let matches = kb.lookup(args.port, args.proto);
    if json {
        println!("{}", render::json::lookup(args.port, &matches)?);
    } else {
        println!("{}", render::table::entries(&matches, color));
    }

    if matches.is_empty() {
        eprintln!("sockbook: no knowledge-base entry for port {}", args.port);
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_search(args: SearchArgs, json: bool, color: bool) -> Result<ExitCode> {
    let term = args.term.trim();
    if term.is_empty() {
        return Err(Error::Message("search term must not be empty".to_string()));
    }

    let (kb, warnings) = kb::Kb::load()?;
    print_warnings(&warnings);

    let (matches, total) = kb.search(term, args.limit);
    if json {
        println!("{}", render::json::search(term, total, &matches)?);
    } else {
        println!("{}", render::table::entries(&matches, color));
        if total > matches.len() {
            eprintln!("sockbook: showing {} of {total} matches", matches.len());
        }
    }

    if matches.is_empty() {
        eprintln!("sockbook: no matches for '{term}'");
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_browse(args: BrowseArgs, json: bool, color: bool) -> Result<ExitCode> {
    let (kb, warnings) = kb::Kb::load()?;
    print_warnings(&warnings);

    let (entries, total) = kb.browse(args.category, args.all, args.offset, args.limit);
    if json {
        println!(
            "{}",
            render::json::browse(total, args.offset, args.limit, &entries)?
        );
    } else {
        println!("{}", render::table::entries(&entries, color));
        if entries.is_empty() {
            eprintln!("sockbook: no entries in range");
        } else {
            let start = args.offset.min(total) + 1;
            let end = start + entries.len() - 1;
            eprintln!("sockbook: showing {start}-{end} of {total}");
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn cmd_db(args: DbArgs, json: bool) -> Result<ExitCode> {
    match args.command {
        DbCommand::Update(args) => {
            let installed = if let Some(file) = args.file {
                kb::update::install_file(&file)?
            } else {
                let url = args
                    .url
                    .as_deref()
                    .unwrap_or(kb::update::DEFAULT_UPDATE_URL);
                let bytes = kb::update::fetch(url)?;
                kb::update::install_bytes(&bytes)?
            };

            if json {
                println!(
                    "{}",
                    render::json::update(
                        &installed.path,
                        installed.entries,
                        installed.source.as_deref()
                    )?
                );
            } else {
                println!(
                    "updated {} ({} entries)",
                    installed.path.display(),
                    installed.entries
                );
                if let Some(source) = installed.source {
                    println!("source: {source}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn print_warnings(warnings: &[String]) {
    for warning in warnings {
        eprintln!("sockbook: warning: {warning}");
    }
}

fn proto_rank(proto: Proto) -> u8 {
    match proto {
        Proto::Tcp => 0,
        Proto::Udp => 1,
    }
}
