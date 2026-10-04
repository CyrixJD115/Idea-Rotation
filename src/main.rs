//! Idea Rotation — config-driven idea & name generator.
//!
//! One binary, three faces:
//!   idea-rotation            launch the GUI (dark, compact, blue)
//!   idea-rotation mcp        run an MCP server over stdio for AI agents
//!   idea-rotation generate   roll ideas from the terminal
//!   idea-rotation init       write a fresh default config.toml
//!   idea-rotation path       print the config path being used
//!
//! Config is found via --config PATH, $IDEA_ROTATION_CONFIG, a config.toml
//! next to the executable, or ./config.toml — in that order.

mod config;
mod engine;
mod gui;
mod mcp;
mod theme;

use config::Config;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
idea-rotation — config-driven idea & name generator

USAGE:
    idea-rotation [COMMAND] [--config PATH]

COMMANDS:
    (none)                     launch the GUI
    mcp                        MCP server on stdio (for AI agents)
    generate [FLAGS]           print ideas to stdout
        --count N              how many ideas (1-12)
        --seed S               repeatable results
        --namespaces a,b,c     restrict to these namespace ids
        --pattern P            naming pattern: name, 1-based index, \"auto\",
                               or an ad-hoc template like \"{a} {b} of the {c}\"
        --names-only           print just the names, no descriptions
        --json                 machine-readable output
    patterns                   list the naming patterns in the config
    init                       write a default config.toml (no clobber)
    path                       print the config path in use
    help                       this text

CONFIG:  --config PATH | $IDEA_ROTATION_CONFIG | next to the AppImage / exe |
         ~/.config/idea-rotation/ (or %APPDATA%/idea-rotation/) | ./config.toml
";

fn config_path(explicit: Option<PathBuf>) -> PathBuf {
    // 1. explicit --config wins
    if let Some(p) = explicit {
        return p;
    }
    // 2. environment override
    if let Ok(p) = std::env::var("IDEA_ROTATION_CONFIG") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    // 3. portable: config.toml next to the AppImage file itself
    //    (the mount point beside the binary is read-only)
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        if let Some(p) = PathBuf::from(&appimage).parent().map(|d| d.join("config.toml")) {
            if p.exists() {
                return p;
            }
        }
    }
    // 4. portable: config.toml beside the executable (release bundles)
    let beside_exe = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("config.toml")));
    if let Some(p) = beside_exe {
        if p.exists() {
            return p;
        }
    }
    // 5./6. per-user config home (binary installed on PATH); fresh installs
    // write there instead of littering whatever cwd they were launched from
    if let Some(home) = user_config_path() {
        return home;
    }
    PathBuf::from("config.toml")
}

/// ~/.config/idea-rotation/config.toml (Linux/macOS) or
/// %APPDATA%\idea-rotation\config.toml (Windows). Creates the directory.
fn user_config_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("APPDATA").ok().map(|d| {
            let p = PathBuf::from(d).join("idea-rotation");
            let _ = std::fs::create_dir_all(&p);
            p.join("config.toml")
        })
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))?;
        let p = base.join("idea-rotation");
        let _ = std::fs::create_dir_all(&p);
        Some(p.join("config.toml"))
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut command: Option<&str> = None;
    let mut command_args: Vec<String> = Vec::new();
    let mut explicit_config: Option<PathBuf> = None;

    const COMMANDS: [&str; 8] =
        ["gui", "mcp", "generate", "patterns", "init", "path", "help", "--help"];

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "--config" || arg == "-c" {
            i += 1;
            match args.get(i) {
                Some(p) => explicit_config = Some(PathBuf::from(p)),
                None => {
                    eprintln!("--config needs a path");
                    return ExitCode::from(2);
                }
            }
        } else if arg == "-h" {
            command = Some("help");
        } else if command.is_none() && COMMANDS.contains(&arg) {
            command = Some(arg);
        } else if command.is_some() {
            command_args.push(arg.to_string());
        } else {
            eprintln!("unknown command: {arg}\n\n{USAGE}");
            return ExitCode::from(2);
        }
        i += 1;
    }

    let path = config_path(explicit_config);

    match command {
        None | Some("gui") => {
            if let Err(e) = gui::run(path) {
                eprintln!("GUI error: {e}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Some("mcp") => ExitCode::from(mcp::run(path)),
        Some("init") => {
            if path.exists() {
                println!("already exists: {}", path.display());
                return ExitCode::SUCCESS;
            }
            match Config::default().save(&path) {
                Ok(()) => {
                    println!("wrote {}", path.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("path") => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Some("generate") => cli_generate(&command_args, &path),
        Some("patterns") => {
            match Config::load(&path) {
                Ok(cfg) => {
                    if cfg.patterns.is_empty() {
                        println!("no patterns configured — names fall back to “{}” joined words", cfg.settings.separator);
                    } else {
                        for (i, p) in cfg.patterns.iter().enumerate() {
                            println!("{}. {:<16} {}", i + 1, p.name, p.template);
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("help") | Some("--help") | Some("-h") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("unknown command: {other}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn cli_generate(args: &[String], path: &std::path::Path) -> ExitCode {
    let mut count: Option<usize> = None;
    let mut seed: Option<String> = None;
    let mut namespaces: Option<Vec<String>> = None;
    let mut pattern: Option<String> = None;
    let mut json = false;
    let mut names_only = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--count" | "-n" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse().ok()) {
                    Some(n) => count = Some(n),
                    None => {
                        eprintln!("--count needs a number 1-12");
                        return ExitCode::from(2);
                    }
                }
            }
            "--seed" | "-s" => {
                i += 1;
                match args.get(i) {
                    Some(s) => seed = Some(s.clone()),
                    None => {
                        eprintln!("--seed needs a value");
                        return ExitCode::from(2);
                    }
                }
            }
            "--namespaces" => {
                i += 1;
                match args.get(i) {
                    Some(list) => {
                        namespaces = Some(
                            list.split(',').map(|s| s.trim().to_lowercase()).collect(),
                        )
                    }
                    None => {
                        eprintln!("--namespaces needs a comma list");
                        return ExitCode::from(2);
                    }
                }
            }
            "--json" => json = true,
            "--names-only" => names_only = true,
            "--pattern" | "-p" => {
                i += 1;
                match args.get(i) {
                    Some(p) => pattern = Some(p.clone()),
                    None => {
                        eprintln!("--pattern needs a value (name, index, or template)");
                        return ExitCode::from(2);
                    }
                }
            }
            "generate" => {}
            other => {
                eprintln!("unknown flag for generate: {other}");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }

    let cfg = match Config::load(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let count = count.unwrap_or(cfg.settings.count).clamp(1, 12);

    let ideas = match engine::generate(
        &cfg,
        count,
        namespaces.as_deref(),
        seed.as_deref(),
        pattern.as_deref(),
    ) {
        Ok(ideas) => ideas,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if ideas.is_empty() {
        eprintln!("no ideas: every namespace is disabled or empty");
        return ExitCode::FAILURE;
    }

    if json {
        let payload: Vec<serde_json::Value> = ideas
            .iter()
            .map(|idea| {
                serde_json::json!({
                    "name": idea.name,
                    "pattern": idea.pattern,
                    "picks": idea.picks.iter().map(|p| serde_json::json!({
                        "word": p.word,
                        "namespace": p.namespace,
                        "namespace_id": p.namespace_id,
                        "description": p.description,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_default()
        );
    } else {
        print!("{}", engine::render_ideas(&ideas, names_only));
    }
    ExitCode::SUCCESS
}
