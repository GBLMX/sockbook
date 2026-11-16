pub mod json;
pub mod table;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub struct Column {
    pub header: &'static str,
    pub max_width: usize,
}

/// Remove terminal control sequences and bidi overrides from a table cell.
///
/// Command lines and downloaded knowledge-base text are untrusted. JSON output
/// intentionally keeps raw strings; only the human table is sanitized.
pub fn sanitize(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut previous_space = false;
    for ch in value.chars() {
        let ch = match ch {
            '\n' | '\r' | '\t' | '\u{2028}' | '\u{2029}' => ' ',
            other => other,
        };
        if ch.is_control() || is_bidi_control(ch) {
            continue;
        }
        if ch == ' ' {
            if previous_space {
                continue;
            }
            previous_space = true;
        } else {
            previous_space = false;
        }
        out.push(ch);
    }
    out.trim().to_string()
}

fn is_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

pub fn render_grid(
    columns: &[Column],
    rows: &[Vec<String>],
    color: bool,
    dim_column: Option<usize>,
) -> String {
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| row.iter().map(|cell| sanitize(cell)).collect())
        .collect();

    let mut widths: Vec<usize> = columns
        .iter()
        .map(|column| UnicodeWidthStr::width(column.header).max(1))
        .collect();

    for row in &rows {
        for (index, cell) in row.iter().enumerate().take(widths.len()) {
            widths[index] = widths[index].max(UnicodeWidthStr::width(cell.as_str()));
        }
    }
    for (index, column) in columns.iter().enumerate() {
        widths[index] = widths[index].min(column.max_width.max(1));
    }

    let mut lines = Vec::with_capacity(rows.len() + 2);

    let header = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let cell = fit(&column.header.to_uppercase(), widths[index]);
            if color {
                format!("\x1b[1m{cell}\x1b[0m")
            } else {
                cell
            }
        })
        .collect::<Vec<_>>()
        .join("  ");
    lines.push(header.trim_end().to_string());

    lines.push(
        widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("  "),
    );

    for row in &rows {
        let cells = widths
            .iter()
            .enumerate()
            .map(|(index, width)| {
                let value = row.get(index).map(String::as_str).unwrap_or("");
                let cell = fit(value, *width);
                if color && dim_column == Some(index) {
                    format!("\x1b[2m{cell}\x1b[0m")
                } else {
                    cell
                }
            })
            .collect::<Vec<_>>()
            .join("  ");
        lines.push(cells.trim_end().to_string());
    }

    lines.join("\n")
}

fn fit(value: &str, width: usize) -> String {
    let width = width.max(1);
    if UnicodeWidthStr::width(value) <= width {
        return pad(value, width);
    }

    let target = width.saturating_sub(1);
    let mut out = String::new();
    let mut used = 0;
    for ch in value.chars() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + char_width > target {
            break;
        }
        out.push(ch);
        used += char_width;
    }
    out.push('…');
    used += 1;
    if used < width {
        out.push_str(&" ".repeat(width - used));
    }
    out
}

fn pad(value: &str, width: usize) -> String {
    let current = UnicodeWidthStr::width(value);
    let mut out = String::with_capacity(value.len() + width.saturating_sub(current));
    out.push_str(value);
    out.push_str(&" ".repeat(width.saturating_sub(current)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_removes_controls_and_bidi() {
        assert_eq!(sanitize("a\x1b[31mb\u{202e}c\nx"), "a[31mbc x");
        assert_eq!(sanitize("  lots   of\tspace  "), "lots of space");
    }

    #[test]
    fn fit_respects_display_width() {
        for width in 1..=8 {
            let rendered = fit("数据库abc", width);
            assert_eq!(UnicodeWidthStr::width(rendered.as_str()), width);
        }
    }

    #[test]
    fn grid_includes_header_separator_and_rows() {
        let columns = [
            Column {
                header: "A",
                max_width: 4,
            },
            Column {
                header: "B",
                max_width: 8,
            },
        ];
        let rows = vec![vec!["one".to_string(), "two".to_string()]];
        let grid = render_grid(&columns, &rows, false, None);
        assert_eq!(grid.lines().count(), 3);
        assert!(grid.starts_with("A "));
        assert!(grid.contains("one"));
    }
}
