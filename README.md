# Idea Rotation

Config-driven idea & name generator. One Rust binary, three faces:

- **GUI** (`idea-rotation`) — Iced 0.13, dark theme with a blue accent, custom
  window icon. Three tabs: Generate (roll names with a pattern picker),
  Namespaces (edit word pools), Presets (save/apply enabled-mixes). All
  edits are saved back to `config.toml` with the **save** button.
- **CLI** (`idea-rotation generate`) — seeded, pattern-driven (`--pattern` by
  name / index / ad-hoc template), `--names-only`, `--json`; `patterns`
  lists configured patterns.
- **MCP server** (`idea-rotation mcp`) — stdio JSON-RPC for AI agents; read
  tools `generate_ideas` / `list_namespaces` / `get_config` plus write tools
  `add_namespace` / `add_option` / `set_namespace_enabled` so agents can
  build word pools into config.toml.

Everything lives in a single `config.toml`: namespaces (word pools),
**patterns** (name templates like `"{subject} of the {format}"` with
`{id.lower}`-style transforms), presets, settings — no code changes needed
to redefine the generator. `idea-rotation init` writes a fresh default
config.

## Layout

```
src/config.rs   model, TOML load/save, defaults, sanitize
src/engine.rs   seeded generation + rendering
src/mcp.rs      MCP stdio server (pure core + IO loop)
src/gui.rs      Iced GUI
src/theme.rs    dark/blue design system
assets/fonts/   embedded Noto Sans (OFL)
release/Idea-Rotation/   deployable: binary + config.toml + SKILL.md
```

## Commands

```sh
cargo run                        # GUI
cargo test                       # 14 unit tests
cargo build --release            # binary → target/release/idea-rotation
```

Deployable folder is rebuilt by copying `target/release/idea-rotation` into
`release/Idea-Rotation/` alongside `config.toml` and `SKILL.md`.
