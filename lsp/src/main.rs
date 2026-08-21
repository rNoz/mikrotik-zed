// ── MikroTik RouterOS Script Language Server ─────────────────────
//
// LSP over stdio, implemented in pure Rust.  Commands.toml is
// embedded at compile time — no external files needed.
//
// LSP handlers:
//   textDocument/completion – menu path, command verb, and property suggestions
//   textDocument/hover        – description for commands and properties

mod completion;
mod hover;
mod menus;

use menus::{LineContext, MenuData};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};

fn main() {
    let data = MenuData::load();
    eprintln!("[rsc-ls] language server started, {} menus loaded", data.menus.len());

    let mut server = Server::new(data);
    server.run();
}

// ── Server state ────────────────────────────────────────────────

struct Server {
    data: MenuData,
    docs: HashMap<String, String>, // URI → document text
}

impl Server {
    fn new(data: MenuData) -> Self {
        Server {
            data,
            docs: HashMap::new(),
        }
    }

    fn run(&mut self) {
        let stdin = std::io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        let mut buffer = String::new();
        let mut content_length: usize;

        loop {
            // Read headers
            buffer.clear();
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => return, // EOF
                    Ok(_) => {
                        buffer.push_str(&line);
                        if line == "\r\n" {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("[rsc-ls] read error: {e}");
                        return;
                    }
                }
            }

            // Parse Content-Length
            content_length = 0;
            for line in buffer.lines() {
                if let Some(val) = line.strip_prefix("Content-Length:") {
                    content_length = val.trim().parse().unwrap_or(0);
                }
            }

            if content_length == 0 {
                continue;
            }

            // Read body
            let mut body = vec![0u8; content_length];
            if let Err(e) = reader.read_exact(&mut body) {
                eprintln!("[rsc-ls] read body error: {e}");
                return;
            }

            let msg: serde_json::Value = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[rsc-ls] JSON parse error: {e}");
                    continue;
                }
            };

            let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
            let _id = msg.get("id").cloned();
            eprintln!("[rsc-ls] ← {method}");

            let response = self.handle_message(method, &msg);

            if let Some(resp) = response {
                let json = serde_json::to_string(&resp).unwrap();
                let header = format!("Content-Length: {}\r\n\r\n", json.len());
                let mut stdout = std::io::stdout().lock();
                let _ = stdout.write_all(header.as_bytes());
                let _ = stdout.write_all(json.as_bytes());
                let _ = stdout.flush();
            }
        }
    }

    fn handle_message(
        &mut self,
        method: &str,
        params: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        let id = params.get("id").cloned().unwrap_or(serde_json::Value::Null);

        match method {
            "initialize" => {
                let id = params.get("id").cloned().unwrap_or(serde_json::Value::Null);
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "capabilities": {
                            "textDocumentSync": 1,
                            "completionProvider": {
                                "triggerCharacters": ["/", " ", "="],
                            },
                            "hoverProvider": true,
                        },
                        "serverInfo": {
                            "name": "mikrotik-rsc-ls",
                            "version": "0.1.0",
                        },
                    },
                }))
            }

            "shutdown" => Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": null,
            })),

            "exit" => {
                std::process::exit(0);
            }

            "textDocument/didOpen" => {
                let uri = params["params"]["textDocument"]["uri"].as_str()?;
                let text = params["params"]["textDocument"]["text"].as_str()?;
                self.docs.insert(uri.to_string(), text.to_string());
                None
            }

            "textDocument/didChange" => {
                let uri = params["params"]["textDocument"]["uri"].as_str()?;
                let changes = params["params"]["contentChanges"].as_array()?;
                if let Some(change) = changes.first()
                    && let Some(text) = change["text"].as_str()
                {
                    self.docs.insert(uri.to_string(), text.to_string());
                }
                None
            }

            "textDocument/didClose" => {
                if let Some(uri) = params["params"]["textDocument"]["uri"].as_str() {
                    self.docs.remove(uri);
                }
                None
            }

            "textDocument/completion" => {
                let uri = params["params"]["textDocument"]["uri"].as_str()?;
                let pos = &params["params"]["position"];
                let line = pos["line"].as_u64()?;
                let character = pos["character"].as_u64()?;
                let doc = self.docs.get(uri)?;

                let before_cursor =
                    build_before_cursor(doc, line as usize, character as usize);
                let items = completion::compute_completions(&self.data, &before_cursor);

                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "isIncomplete": false,
                        "items": items,
                    },
                }))
            }

            "textDocument/hover" => {
                let uri = params["params"]["textDocument"]["uri"].as_str()?;
                let pos = &params["params"]["position"];
                let line = pos["line"].as_u64()? as usize;
                let character = pos["character"].as_u64()? as usize;
                let doc = self.docs.get(uri)?;

                let lines: Vec<&str> = doc.lines().collect();
                let current_line = lines.get(line).copied().unwrap_or("");

                let hover = hover::compute_hover(
                    &self.data,
                    current_line,
                    character,
                    doc,
                    line,
                );

                let result = hover.map(|h| serde_json::to_value(h).unwrap());

                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": result,
                }))
            }

            _ => {
                // Unknown method
                if !id.is_null() {
                    Some(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32601,
                            "message": format!("Method not found: {method}"),
                        },
                    }))
                } else {
                    None
                }
            }
        }
    }
}

// ── Tokenizer / parser (ported from ls.mjs) ─────────────────────

/// Split a line into tokens: quoted strings, /-prefixed paths, or bare words.
/// A bare word may contain a quoted value (e.g. `comment="hello world"`), in
/// which case the whole `key="value"` string is returned as a single token.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Skip whitespace
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        let start = i;

        // /-prefixed path segment: take until whitespace
        if bytes[i] == b'/' {
            i += 1;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            tokens.push(std::str::from_utf8(&bytes[start..i]).unwrap_or("").to_string());
            continue;
        }

        // Bare word, but it may contain an embedded quoted value.
        loop {
            if i >= bytes.len() || bytes[i].is_ascii_whitespace() {
                break;
            }

            if bytes[i] == b'"' {
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else if bytes[i] == b'"' {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
                // After the closing quote we may immediately hit whitespace
                // (or another quoted segment); continue the outer loop.
                continue;
            }

            i += 1;
        }

        tokens.push(std::str::from_utf8(&bytes[start..i]).unwrap_or("").to_string());
    }

    tokens
}

/// Build the "before cursor" context across multiple lines.
///
/// RouterOS commands can span multiple lines — properties on subsequent lines
/// are continuations of the same command.  Walks backwards from the cursor
/// line, collecting all lines belonging to the current command.
pub fn build_before_cursor(doc: &str, cursor_line: usize, cursor_char: usize) -> String {
    let lines: Vec<&str> = doc.lines().collect();
    if cursor_line >= lines.len() {
        return String::new();
    }

    let current_part = &lines[cursor_line][..cursor_char.min(lines[cursor_line].len())];
    if current_part.trim().is_empty() {
        return String::new();
    }

    // Trim a trailing line-continuation backslash and any leading spaces on
    // the current line so that continued tokens read as one token.
    let current = current_part
        .trim_end_matches('\\')
        .trim_start_matches(|c: char| c.is_ascii_whitespace());
    let mut parts = vec![current];

    for i in (0..cursor_line).rev() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty() {
            break;
        }
        if trimmed.starts_with('/') || trimmed.starts_with(':') {
            parts.insert(0, lines[i].trim_end_matches('\\').trim());
            break;
        }
        parts.insert(0, lines[i].trim_end_matches('\\').trim());
    }

    parts.join(" ").trim().to_string()
}

/// Parse a line of RouterOS script into structural components.
pub fn parse_line(data: &MenuData, before_cursor: &str) -> LineContext {
    let tokens = tokenize(before_cursor);
    let mut path_parts: Vec<String> = Vec::new();
    let mut command: Option<String> = None;
    let mut properties: HashMap<String, String> = HashMap::new();
    let last_token = tokens.last().cloned().unwrap_or_default();

    for token in &tokens {
        if token.starts_with('/') {
            let part = token.trim_start_matches('/').to_string();
            if !part.is_empty() {
                path_parts.push(part);
            }
            continue;
        }

        if let Some(eq_idx) = token.find('=') {
            let key = token[..eq_idx].to_string();
            let value = token[eq_idx + 1..].to_string();
            properties.insert(key, value);
            continue;
        }

        if !path_parts.is_empty() {
            let current_path = format!("/{}", path_parts.join("/"));
            // Use child_names_by_parent (not menu_by_path) so implicit
            // intermediate menus like /ip/firewall are recognized as valid
            // path segments even though they have no direct TOML entry.
            let child = data
                .child_names_by_parent
                .get(&current_path)
                .and_then(|children| children.iter().find(|c| c.name == *token));

            if let Some(child) = child {
                if child.menu_type == "Command" {
                    // Action command under the current menu (e.g. /ip route check).
                    if command.is_none() {
                        command = Some(token.clone());
                    }
                } else {
                    // Directory / settings directory: extend the path.
                    path_parts.push(token.clone());
                }
            } else if command.is_none() {
                command = Some(token.clone());
            }
            continue;
        }

        if command.is_none() {
            command = Some(token.clone());
        }
    }

    LineContext {
        path: if path_parts.is_empty() {
            String::new()
        } else {
            format!("/{}", path_parts.join("/"))
        },
        command,
        properties,
        last_token,
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menus::{ChildEntry, MenuData, MenuEntry};

    fn make_menus(raw: &[(&str, &str)]) -> (Vec<MenuEntry>, HashMap<String, MenuEntry>) {
        let mut menus = Vec::new();
        let mut menu_by_path = HashMap::new();
        for &(path, typ) in raw {
            let m = MenuEntry {
                path: path.to_string(),
                menu_type: typ.to_string(),
                flags: vec![],
                arguments: vec![],
                read_only: vec![],
            };
            menu_by_path.insert(path.to_string(), m.clone());
            menus.push(m);
        }
        (menus, menu_by_path)
    }

    fn build_data(menus: Vec<MenuEntry>, menu_by_path: HashMap<String, MenuEntry>) -> MenuData {
        let mut child_map: HashMap<String, HashMap<String, ChildEntry>> = HashMap::new();
        for m in &menus {
            let parts: Vec<&str> = m.path.split('/').collect();
            for i in 2..parts.len() {
                let parent = format!("/{}", parts[1..i].join("/"));
                let child = parts[i].to_string();
                let child_path = format!("/{}", parts[1..i + 1].join("/"));
                child_map
                    .entry(parent)
                    .or_default()
                    .entry(child.clone())
                    .or_insert(ChildEntry {
                        name: child,
                        path: child_path,
                        menu_type: m.menu_type.clone(),
                    });
            }
        }
        let mut root_children = HashMap::new();
        for m in &menus {
            if let Some(root) = m.path.split('/').nth(1) {
                root_children
                    .entry(root.to_string())
                    .or_insert_with(|| ChildEntry {
                        name: root.to_string(),
                        path: format!("/{root}"),
                        menu_type: "Directory".to_string(),
                    });
            }
        }
        child_map.insert(String::new(), root_children);

        MenuData {
            menus,
            menu_by_path,
            child_names_by_parent: child_map
                .into_iter()
                .map(|(k, v)| (k, v.into_values().collect()))
                .collect(),
        }
    }

    fn test_data() -> MenuData {
        let (menus, menu_by_path) = make_menus(&[
            ("/ip/address", "Directory"),
            ("/ip/route", "Directory"),
            ("/ip/route/check", "Command"),
        ]);
        build_data(menus, menu_by_path)
    }

    #[test]
    fn tokenize_splits_words_and_paths() {
        assert_eq!(
            tokenize("/ip address add gateway=1.1.1.1"),
            vec!["/ip", "address", "add", "gateway=1.1.1.1"]
        );
    }

    #[test]
    fn tokenize_quoted_string() {
        assert_eq!(
            tokenize("add comment=\"hello world\""),
            vec!["add", "comment=\"hello world\""]
        );
    }

    #[test]
    fn build_before_cursor_collects_continuation_lines() {
        let doc = "/ip address add \\\n  address=10.0.0.1/24 \\\n  interface=ether1";
        // Cursor on the last line, column 7 = "inter" (after two leading spaces).
        let ctx = build_before_cursor(doc, 2, 7);
        assert_eq!(ctx, "/ip address add address=10.0.0.1/24 inter");
    }

    #[test]
    fn parse_line_root_menu() {
        let data = test_data();
        let ctx = parse_line(&data, "/ip address add address=10.0.0.1/24");
        assert_eq!(ctx.path, "/ip/address");
        assert_eq!(ctx.command, Some("add".to_string()));
        assert_eq!(
            ctx.properties.get("address"),
            Some(&"10.0.0.1/24".to_string())
        );
    }

    #[test]
    fn parse_line_partial_submenu() {
        let data = test_data();
        let ctx = parse_line(&data, "/ip route");
        assert_eq!(ctx.path, "/ip/route");
        assert!(ctx.command.is_none());
    }

    #[test]
    fn parse_line_action_command() {
        let data = test_data();
        let ctx = parse_line(&data, "/ip route check");
        assert_eq!(ctx.path, "/ip/route");
        assert_eq!(ctx.command, Some("check".to_string()));
    }

    #[test]
    fn parse_line_no_menu() {
        let data = test_data();
        let ctx = parse_line(&data, ":put $x");
        assert!(ctx.path.is_empty());
        assert_eq!(ctx.command, Some(":put".to_string()));
    }

    #[test]
    fn parse_line_lone_root_prefix() {
        let data = test_data();
        let ctx = parse_line(&data, "/");
        assert!(ctx.path.is_empty(), "lone '/' should map to the root path");
        assert!(ctx.command.is_none());
    }
}
