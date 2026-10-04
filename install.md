# Installing Idea Rotation

This guide is written for **AI agents and humans** to follow verbatim. It
installs the binary, creates the config, verifies the install, and
registers the MCP server. If anything fails, see [Troubleshooting](#troubleshooting).

Prebuilt binaries live at
<https://github.com/CyrixJD115/Idea-Rotation/releases/latest>.

| OS | Asset | Contains |
|---|---|---|
| Windows x64 | `Idea-Rotation-windows-x64.zip` | `idea-rotation.exe`, `config.toml`, `SKILL.md`, `icon.png` |
| Linux x86-64 | `Idea-Rotation-linux-x86_64.tar.gz` | `idea-rotation`, `config.toml`, `SKILL.md`, `icon.png` |
| Linux x86-64 | `Idea-Rotation-linux-x86_64.AppImage` | self-contained GUI app (config lives next to the AppImage) |
| macOS Apple Silicon | `Idea-Rotation-macos-arm64.zip` | `Idea Rotation.app`, `config.toml`, `SKILL.md` |
| macOS Intel | `Idea-Rotation-macos-x64.zip` | same, x86_64 build |

Agents: prefer the `.zip` / `.tar.gz` archives — no FUSE, no Gatekeeper
dialogs. The `.AppImage` is aimed at humans who want double-click-to-open.

## 1. Download & extract

Resolve the latest download URL for this OS (agent-friendly, no guessing):

```sh
curl -s https://api.github.com/repos/CyrixJD115/Idea-Rotation/releases/latest \
| python3 -c "import json,sys;[print(a['browser_download_url']) for a in json.load(sys.stdin)['assets']]"
```

Pick the matching asset from the list, then:

**Linux:**

```sh
mkdir -p ~/.local/opt/idea-rotation && cd ~/.local/opt/idea-rotation
curl -LO <linux-x86_64.tar.gz-url>
tar -xzf Idea-Rotation-linux-x86_64.tar.gz --strip-components=1
mkdir -p ~/.local/bin && ln -sf ~/.local/opt/idea-rotation/idea-rotation ~/.local/bin/idea-rotation
```

**macOS:**

```sh
mkdir -p ~/Applications/IdeaRotation && cd ~/Applications/IdeaRotation
curl -LO <macos-zip-url>
unzip -o Idea-Rotation-macos-*.zip
# CLI access to the binary inside the .app bundle:
mkdir -p ~/.local/bin
ln -sf ~/Applications/IdeaRotation/"Idea Rotation.app"/Contents/MacOS/idea-rotation ~/.local/bin/idea-rotation
# First launch (unsigned binary — right-click → Open, or:)
xattr -dr com.apple.quarantine ~/Applications/IdeaRotation/"Idea Rotation.app"
```

**Windows (PowerShell):**

```powershell
$dir = "$env:LOCALAPPDATA\IdeaRotation"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
cd $dir
Invoke-WebRequest <windows-x64.zip-url> -OutFile idea-rotation.zip
Expand-Archive idea-rotation.zip -DestinationPath . -Force
Add-Content $PROFILE '`n$env:Path += ";' + $dir + '"'
```

(Or just extract anywhere and use the full path.)

## 2. Initialize the config

```sh
idea-rotation init        # writes config.toml (skipped if one exists)
idea-rotation path        # shows which config is live
```

Config lives beside the extracted binary (portable). If the binary is on
`PATH` without a nearby config, it uses `~/.config/idea-rotation/config.toml`
(Linux/macOS) or `%APPDATA%\idea-rotation\config.toml` (Windows).

## 3. Verify

```sh
idea-rotation generate --count 2
idea-rotation patterns
```

Both must print names/patterns and exit 0. To check the GUI (needs a
desktop session): run `idea-rotation` with no arguments.

## 4. Register the MCP server

Add to any MCP client config (Claude Desktop, ZCode, etc. — adjust the
client's own config file location):

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

Windows: `"command": "C:\\Users\\<you>\\AppData\\Local\\IdeaRotation\\idea-rotation.exe"`.

Smoke-test the server directly:

```sh
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}' \
| idea-rotation mcp
```

A JSON line containing `"serverInfo"` means it works.

## 5. Learn the tool

Read **SKILL.md** (shipped in every archive, also at the repo root) — it
documents the config format, naming patterns with transforms, CLI flags,
all MCP tools (including write tools that edit the config), and the GUI.

## Troubleshooting

| Symptom | Fix |
|---|---|
| `idea-rotation: command not found` | `~/.local/bin` not on PATH — `export PATH="$HOME/.local/bin:$PATH"` in your shell rc |
| AppImage won't run | Needs FUSE (`sudo apt install libfuse2`), or run extracted: `./Idea-Rotation-*.AppImage --appimage-extract-and-run` |
| AppImage config edits don't stick | Config inside the mount is read-only — keep `config.toml` **next to the .AppImage file** (auto-detected) |
| Linux GUI fails to open (no display) | The GUI needs a desktop session; CLI (`generate`, `mcp`) works headless |
| Linux GUI opens then crashes | GPU drivers without Vulkan — install mesa-vulkan-drivers; CLI is unaffected |
| macOS "unidentified developer" | `xattr -dr com.apple.quarantine "Idea Rotation.app"` (releases are unsigned) |
| MCP client can't connect | Use an **absolute** path in the `command` field; verify with the smoke-test above |
| Config not where expected | `idea-rotation path` prints the exact file in use |

## Building from source instead

Rust ≥ 1.77: `cargo build --release` → binary at `target/release/idea-rotation`.
Windows cross-compile from Linux: `cargo build --release --target x86_64-pc-windows-gnu`.
