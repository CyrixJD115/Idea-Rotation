---
name: idea-rotation
description: Config-driven idea & name generator — synthesizes seamless names ("Godslaying Crow of the Ashen Mire") from user-defined word pools and naming patterns. Use when brainstorming names, project ideas, game assets, or themed combinations from a TOML config. Runnable as GUI, CLI, or MCP server.
version: 1.2.2
---

# Idea Rotation

One binary, three interfaces — all driven by a single `config.toml`:

| Command | What it does |
|---|---|
| `idea-rotation` | launches the GUI (dark, compact, 3 tabs) |
| `idea-rotation generate` | prints names to stdout (scripts, quick rolls) |
| `idea-rotation patterns` | lists the naming patterns in the config |
| `idea-rotation mcp` | runs an MCP server over stdio (for AI agents) |
| `idea-rotation init` | writes a default `config.toml` if none exists |
| `idea-rotation path` | prints which config path is being used |

Config resolution order: `--config PATH` → `$IDEA_ROTATION_CONFIG` →
`config.toml` next to the AppImage (Linux) → `config.toml` beside the
executable → `~/.config/idea-rotation/config.toml`
(or `%APPDATA%\idea-rotation\config.toml` on Windows) → `./config.toml`.
Run `idea-rotation path` to see which one is live; the GUI footer shows it too.

## How naming works

Two layers:

1. **Namespaces** are word pools (e.g. `style`, `subject`, `material`) —
   each option is a word plus a short description of what it evokes.
2. **Patterns** are templates that pick one word from each referenced
   namespace and fuse them into a seamless name.

```toml
[[patterns]]
name = "Of The"
template = "{subject} of the {format}"       # → "Garden of the Atlas"

[[patterns]]
name = "Compound"
template = "{style}{subject}"                # → "CyberBakery"
```

Placeholders are `{namespace_id}` with optional transforms:
`{style.lower}` `→ cyber`, `{style.upper}` `→ CYBER`,
`{word.title}` `→ Title Case`, `{word.cap}` `→ First letter capitalized`.
Any literal glue works: `"the {style} {subject} of {material} {format}"`
→ *"the Playful Garden of Obsidian Journal"*.

With no `pattern` given, one is chosen at random from the usable ones
(patterns whose namespaces are all available). A pattern can also be given
ad hoc without touching the config — see below. When a config defines no
patterns at all, names fall back to `settings.separator`-joined picks.

Same seed + same config + same pattern = same names, everywhere.

## Config format (config.toml)

```toml
[settings]
count = 3               # names per generate (1-12)
separator = " + "       # fallback join when no patterns exist

[[namespaces]]
id = "style"            # stable slug — patterns/presets reference this
name = "Style"          # display name
description = "The aesthetic flavor."
enabled = true

  [[namespaces.options]]
  word = "Cyber"
  description = "Neon, high-tech, chrome futurism"

[[patterns]]
name = "Clean Pair"
template = "{style} {subject}"

[[presets]]
name = "Everything"
enabled = ["style", "subject"]   # namespace ids turned on when applied
```

Edit the file directly, or in the GUI (Namespaces tab) and press **save** —
the GUI rewrites the file (comments are not preserved).

## CLI usage

```sh
idea-rotation generate                                   # auto pattern mix
idea-rotation generate --pattern "Of The"                # by name
idea-rotation generate --pattern 3                       # by 1-based index
idea-rotation generate --pattern "{style} {subject} of the {format}"   # ad hoc
idea-rotation generate --count 5 --seed retro --names-only
idea-rotation generate --namespaces style,format --json
```

`--namespaces` restricts picking to those ids (bypasses enabled flags;
patterns that need missing namespaces are skipped or error if explicit).
`--names-only` prints bare names. `--json` outputs
`[{name, pattern, picks:[{word, namespace, namespace_id, description}]}]`.

## MCP usage (AI agents)

Register as a stdio MCP server (replace the path with wherever the binary
was installed — see [INSTALL.md](INSTALL.md)):

```json
{
  "mcpServers": {
    "idea-rotation": {
      "command": "/absolute/path/to/idea-rotation",
      "args": ["mcp"]
    }
  }
}
```

Read tools:

- **generate_ideas** `{count?, namespaces?, pattern?, seed?}` — roll names;
  `pattern` accepts a name, an index, `"auto"`, or an ad-hoc template.
  Each result lists the finished name, the pattern used, and every pick
  with its word + description.
- **list_namespaces** — every namespace with ids, enabled state, all words
  + descriptions, **and every naming pattern**.
- **get_config** — the raw live `config.toml`.

Write tools (mutate `config.toml` on disk; the GUI picks them up via
**reload**):

- **add_namespace** `{name, description?, enabled?}` — creates a word pool,
  returns its generated id.
- **add_option** `{namespace, word, description?}` — `namespace` accepts an
  id or display name; duplicate words are rejected.
- **set_namespace_enabled** `{namespace, enabled}` — turn a pool on/off.

Plus one resource: `idea-rotation://config` (the live TOML).
The config is re-read from disk on every tool call, so edits apply
immediately without restarting the server.

### Typical agent flow

1. `list_namespaces` → see pools + patterns.
2. `generate_ideas` (optionally with an ad-hoc `pattern` to experiment) →
   get names with the reasoning behind each word.
3. Missing a pool? `add_namespace` + `add_option` to build it, then
   generate again referencing it in an ad-hoc pattern.
4. `get_config` → hand the user the full picture.

## GUI quick tour

- **Generate** — roll names; pin a pattern with the dropdown, seed for
  repeatable results, copy any name to the clipboard.
- **Namespaces** — add/rename/delete word pools, toggle them in/out of the
  mix, edit every word and its description inline.
- **Presets** — save which namespaces are enabled as a named mix; apply it
  later in one click.

## Building from source

Rust project using [Iced](https://iced.rs) 0.13. `cargo build --release`
produces the binary; Noto Sans fonts (OFL) and the window icon are
embedded. Cross-compile for Windows with
`cargo build --release --target x86_64-pc-windows-gnu`.
