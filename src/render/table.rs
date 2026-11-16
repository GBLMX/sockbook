use std::net::{IpAddr, SocketAddr};

use crate::model::{ServiceEntry, SocketView, Tier};

use super::{render_grid, Column};

pub fn sockets(views: &[SocketView], color: bool) -> String {
    let columns = [
        Column {
            header: "Proto",
            max_width: 5,
        },
        Column {
            header: "State",
            max_width: 11,
        },
        Column {
            header: "Local",
            max_width: 28,
        },
        Column {
            header: "Peer",
            max_width: 28,
        },
        Column {
            header: "PID",
            max_width: 10,
        },
        Column {
            header: "Process",
            max_width: 28,
        },
        Column {
            header: "Hint",
            max_width: 28,
        },
    ];

    let rows: Vec<Vec<String>> = views
        .iter()
        .map(|view| {
            let socket = &view.socket;
            let pids = socket
                .owners
                .iter()
                .map(|owner| owner.pid.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let process = process_cell(view);
            let hint = hint_cell(&view.hints);
            vec![
                socket.proto.as_str().to_string(),
                socket.state.as_table().to_string(),
                endpoint(socket.local_addr, socket.local_port),
                endpoint(socket.remote_addr, socket.remote_port),
                if pids.is_empty() {
                    "-".to_string()
                } else {
                    pids
                },
                process,
                hint,
            ]
        })
        .collect();

    render_grid(&columns, &rows, color, Some(6))
}

pub fn entries(entries: &[&ServiceEntry], color: bool) -> String {
    let columns = [
        Column {
            header: "Port",
            max_width: 6,
        },
        Column {
            header: "Proto",
            max_width: 7,
        },
        Column {
            header: "Name",
            max_width: 30,
        },
        Column {
            header: "Category",
            max_width: 12,
        },
        Column {
            header: "Tier",
            max_width: 8,
        },
        Column {
            header: "Description",
            max_width: 64,
        },
    ];

    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|entry| {
            let name = if entry.aliases.is_empty() {
                entry.name.clone()
            } else {
                format!("{} ({})", entry.name, entry.aliases.join(", "))
            };
            vec![
                entry.port.to_string(),
                entry
                    .protos
                    .to_vec()
                    .iter()
                    .map(|proto| proto.as_str())
                    .collect::<Vec<_>>()
                    .join("+"),
                name,
                entry
                    .category
                    .map(|category| category.as_str().to_string())
                    .unwrap_or_else(|| "-".to_string()),
                entry.tier.as_str().to_string(),
                if entry.description.is_empty() {
                    "-".to_string()
                } else {
                    entry.description.clone()
                },
            ]
        })
        .collect();

    render_grid(&columns, &rows, color, None)
}

fn process_cell(view: &SocketView) -> String {
    let Some(owner) = view.socket.owners.first() else {
        return "-".to_string();
    };
    let process = if !owner.cmdline.is_empty() {
        owner.cmdline.clone()
    } else {
        owner.exe.clone().unwrap_or_else(|| "?".to_string())
    };
    if view.socket.owners.len() > 1 {
        format!("{process} (+{})", view.socket.owners.len() - 1)
    } else {
        process
    }
}

fn hint_cell(hints: &[ServiceEntry]) -> String {
    let Some(first) = hints.first() else {
        return "-".to_string();
    };
    let tier = if first.tier == Tier::Iana {
        "iana hint"
    } else {
        "hint"
    };
    let extra = if hints.len() > 1 {
        format!(", +{}", hints.len() - 1)
    } else {
        String::new()
    };
    format!("{} ({tier}{extra})", first.name)
}

fn endpoint(addr: IpAddr, port: u16) -> String {
    if port == 0 {
        match addr {
            IpAddr::V4(addr) => format!("{addr}:*"),
            IpAddr::V6(addr) => format!("[{addr}]:*"),
        }
    } else {
        SocketAddr::new(addr, port).to_string()
    }
}
