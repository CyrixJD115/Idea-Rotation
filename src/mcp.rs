//! MCP (Model Context Protocol) server over stdio.
//!
//! Speaks newline-delimited JSON-RPC 2.0 on stdin/stdout, which is the MCP
//! "stdio" transport. Lets AI models / agent platforms discover and drive
//! the generator:
//!
//!   idea-rotation mcp [--config path]
//!
//! Config is re-read from disk on every tool call, so edits made in the
//! GUI (or by hand) are picked up immediately.

use crate::config::Config;
use crate::engine;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

const SERVER_NAME: &str = "idea-rotation";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PROTOCOL: &str = "2024-11-05";
const CONFIG_URI: &str = "idea-rotation://config";

// ---------------------------------------------------------------------------
// Entrypoint (IO)
// ---------------------------------------------------------------------------

/// Run the stdio loop. Returns a process exit code.
pub fn run(config_path: PathBuf) -> u8 {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let mut line = String::new();
    let mut reader = stdin.lock();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return 0, // EOF: client went away
            Ok(_) => {}
            Err(_) => return 1,
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = respond(trimmed, &config_path) {
            let mut encoded = serde_json::to_string(&response).unwrap_or_default();
            encoded.push('\n');
            if out.write_all(encoded.as_bytes()).is_err() {
                return 1;
            }
            let _ = out.flush();
        }
    }
}

// ---------------------------------------------------------------------------
// Protocol core (pure — unit tested)
// ---------------------------------------------------------------------------

/// Handle one raw JSON-RPC message. Returns `Some(response)` for requests
/// (messages carrying an `id`), `None` for notifications.
pub fn respond(raw: &str, config_path: &Path) -> Option<Value> {
    let msg: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            return Some(error_response(Value::Null, -32700, &format!("parse error: {e}")));
        }
    };

    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);

    let result = match method {
        "initialize" => Some(handle_initialize(&params)),
        "ping" => Some(json!({})),
        "tools/list" => Some(handle_tools_list()),
        "tools/call" => Some(handle_tools_call(&params, config_path)),
        "resources/list" => Some(handle_resources_list()),
        "resources/read" => Some(handle_resources_read(&params, config_path)),
        "prompts/list" => Some(json!({ "prompts": [] })),
        // Notifications never get a reply, even unknown ones.
        _ if id.is_none() => None,
        _ => {
            return Some(error_response(
                id.clone().unwrap_or(Value::Null),
                -32601,
                &format!("method not found: {method}"),
            ));
        }
    };

    result.map(|r| {
        json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": r })
    })
}

fn handle_initialize(params: &Value) -> Value {
    // Echo the client's protocol version when given; otherwise advertise
    // our default.
    let version = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_PROTOCOL)
        .to_string();
    json!({
        "protocolVersion": version,
        "capabilities": {
            "tools": { "listChanged": false },
            "resources": { "subscribe": false, "listChanged": false }
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": SERVER_VERSION
        },
        "instructions": "Config-driven idea/name generator. Use list_namespaces to see word pools, generate_ideas to roll combinations, get_config for the raw TOML."
    })
}

fn handle_tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "generate_ideas",
                "description": "Generate seamless names by picking one word from each namespace a naming pattern references. Returns each finished name, the pattern used, and every pick with its description.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "count": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 12,
                            "description": "How many names to generate (default: settings.count from config, usually 3)"
                        },
                        "namespaces": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional: restrict to these namespace ids (e.g. [\"style\",\"format\"]); bypasses enabled flags"
                        },
                        "pattern": {
                            "type": "string",
                            "description": "Optional: a pattern name (e.g. \"Compound\"), a 1-based index, \"auto\" (default — picks randomly among usable patterns), or an ad-hoc template like \"{style} {subject} of the {format}\" with optional transforms .lower/.upper/.title/.cap"
                        },
                        "seed": {
                            "type": "string",
                            "description": "Optional: repeatable results — same seed + same config gives the same names"
                        }
                    }
                }
            },
            {
                "name": "list_namespaces",
                "description": "List every namespace (ids, enabled state, description, all words + descriptions) AND every naming pattern in the config.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "get_config",
                "description": "Return the full config.toml currently on disk (namespaces, options, presets, patterns, settings).",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "add_namespace",
                "description": "Create a new namespace (word pool) in the config file. Returns its generated id. Use add_option to fill it.",
                "inputSchema": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": { "type": "string", "description": "Display name, e.g. \"Material\"" },
                        "description": { "type": "string", "description": "Optional: what this slot adds to a name" },
                        "enabled": { "type": "boolean", "description": "Default true" }
                    }
                }
            },
            {
                "name": "add_option",
                "description": "Add a word (option) to an existing namespace in the config file. `namespace` accepts an id or display name.",
                "inputSchema": {
                    "type": "object",
                    "required": ["namespace", "word"],
                    "properties": {
                        "namespace": { "type": "string", "description": "Namespace id or name" },
                        "word": { "type": "string" },
                        "description": { "type": "string", "description": "What the word evokes" }
                    }
                }
            },
            {
                "name": "set_namespace_enabled",
                "description": "Turn a namespace on or off in the config file. Disabled namespaces are skipped during generation (unless explicitly requested via `namespaces`).",
                "inputSchema": {
                    "type": "object",
                    "required": ["namespace", "enabled"],
                    "properties": {
                        "namespace": { "type": "string", "description": "Namespace id or name" },
                        "enabled": { "type": "boolean" }
                    }
                }
            }
        ]
    })
}

fn handle_resources_list() -> Value {
    json!({
        "resources": [
            {
                "uri": CONFIG_URI,
                "name": "Idea Rotation config",
                "description": "The live config.toml driving the generator",
                "mimeType": "text/x-toml"
            }
        ]
    })
}

fn handle_resources_read(params: &Value, config_path: &Path) -> Value {
    let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
    if uri != CONFIG_URI {
        return json!({ "contents": [] });
    }
    let body = std::fs::read_to_string(config_path)
        .unwrap_or_else(|e| format!("# cannot read {}: {e}", config_path.display()));
    json!({
        "contents": [ { "uri": CONFIG_URI, "mimeType": "text/x-toml", "text": body } ]
    })
}

fn handle_tools_call(params: &Value, config_path: &Path) -> Value {
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let outcome: Result<String, String> = match name {
        "generate_ideas" => call_generate(&args, config_path),
        "list_namespaces" => call_list_namespaces(config_path),
        "get_config" => call_get_config(config_path),
        "add_namespace" => call_add_namespace(&args, config_path),
        "add_option" => call_add_option(&args, config_path),
        "set_namespace_enabled" => call_set_enabled(&args, config_path),
        _ => Err(format!("unknown tool: {name}")),
    };

    match outcome {
        Ok(text) => json!({ "content": [ { "type": "text", "text": text } ] }),
        Err(err) => json!({
            "content": [ { "type": "text", "text": err } ],
            "isError": true
        }),
    }
}

fn load_config(config_path: &Path) -> Result<Config, String> {
    match std::path::Path::new(config_path).exists() {
        true => Config::load(config_path),
        false => Ok(Config::default()),
    }
}

fn call_generate(args: &Value, config_path: &Path) -> Result<String, String> {
    let cfg = load_config(config_path)?;

    let count = match args.get("count") {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(cfg.settings.count as u64) as usize,
        _ => cfg.settings.count,
    };
    let count = count.clamp(1, 12);

    let only: Option<Vec<String>> = args.get("namespaces").and_then(|v| v.as_array()).map(|a| {
        a.iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect()
    });

    let seed = args.get("seed").and_then(|s| s.as_str());
    let pattern = args.get("pattern").and_then(|s| s.as_str());

    let ideas = engine::generate(&cfg, count, only.as_deref(), seed, pattern)?;
    if ideas.is_empty() {
        return Ok("No names generated: every namespace is disabled or empty. \
                   Use list_namespaces / get_config to inspect the config."
            .to_string());
    }

    Ok(engine::render_ideas(&ideas, false))
}

fn call_list_namespaces(config_path: &Path) -> Result<String, String> {
    let cfg = load_config(config_path)?;
    let mut out = String::new();
    for ns in &cfg.namespaces {
        out.push_str(&format!(
            "## {}  (id: {}, {}, {} options)\n",
            ns.name,
            ns.id,
            if ns.enabled { "enabled" } else { "disabled" },
            ns.options.len()
        ));
        if !ns.description.is_empty() {
            out.push_str(&format!("   {}\n", ns.description));
        }
        for opt in &ns.options {
            if opt.description.is_empty() {
                out.push_str(&format!("   - {}\n", opt.word));
            } else {
                out.push_str(&format!("   - {}: {}\n", opt.word, opt.description));
            }
        }
        out.push('\n');
    }
    out.push_str("# Naming patterns\n");
    if cfg.patterns.is_empty() {
        out.push_str(&format!(
            "none — names fall back to “{}”-joined words\n",
            cfg.settings.separator
        ));
    } else {
        for (i, p) in cfg.patterns.iter().enumerate() {
            out.push_str(&format!("{}. {} = {}\n", i + 1, p.name, p.template));
        }
        out.push_str(
            "\nplaceholders are {{namespace_id}} with optional transforms: \
             .lower .upper .title .cap — ad-hoc templates work via the `pattern` arg\n",
        );
    }
    if out.is_empty() {
        out.push_str("Config has no namespaces yet.");
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Write tools (mutate config.toml; the GUI picks changes up via reload)
// ---------------------------------------------------------------------------

/// Load from disk, mutate, sanitize, write back.
fn mutate_config<F>(config_path: &Path, f: F) -> Result<Config, String>
where
    F: FnOnce(&mut Config) -> Result<(), String>,
{
    let mut cfg = load_config(config_path)?;
    f(&mut cfg)?;
    cfg.sanitize();
    cfg.save(config_path)?;
    Ok(cfg)
}

fn find_ns_mut<'a>(
    cfg: &'a mut Config,
    key: &str,
) -> Option<&'a mut crate::config::Namespace> {
    let key_lc = key.trim().to_lowercase();
    cfg.namespaces
        .iter_mut()
        .find(|n| n.id == key_lc || n.name.to_lowercase() == key_lc)
}

fn call_add_namespace(args: &Value, config_path: &Path) -> Result<String, String> {
    let Some(name) = args.get("name").and_then(|v| v.as_str()).map(str::trim) else {
        return Err("add_namespace needs a \"name\" string".into());
    };
    if name.is_empty() {
        return Err("name cannot be empty".into());
    }
    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let enabled = args.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

    let mut created_id = String::new();
    let cfg = mutate_config(config_path, |cfg| {
        let taken: Vec<String> = cfg.namespaces.iter().map(|n| n.id.clone()).collect();
        let mut ns = crate::config::Namespace::new(name, &taken);
        ns.description = description;
        ns.enabled = enabled;
        created_id = ns.id.clone();
        cfg.namespaces.push(ns);
        Ok(())
    })?;
    Ok(format!(
        "namespace “{name}” created with id “{created_id}” — config now has {} namespaces. \
         Add words with add_option.",
        cfg.namespaces.len()
    ))
}

fn call_add_option(args: &Value, config_path: &Path) -> Result<String, String> {
    let Some(namespace) = args.get("namespace").and_then(|v| v.as_str()) else {
        return Err("add_option needs a \"namespace\" (id or name)".into());
    };
    let Some(word) = args
        .get("word")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|w| !w.is_empty())
    else {
        return Err("add_option needs a non-empty \"word\"".into());
    };
    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut count = 0usize;
    mutate_config(config_path, |cfg| {
        let Some(ns) = find_ns_mut(cfg, namespace) else {
            return Err(format!("namespace “{namespace}” not found"));
        };
        if ns.options.iter().any(|o| o.word.eq_ignore_ascii_case(word)) {
            return Err(format!(
                "“{word}” already exists in “{}”",
                ns.name
            ));
        }
        ns.options.push(crate::config::OptionEntry {
            word: word.to_string(),
            description,
        });
        count = ns.options.len();
        Ok(())
    })?;
    Ok(format!("added “{word}” — namespace now has {count} options"))
}

fn call_set_enabled(args: &Value, config_path: &Path) -> Result<String, String> {
    let Some(namespace) = args.get("namespace").and_then(|v| v.as_str()) else {
        return Err("set_namespace_enabled needs a \"namespace\" (id or name)".into());
    };
    let Some(enabled) = args.get("enabled").and_then(|v| v.as_bool()) else {
        return Err("set_namespace_enabled needs an \"enabled\" boolean".into());
    };
    mutate_config(config_path, |cfg| {
        let Some(ns) = find_ns_mut(cfg, namespace) else {
            return Err(format!("namespace “{namespace}” not found"));
        };
        ns.enabled = enabled;
        Ok(())
    })?;
    Ok(format!("namespace “{namespace}” {}", if enabled { "enabled" } else { "disabled" }))
}

fn call_get_config(config_path: &Path) -> Result<String, String> {
    let path = std::path::Path::new(config_path);
    if !path.exists() {
        return Ok(format!(
            "# no config file at {} — built-in default is in use\n{}",
            path.display(),
            toml::to_string_pretty(&Config::default()).map_err(|e| e.to_string())?
        ));
    }
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_config() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("idea-rotation-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        Config::default().save(&path).unwrap();
        path
    }

    fn roundtrip(req: &Value, path: &Path) -> Value {
        let raw = serde_json::to_string(req).unwrap();
        respond(&raw, path).expect("request must produce a response")
    }

    #[test]
    fn initialize_handshake() {
        let path = tmp_config();
        let resp = roundtrip(
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                     "params": { "protocolVersion": "2025-06-18", "capabilities": {} } }),
            &path,
        );
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(resp["result"]["serverInfo"]["name"], "idea-rotation");
    }

    #[test]
    fn notifications_are_silent() {
        let path = tmp_config();
        assert!(respond(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &path
        )
        .is_none());
    }

    #[test]
    fn tools_list_then_generate() {
        let path = tmp_config();
        let list = roundtrip(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }), &path);
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"generate_ideas"));
        assert!(names.contains(&"list_namespaces"));
        assert!(names.contains(&"get_config"));

        let call = roundtrip(
            &json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                     "params": { "name": "generate_ideas",
                                 "arguments": { "count": 2, "seed": "t", "namespaces": ["style"] } } }),
            &path,
        );
        let text = call["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("1."), "text was: {text}");
        assert!(text.contains("[Style]"), "text was: {text}");
    }

    #[test]
    fn unknown_method_is_32601() {
        let path = tmp_config();
        let resp = roundtrip(&json!({ "jsonrpc": "2.0", "id": 9, "method": "bogus" }), &path);
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[test]
    fn resource_read_returns_toml() {
        let path = tmp_config();
        let resp = roundtrip(
            &json!({ "jsonrpc": "2.0", "id": 4, "method": "resources/read",
                     "params": { "uri": CONFIG_URI } }),
            &path,
        );
        let text = resp["result"]["contents"][0]["text"].as_str().unwrap();
        assert!(text.contains("[[namespaces]]"));
        assert!(text.contains("[[patterns]]"));
    }

    #[test]
    fn generate_with_adhoc_pattern() {
        let path = tmp_config();
        let call = roundtrip(
            &json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call",
                     "params": { "name": "generate_ideas",
                                 "arguments": { "count": 1, "seed": "p",
                                                "pattern": "the {subject} of the {style.lower}" } } }),
            &path,
        );
        let text = call["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("1. the "), "text was: {text}");
        assert!(text.contains("via custom"), "text was: {text}");
    }

    #[test]
    fn generate_with_bad_pattern_is_tool_error() {
        let path = tmp_config();
        let call = roundtrip(
            &json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call",
                     "params": { "name": "generate_ideas",
                                 "arguments": { "pattern": "Nope" } } }),
            &path,
        );
        assert_eq!(call["result"]["isError"], true);
        assert!(call["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown pattern"));
    }

    #[test]
    fn write_tools_round_trip() {
        let path = tmp_config();

        let add_ns = roundtrip(
            &json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call",
                     "params": { "name": "add_namespace",
                                 "arguments": { "name": "Material" } } }),
            &path,
        );
        let text = add_ns["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("id “material”"), "text was: {text}");

        let add_opt = roundtrip(
            &json!({ "jsonrpc": "2.0", "id": 8, "method": "tools/call",
                     "params": { "name": "add_option",
                                 "arguments": { "namespace": "Material",
                                                "word": "Obsidian",
                                                "description": "Volcanic glass, sharp edges" } } }),
            &path,
        );
        assert!(add_opt["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Obsidian"));

        let dup = roundtrip(
            &json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call",
                     "params": { "name": "add_option",
                                 "arguments": { "namespace": "material", "word": "obsidian" } } }),
            &path,
        );
        assert_eq!(dup["result"]["isError"], true);

        let off = roundtrip(
            &json!({ "jsonrpc": "2.0", "id": 10, "method": "tools/call",
                     "params": { "name": "set_namespace_enabled",
                                 "arguments": { "namespace": "format", "enabled": false } } }),
            &path,
        );
        assert!(off["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("disabled"));

        // everything landed on disk and re-reads cleanly
        let cfg = Config::load(&path).unwrap();
        assert!(cfg.namespaces.iter().any(|n| n.id == "material"
            && n.options.iter().any(|o| o.word == "Obsidian")));
        assert!(cfg.namespaces.iter().any(|n| n.id == "format" && !n.enabled));
    }
}
