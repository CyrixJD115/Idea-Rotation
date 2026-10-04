//! Configuration model: everything the tool knows (namespaces, options,
//! descriptions, presets, settings) lives in a single TOML file that is
//! loaded at startup and can be rewritten from the GUI.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub namespaces: Vec<Namespace>,
    #[serde(default)]
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// How many ideas one "generate" action produces.
    #[serde(default = "default_count")]
    pub count: usize,
    /// String glued between the picked words, e.g. " + ".
    #[serde(default = "default_separator")]
    pub separator: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            count: default_count(),
            separator: default_separator(),
        }
    }
}

fn default_count() -> usize {
    3
}

fn default_separator() -> String {
    " + ".to_string()
}

/// A "namespace" is one slot of the generated idea (e.g. Style, Subject,
/// Format). Users can add / remove / rename these freely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Namespace {
    /// Stable slug used by presets; survives renames of the display name.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub options: Vec<OptionEntry>,
}

fn default_true() -> bool {
    true
}

/// One word inside a namespace, with a human description of what it evokes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionEntry {
    pub word: String,
    #[serde(default)]
    pub description: String,
}

/// A preset remembers *which namespaces were enabled* so the mix can be
/// recalled with one click.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    /// Namespace ids that are turned on when this preset is applied.
    pub enabled: Vec<String>,
}

/// A naming pattern turns picks into a seamless name. The template uses
/// `{namespace_id}` placeholders (optionally with a transform:
/// `{style.lower}`, `{subject.upper}`, `{word.title}`, `{word.cap}`) plus
/// any literal glue text: `"{style} {subject} of the {format}"`,
/// `"{style}{subject}"` for compounds, `"The {style} {subject}"`, ...
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub name: String,
    pub template: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// "Cyber Punk 2" -> "cyber-punk-2"
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = true; // avoid leading dash
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug.push_str("ns");
    }
    slug
}

/// Returns `base`, or `base-2`, `base-3`, ... until it does not collide.
pub fn unique_id(base: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == base) {
        return base.to_string();
    }
    for n in 2.. {
        let candidate = format!("{base}-{n}");
        if !taken.iter().any(|t| t == &candidate) {
            return candidate;
        }
    }
    unreachable!()
}

impl Namespace {
    pub fn new(name: &str, taken_ids: &[String]) -> Self {
        Namespace {
            id: unique_id(&slugify(name), taken_ids),
            name: name.to_string(),
            description: String::new(),
            enabled: true,
            options: Vec::new(),
        }
    }
}

impl Config {
    /// Fix anything that would break the generator: empty ids, duplicate
    /// ids, out-of-range count, empty separator.
    pub fn sanitize(&mut self) {
        if self.settings.count == 0 {
            self.settings.count = default_count();
        }
        self.settings.count = self.settings.count.clamp(1, 12);
        if self.settings.separator.trim().is_empty() {
            self.settings.separator = default_separator();
        }

        let mut taken: Vec<String> = Vec::new();
        for ns in &mut self.namespaces {
            if ns.id.trim().is_empty() {
                ns.id = slugify(&ns.name);
            }
            ns.id = unique_id(&ns.id, &taken);
            taken.push(ns.id.clone());
            if ns.name.trim().is_empty() {
                ns.name = ns.id.clone();
            }
            ns.options.retain(|o| !o.word.trim().is_empty());
            for o in &mut ns.options {
                o.word = o.word.trim().to_string();
                o.description = o.description.trim().to_string();
            }
        }
        for p in &mut self.presets {
            p.name = p.name.trim().to_string();
        }
        self.presets.retain(|p| !p.name.is_empty());

        for p in &mut self.patterns {
            p.name = p.name.trim().to_string();
            p.template = p.template.trim().to_string();
        }
        self.patterns.retain(|p| !p.name.is_empty() && p.template.contains('{'));
    }

    pub fn load(path: &Path) -> Result<Config, String> {
        let raw = fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let mut cfg: Config =
            toml::from_str(&raw).map_err(|e| format!("parse error in {}: {e}", path.display()))?;
        cfg.sanitize();
        Ok(cfg)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut body = toml::to_string_pretty(self)
            .map_err(|e| format!("cannot serialize config: {e}"))?;
        body.push_str("# -- generated by Idea Rotation; comments are not preserved --\n");
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }
        fs::write(path, body).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }
}

// ---------------------------------------------------------------------------
// Default content (what `init` / a fresh folder ships with)
// ---------------------------------------------------------------------------

fn ns(id: &str, name: &str, description: &str, options: &[(&str, &str)]) -> Namespace {
    Namespace {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        enabled: true,
        options: options
            .iter()
            .map(|(w, d)| OptionEntry {
                word: w.to_string(),
                description: d.to_string(),
            })
            .collect(),
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            settings: Settings::default(),
            namespaces: vec![
                ns(
                    "style",
                    "Style",
                    "The overall aesthetic flavor of the idea.",
                    &[
                        ("Cyber", "Neon, high-tech, chrome-and-rain futurism"),
                        ("Rustic", "Warm wood, countryside craft, slower pace"),
                        ("Minimal", "Stripped to the essentials, lots of air"),
                        ("Brutalist", "Raw concrete, heavy forms, unapologetic"),
                        ("Vintage", "Aged, nostalgic, pre-digital character"),
                        ("Organic", "Curved, living, grown rather than built"),
                        ("Sacred", "Ritual, cathedral calm, hushed reverence"),
                        ("Playful", "Bright, bouncy, toy-box energy"),
                    ],
                ),
                ns(
                    "subject",
                    "Subject",
                    "What the idea is actually about.",
                    &[
                        ("Farm", "Crops, seasons, patient hands in soil"),
                        ("Dungeon", "Depths, danger, torchlight and traps"),
                        ("Bakery", "Flour, ovens, small-hours craft"),
                        ("City", "Streets, districts, people in motion"),
                        ("Archive", "Shelves, catalogs, preserved memory"),
                        ("Garden", "Cultivation, pruning, quiet growth"),
                        ("Fleet", "Ships, routes, logistics at scale"),
                        ("Market", "Stalls, haggling, goods changing hands"),
                    ],
                ),
                ns(
                    "format",
                    "Format",
                    "The shape the final thing takes.",
                    &[
                        ("Sim", "Systems you tend and steer over time"),
                        ("Roguelike", "Runs, permadeath, escalating builds"),
                        ("Manager", "Hire, schedule, balance the books"),
                        ("Almanac", "Seasonal reference, entries, tables"),
                        ("Toolkit", "A box of instruments for a job"),
                        ("Journal", "Dated entries, a life on record"),
                        ("Protocol", "Strict steps, handshakes, procedure"),
                        ("Atlas", "Maps, regions, the lay of the land"),
                    ],
                ),
                ns(
                    "cosmos",
                    "Cosmos",
                    "The cosmic backdrop the world sits in.",
                    &[
                        ("Nebula", "Glowing dust cloud, a nursery of stars"),
                        ("Galaxy", "A hundred billion suns on a slow spin"),
                        ("Void", "The dark between stars, nothing for light-years"),
                        ("Supernova", "A star's last, brightest act"),
                        ("Pulsar", "A lighthouse of radiation sweeping the dark"),
                        ("Black Hole", "Gravity that eats light itself"),
                        ("Star Cluster", "A jewel-box swarm of young suns"),
                        ("Aurora", "Skies that ripple with charged light"),
                    ],
                ),
                ns(
                    "planet",
                    "Planets",
                    "World types to land on, No Man's Sky style.",
                    &[
                        ("Rainforest", "Dense canopy, the constant drum of rain"),
                        ("Lava", "Molten veins, ember storms, basalt plains"),
                        ("Windy", "Gale-scoured plains, wind-carved arches"),
                        ("Frozen", "Kilometer-deep ice under aurora-lit nights"),
                        ("Toxic", "Acid pools, spore forests, rust fog"),
                        ("Irradiated", "Crackling dunes beneath a hostile sun"),
                        ("Scorched", "Heat mirages on glass flats, fire storms"),
                        ("Ocean", "Endless swells over drowned ruins"),
                        ("Exotic", "Where the rules of nature politely bend"),
                        ("Barren", "Dead rock, silent craters, thin dust"),
                    ],
                ),
                ns(
                    "phenomenon",
                    "Phenomenon",
                    "Strange events sweeping the world or sky.",
                    &[
                        ("Solar Flare", "The sun exhales fire"),
                        ("Gravity Storm", "Weight becomes a suggestion"),
                        ("Crystal Rain", "Shards drift down, singing as they land"),
                        ("Eclipse", "A staged, unnatural dusk"),
                        ("Magnetar Pulse", "Every compass on the planet spins"),
                        ("Dimensional Rift", "A tear in the sky with weather of its own"),
                        ("Sentinel Surge", "The watchers all wake at once"),
                        ("Plasma Bloom", "Ionized flowers of light opening at dusk"),
                    ],
                ),
            ],
            presets: vec![
                Preset {
                    name: "Everything".into(),
                    enabled: vec![
                        "style".into(),
                        "subject".into(),
                        "format".into(),
                        "cosmos".into(),
                        "planet".into(),
                        "phenomenon".into(),
                    ],
                },
                Preset {
                    name: "Cosmos".into(),
                    enabled: vec![
                        "cosmos".into(),
                        "planet".into(),
                        "phenomenon".into(),
                    ],
                },
            ],
            patterns: vec![
                Pattern {
                    name: "Clean Pair".into(),
                    template: "{style} {subject}".into(),
                },
                Pattern {
                    name: "Full Triple".into(),
                    template: "{style} {subject} {format}".into(),
                },
                Pattern {
                    name: "Compound".into(),
                    template: "{style}{subject}".into(),
                },
                Pattern {
                    name: "Of The".into(),
                    template: "{subject} of the {format}".into(),
                },
                Pattern {
                    name: "The Something".into(),
                    template: "The {style} {subject}".into(),
                },
                Pattern {
                    name: "Worlds".into(),
                    template: "{planet} {phenomenon}".into(),
                },
                Pattern {
                    name: "Orbit".into(),
                    template: "{planet} of the {cosmos}".into(),
                },
                Pattern {
                    name: "Deep Space".into(),
                    template: "{cosmos} {planet} {phenomenon}".into(),
                },
                Pattern {
                    name: "Survey".into(),
                    template: "the {planet.lower} {phenomenon.lower} survey".into(),
                },
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basics() {
        assert_eq!(slugify("Cyber Punk"), "cyber-punk");
        assert_eq!(slugify("  Odd   Name!! "), "odd-name");
        assert_eq!(slugify("///"), "ns");
    }

    #[test]
    fn unique_ids() {
        assert_eq!(unique_id("a", &[]), "a");
        assert_eq!(unique_id("a", &["a".into()]), "a-2");
        assert_eq!(
            unique_id("a", &["a".into(), "a-2".into()]),
            "a-3"
        );
    }

    #[test]
    fn toml_round_trip() {
        let cfg = Config::default();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn sanitize_fixes_duplicates_and_ranges() {
        let mut cfg = Config {
            settings: Settings {
                count: 99,
                separator: "  ".into(),
            },
            namespaces: vec![
                Namespace {
                    id: "x".into(),
                    name: "X".into(),
                    description: String::new(),
                    enabled: true,
                    options: vec![OptionEntry {
                        word: "  A ".into(),
                        description: " a ".into(),
                    }],
                },
                Namespace {
                    id: "x".into(),
                    name: "X2".into(),
                    description: String::new(),
                    enabled: true,
                    options: vec![],
                },
            ],
            presets: vec![Preset {
                name: "  ".into(),
                enabled: vec![],
            }],
            patterns: vec![
                Pattern { name: "ok".into(), template: " {a} {b} ".into() },
                Pattern { name: "no placeholders".into(), template: "plain".into() },
                Pattern { name: "  ".into(), template: "{a}".into() },
            ],
        };
        cfg.sanitize();
        assert_eq!(cfg.settings.count, 12);
        assert_eq!(cfg.settings.separator, " + ");
        assert!(cfg.namespaces[0].id != cfg.namespaces[1].id);
        assert_eq!(cfg.namespaces[0].options[0].word, "A");
        assert!(cfg.presets.is_empty());
        assert_eq!(cfg.patterns.len(), 1);
        assert_eq!(cfg.patterns[0].template, "{a} {b}");
    }
}
