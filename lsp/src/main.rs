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
                if let Some(change) = changes.first() {
                    if let Some(text) = change["text"].as_str() {
                        self.docs.insert(uri.to_string(), text.to_string());
                    }
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

        // Quoted string
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i += 2; // skip escaped char
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            tokens.push(std::str::from_utf8(&bytes[start..i]).unwrap_or("").to_string());
            continue;
        }

        // /-prefixed path segment
        if bytes[i] == b'/' {
            let start = i;
            i += 1;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            tokens.push(std::str::from_utf8(&bytes[start..i]).unwrap_or("").to_string());
            continue;
        }

        // Bare word
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
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

    let mut parts = vec![current_part];

    for i in (0..cursor_line).rev() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty() {
            break;
        }
        if trimmed.starts_with('/') || trimmed.starts_with(':') {
            parts.insert(0, lines[i]);
            break;
        }
        parts.insert(0, lines[i]);
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
            path_parts.push(token.trim_start_matches('/').to_string());
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
            let is_sub_menu = data
                .child_names_by_parent
                .get(&current_path)
                .map(|children| children.iter().any(|c| c.name == *token))
                .unwrap_or(false);
            if is_sub_menu {
                path_parts.push(token.clone());
            } else {
                command = Some(token.clone());
            }
            continue;
        }

        command = Some(token.clone());
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

/// Count newlines in a document up to a given byte position (for hover context).
pub fn count_newlines(_doc: &str, _pos: usize) -> usize {
    // Hover uses line-based lookup from the position parameter directly,
    // so we don't need to count newlines here.  The hover handler already
    // receives the line number from the LSP position.
    0
}
