// ── Completion logic for the RSC language server ─────────────────
//
// Port of the ls.mjs completion engine.  Strategy:
// - Always return ALL possible candidates (sub-menus, verbs, arguments)
//   and let Zed's fuzzy filter narrow them down.
// - Exception: when cursor sits right after "property=", switch to
//   value suggestions (enum values, booleans, type hints).

use crate::menus::{LineContext, MenuData};

/// LSP CompletionItemKind values (mirrors the LSP spec)
mod kind {
    pub const FUNCTION: i32 = 3;
    pub const PROPERTY: i32 = 5;
    pub const CLASS: i32 = 9;
    pub const ENUM_MEMBER: i32 = 12;
    pub const CONSTANT: i32 = 14;
}

/// A completion item ready for JSON serialization.
#[derive(serde::Serialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<i32>,
    pub detail: Option<String>,
    pub insert_text: Option<String>,
    #[serde(rename = "insertTextFormat")]
    pub insert_text_format: Option<i32>,
}

pub fn compute_completions(data: &MenuData, before_cursor: &str) -> Vec<CompletionItem> {
    let context = crate::parse_line(data, before_cursor);

    // No path yet → suggest root menus
    if context.path.is_empty() {
        return get_root_completion_items(data);
    }

    // Typing a property value right after "=" → suggest enum/bool/type values
    if let Some(eq_pos) = context.last_token.rfind('=')
        && eq_pos == context.last_token.len() - 1
    {
        let key = &context.last_token[..eq_pos];
        return get_value_completions(data, &context, key);
    }

    // If a verb is already typed (e.g., "add", "print"), only suggest
    // arguments — no more sub-menus or verbs.  This matches real RouterOS
    // terminal behavior where Tab after "add" shows property completions.
    if context.command.is_some() {
        return get_arg_completion_items(data, &context);
    }

    // Before a verb: suggest sub-menus + standard verbs
    let mut items = Vec::new();
    items.extend(get_sub_menu_completion_items(data, &context));
    items.extend(get_verb_completion_items(data, &context));
    items
}

// ── Root menus ──────────────────────────────────────────────────

fn get_root_completion_items(data: &MenuData) -> Vec<CompletionItem> {
    match data.child_names_by_parent.get("") {
        Some(roots) => roots
            .iter()
            .map(|r| CompletionItem {
                label: r.path.clone(),
                kind: Some(kind::CLASS),
                detail: Some(format!("root menu — {}", r.path)),
                insert_text: Some(r.path.clone()),
                insert_text_format: Some(1),
            })
            .collect(),
        None => Vec::new(),
    }
}

// ── Sub-menus ───────────────────────────────────────────────────

fn get_sub_menu_completion_items(data: &MenuData, ctx: &LineContext) -> Vec<CompletionItem> {
    match data.child_names_by_parent.get(&ctx.path) {
        Some(children) => children
            .iter()
            .filter(|c| c.menu_type == "Directory" || c.menu_type == "Settings Directory")
            .map(|c| CompletionItem {
                label: c.name.clone(),
                kind: Some(kind::CLASS),
                detail: Some(format!("sub-menu — {}", c.path)),
                insert_text: Some(c.name.clone()),
                insert_text_format: Some(1),
            })
            .collect(),
        None => Vec::new(),
    }
}

// ── Verbs ───────────────────────────────────────────────────────

fn get_verb_completion_items(data: &MenuData, ctx: &LineContext) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = MenuData::STANDARD_VERBS
        .iter()
        .map(|verb| CompletionItem {
            label: verb.to_string(),
            kind: Some(kind::FUNCTION),
            detail: Some(format!("{verb} — standard command")),
            insert_text: Some(verb.to_string()),
            insert_text_format: Some(1),
        })
        .collect();

    // Action commands (type = "Command" entries under this path)
    if let Some(children) = data.child_names_by_parent.get(&ctx.path) {
        for child in children {
            if child.menu_type == "Command" {
                items.push(CompletionItem {
                    label: child.name.clone(),
                    kind: Some(kind::FUNCTION),
                    detail: Some("action command".to_string()),
                    insert_text: Some(child.name.clone()),
                    insert_text_format: Some(1),
                });
            }
        }
    }

    items
}

// ── Arguments ───────────────────────────────────────────────────

fn get_arg_completion_items(data: &MenuData, ctx: &LineContext) -> Vec<CompletionItem> {
    let menu = match data.menu_by_path.get(&ctx.path) {
        Some(m) => m,
        None => return Vec::new(),
    };

    let mut items = Vec::new();

    for arg in &menu.arguments {
        if ctx.properties.contains_key(&arg.name) {
            continue; // already used
        }
        let insert_text = get_insert_text(arg);
        items.push(CompletionItem {
            label: arg.name.clone(),
            kind: Some(kind::PROPERTY),
            detail: Some(get_detail(arg)),
            insert_text: Some(insert_text),
            insert_text_format: Some(2), // snippet
        });
    }

    for flag in &menu.flags {
        items.push(CompletionItem {
            label: flag.name.clone(),
            kind: Some(kind::CONSTANT),
            detail: Some(format!("{}: {}", flag.name, flag.description)),
            insert_text: Some(flag.name.clone()),
            insert_text_format: Some(1),
        });
    }

    items
}

// ── Value completions (after "property=") ───────────────────────

fn get_value_completions(
    data: &MenuData,
    ctx: &LineContext,
    property_key: &str,
) -> Vec<CompletionItem> {
    let menu = match data.menu_by_path.get(&ctx.path) {
        Some(m) => m,
        None => return Vec::new(),
    };

    let arg = match menu.arguments.iter().find(|a| a.name == property_key) {
        Some(a) => a,
        None => return Vec::new(),
    };

    let mut items = Vec::new();

    // Enum values
    if arg.arg_type.starts_with("enum") {
        for val in parse_enum_values(&arg.arg_type) {
            items.push(CompletionItem {
                label: val.clone(),
                kind: Some(kind::ENUM_MEMBER),
                detail: Some(format!("enum value — {}", arg.arg_type)),
                insert_text: Some(val),
                insert_text_format: Some(1),
            });
        }
    }

    // Boolean
    if arg.arg_type == "bool" || arg.arg_type == "boolean" {
        for val in &["yes", "no", "true", "false"] {
            items.push(CompletionItem {
                label: val.to_string(),
                kind: Some(kind::ENUM_MEMBER),
                detail: Some("bool value".to_string()),
                insert_text: Some(val.to_string()),
                insert_text_format: Some(1),
            });
        }
    }

    // Interface references
    if arg.arg_type.starts_with("iface_enum") {
        for val in &["ether1", "bridge"] {
            items.push(CompletionItem {
                label: val.to_string(),
                kind: Some(kind::ENUM_MEMBER),
                detail: Some("common interface name".to_string()),
                insert_text: Some(val.to_string()),
                insert_text_format: Some(1),
            });
        }
    }

    // IP address / prefix
    if arg.arg_type.starts_with("ipAddr")
        || arg.arg_type.starts_with("ipPrefix")
        || arg.arg_type == "address"
    {
        items.push(CompletionItem {
            label: "0.0.0.0/0".to_string(),
            kind: Some(kind::ENUM_MEMBER),
            detail: Some(format!("type: {}", arg.arg_type)),
            insert_text: Some("0.0.0.0/0".to_string()),
            insert_text_format: Some(1),
        });
    }

    items
}

// ── Helpers ─────────────────────────────────────────────────────

fn parse_enum_values(type_str: &str) -> Vec<String> {
    let inner = type_str
        .strip_prefix("enum")
        .and_then(|s| s.trim().strip_prefix('('))
        .and_then(|s| s.strip_suffix(')'));
    match inner {
        Some(body) => body.split('|').map(|s| s.trim().to_string()).collect(),
        None => Vec::new(),
    }
}

fn get_insert_text(arg: &crate::menus::ArgEntry) -> String {
    if arg.arg_type == "string" {
        format!("{}=\"{}\"", arg.name, "$1")
    } else {
        format!("{}={}", arg.name, "$1")
    }
}

fn get_detail(arg: &crate::menus::ArgEntry) -> String {
    if arg.arg_type.is_empty() {
        "property".to_string()
    } else {
        format!("type: {}", arg.arg_type)
    }
}
