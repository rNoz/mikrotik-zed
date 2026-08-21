// ── Hover logic for the RSC language server ─────────────────────
//
// When the user hovers over a word, check:
// 1. Is it a menu path (starts with /)?
// 2. Is it a property name for the current menu?
// 3. Is it a standard RouterOS verb?

use crate::menus::MenuData;

/// Find word start (including /, -, _)
fn find_word_start(line: &str, pos: usize) -> usize {
    let pos = pos.min(line.len());
    let mut i = pos;
    while i > 0 {
        let ch = line.as_bytes()[i - 1] as char;
        if !ch.is_ascii_alphanumeric() && ch != '/' && ch != '-' && ch != '_' {
            break;
        }
        i -= 1;
    }
    i
}

/// Find word end (including /, -, _)
fn find_word_end(line: &str, pos: usize) -> usize {
    let pos = pos.min(line.len());
    let mut i = pos;
    while i < line.len() {
        let ch = line.as_bytes()[i] as char;
        if !ch.is_ascii_alphanumeric() && ch != '/' && ch != '-' && ch != '_' {
            break;
        }
        i += 1;
    }
    i
}

#[derive(serde::Serialize)]
pub struct HoverContents {
    pub kind: String,
    pub value: String,
}

#[derive(serde::Serialize)]
pub struct Hover {
    pub contents: HoverContents,
}

pub fn compute_hover(
    data: &MenuData,
    line: &str,
    pos: usize,
    full_doc: &str,
    line_idx: usize,
) -> Option<Hover> {
    let word_start = find_word_start(line, pos);
    let word_end = find_word_end(line, pos);
    let word = &line[word_start..word_end];
    if word.is_empty() {
        return None;
    }

    // Check if it's a menu path
    if word.starts_with('/') {
        if let Some(menu) = data.menu_by_path.get(word) {
            let mut md = format!(
                "### {}\n\n**Type:** {}",
                word,
                if menu.menu_type.is_empty() { "Directory" } else { &menu.menu_type }
            );

            if !menu.arguments.is_empty() {
                md.push_str("\n\n**Arguments:**");
                for arg in &menu.arguments {
                    let typ = if arg.arg_type.is_empty() { "(any)" } else { &arg.arg_type };
                    md.push_str(&format!("\n  {}: {}", arg.name, typ));
                }
            }

            if !menu.flags.is_empty() {
                md.push_str("\n\n**Flags:**");
                for flag in &menu.flags {
                    let desc = if flag.description.is_empty() { "" } else { &flag.description };
                    md.push_str(&format!("\n  {} — {}", flag.name, desc));
                }
            }

            return Some(Hover {
                contents: HoverContents {
                    kind: "markdown".to_string(),
                    value: md,
                },
            });
        }
    }

    // Check if it's a property name for the current menu.
    // The LSP position gives us the line directly; build the before-cursor
    // context from that line and the character offset.
    let before_cursor = crate::build_before_cursor(full_doc, line_idx, pos);
    let context = crate::parse_line(data, &before_cursor);

    if let Some(menu) = data.menu_by_path.get(&context.path) {
        if let Some(arg) = menu.arguments.iter().find(|a| a.name == word) {
            let typ = if arg.arg_type.is_empty() { "any" } else { &arg.arg_type };
            let md = format!("**{}**\n\nType: `{}`", arg.name, typ);
            return Some(Hover {
                contents: HoverContents {
                    kind: "markdown".to_string(),
                    value: md,
                },
            });
        }
        if let Some(flag) = menu.flags.iter().find(|f| f.name == word) {
            let desc = if flag.description.is_empty() { "" } else { &flag.description };
            let md = format!("**{}**\n\n{}", flag.name, desc);
            return Some(Hover {
                contents: HoverContents {
                    kind: "markdown".to_string(),
                    value: md,
                },
            });
        }
    }

    // Check if it's a standard verb
    if MenuData::STANDARD_VERBS.contains(&word) {
        let md = format!("**{}**\n\nStandard RouterOS command.", word);
        return Some(Hover {
            contents: HoverContents {
                kind: "markdown".to_string(),
                value: md,
            },
        });
    }

    None
}
