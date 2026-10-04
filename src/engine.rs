//! The rotation logic: pick one word from each namespace referenced by a
//! naming pattern and synthesize a seamless name ("Godslaying Crow of the
//! Ashen Mire" rather than "God + Crow + Mire").

use crate::config::{Config, Pattern};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    pub namespace_id: String,
    pub namespace: String,
    pub word: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Idea {
    /// The finished, seamless name (pattern-rendered or separator-joined).
    pub name: String,
    /// Which pattern produced it, when one did.
    pub pattern: Option<String>,
    pub picks: Vec<Pick>,
}

// ---------------------------------------------------------------------------
// Template parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    Lower,
    Upper,
    Title,
    Cap,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Placeholder {
    pub id: String,
    pub transform: Option<Transform>,
}

/// Extract `{id}` / `{id.lower}` / `{id.upper}` / `{id.title}` / `{id.cap}`
/// placeholders from a template, in order of appearance.
pub fn parse_template(template: &str) -> Result<Vec<Placeholder>, String> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close_rel) = after.find('}') else {
            return Err(format!("unclosed '{{' in template “{template}”"));
        };
        let token = &after[..close_rel];
        let (id, transform) = match token.split_once('.') {
            None => (token.to_string(), None),
            Some((id, t)) => {
                let transform = match t {
                    "lower" => Transform::Lower,
                    "upper" => Transform::Upper,
                    "title" => Transform::Title,
                    "cap" => Transform::Cap,
                    other => {
                        return Err(format!(
                            "unknown transform “.{other}” in template “{template}” \
                             (use .lower / .upper / .title / .cap)"
                        ));
                    }
                };
                (id.to_string(), Some(transform))
            }
        };
        if id.trim().is_empty() {
            return Err(format!("empty placeholder in template “{template}”"));
        }
        out.push(Placeholder { id, transform });
        rest = &after[close_rel + 1..];
    }
    Ok(out)
}

fn apply_transform(word: &str, transform: Option<Transform>) -> String {
    match transform {
        None => word.to_string(),
        Some(Transform::Lower) => word.to_lowercase(),
        Some(Transform::Upper) => word.to_uppercase(),
        Some(Transform::Title) => word
            .split_whitespace()
            .map(|w| {
                let mut cs = w.chars();
                match cs.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        Some(Transform::Cap) => {
            let mut cs = word.chars();
            match cs.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
                None => String::new(),
            }
        }
    }
}

/// Render a template with the given id → word mapping.
fn render_template(template: &str, words: &[(String, String)]) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close_rel) = after.find('}') else {
            return Err(format!("unclosed '{{' in “{template}”"));
        };
        let token = &after[..close_rel];
        let (id, transform) = match token.split_once('.') {
            None => (token, None),
            Some((id, t)) => (
                id,
                Some(match t {
                    "lower" => Transform::Lower,
                    "upper" => Transform::Upper,
                    "title" => Transform::Title,
                    _ => Transform::Cap,
                }),
            ),
        };
        let Some((_, word)) = words.iter().find(|(wid, _)| wid == id) else {
            return Err(format!("no word picked for {{{id}}}"));
        };
        out.push_str(&apply_transform(word, transform));
        rest = &after[close_rel + 1..];
    }
    out.push_str(rest);
    // collapse accidental double spaces from glue text
    let squashed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(squashed)
}

// ---------------------------------------------------------------------------
// Pattern resolution
// ---------------------------------------------------------------------------

/// Patterns usable against this pool. `pool_ids` = namespace ids that can
/// actually be picked right now.
fn usable_patterns<'a>(patterns: &'a [Pattern], pool_ids: &[String]) -> Vec<&'a Pattern> {
    patterns
        .iter()
        .filter(|p| {
            parse_template(&p.template)
                .map(|ph| {
                    !ph.is_empty()
                        && ph.iter().all(|h| pool_ids.iter().any(|id| id == &h.id))
                })
                .unwrap_or(false)
        })
        .collect()
}

/// Interpret a user-supplied pattern choice: a pattern name (case
/// insensitive), a 1-based index, or an ad-hoc template (contains `{`).
fn choose_pattern<'a>(
    patterns: &'a [Pattern],
    choice: Option<&str>,
    pool_ids: &[String],
) -> Result<Option<Pattern>, String> {
    let Some(choice) = choice
        .map(str::trim)
        .filter(|c| !c.is_empty() || c.contains('{'))
    else {
        return Ok(None); // no choice / empty -> auto
    };
    if choice.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let chosen: Option<Pattern> = if choice.contains('{') {
        Some(Pattern { name: "custom".into(), template: choice.into() })
    } else if let Some(found) = patterns
        .iter()
        .find(|p| p.name.to_lowercase() == choice.to_lowercase())
    {
        Some(found.clone())
    } else if let Ok(idx) = choice.parse::<usize>() {
        if idx >= 1 && idx <= patterns.len() {
            Some(patterns[idx - 1].clone())
        } else {
            None
        }
    } else {
        None
    };
    let Some(pattern) = chosen else {
        return Err(format!(
            "unknown pattern “{choice}” — use a name, a 1-based index, \
             or an ad-hoc template like \"{{{{style}}}} {{{{subject}}}} of the {{{{format}}}}\""
        ));
    };
    let placeholders = parse_template(&pattern.template)?;
    if placeholders.is_empty() {
        return Err(format!("pattern “{}” has no placeholders", pattern.name));
    }
    let missing: Vec<&str> = placeholders
        .iter()
        .filter(|h| !pool_ids.iter().any(|id| id == &h.id))
        .map(|h| h.id.as_str())
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "pattern “{}” needs namespace(s) not available right now: {}",
            pattern.name,
            missing.join(", ")
        ));
    }
    Ok(Some(pattern))
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// Deterministically map a seed string onto a u64.
fn seed_u64(seed: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    hasher.finish()
}

/// Generate `count` ideas.
///
/// * `only` — restrict picking to these namespace ids (bypasses `enabled`).
/// * `seed` — reproducible results for the same config + inputs.
/// * `pattern` — `None`/`"auto"` = pick randomly among usable patterns;
///   otherwise a pattern name, a 1-based index, or an ad-hoc template
///   containing `{...}` placeholders. When no patterns are configured at
///   all, names fall back to `settings.separator`-joined picks.
pub fn generate(
    config: &Config,
    count: usize,
    only: Option<&[String]>,
    seed: Option<&str>,
    pattern: Option<&str>,
) -> Result<Vec<Idea>, String> {
    let pool: Vec<&crate::config::Namespace> = config
        .namespaces
        .iter()
        .filter(|n| !n.options.is_empty())
        .filter(|n| match only {
            Some(ids) => ids.iter().any(|id| id == &n.id),
            None => n.enabled,
        })
        .collect();

    if pool.is_empty() || count == 0 {
        return Ok(Vec::new());
    }
    let pool_ids: Vec<String> = pool.iter().map(|n| n.id.clone()).collect();

    let fixed_pattern = choose_pattern(&config.patterns, pattern, &pool_ids)?;
    let candidates: Vec<Pattern> = match &fixed_pattern {
        Some(p) => vec![p.clone()],
        None => usable_patterns(&config.patterns, &pool_ids)
            .into_iter()
            .cloned()
            .collect(),
    };
    // No patterns usable -> legacy separator join of every pool namespace.
    let pattern_mode = !candidates.is_empty();

    let mut rng = match seed {
        Some(s) if !s.is_empty() => StdRng::seed_from_u64(seed_u64(s)),
        _ => StdRng::from_entropy(),
    };

    let space: usize = if pattern_mode {
        candidates
            .iter()
            .map(|p| {
                parse_template(&p.template)
                    .map(|ph| {
                        ph.iter().map(|h| {
                            pool.iter()
                                .find(|n| n.id == h.id)
                                .map(|n| n.options.len())
                                .unwrap_or(1)
                        })
                        .product::<usize>()
                    })
                    .unwrap_or(1)
            })
            .sum()
    } else {
        pool.iter().map(|n| n.options.len()).product()
    };

    let mut ideas: Vec<Idea> = Vec::with_capacity(count);
    let mut seen: Vec<String> = Vec::new();
    while ideas.len() < count {
        let mut attempts = 0;
        let idea = loop {
            let candidate = roll_once(config, &pool, &candidates, pattern_mode, &mut rng)?;
            attempts += 1;
            if !seen.contains(&candidate.name) || attempts >= 64 || space <= ideas.len() + 1 {
                break candidate;
            }
        };
        seen.push(idea.name.clone());
        ideas.push(idea);
    }
    Ok(ideas)
}

fn roll_once(
    config: &Config,
    pool: &[&crate::config::Namespace],
    candidates: &[Pattern],
    pattern_mode: bool,
    rng: &mut StdRng,
) -> Result<Idea, String> {
    if !pattern_mode {
        let picks = pool
            .iter()
            .map(|n| pick_word(n, rng))
            .collect::<Vec<_>>();
        let name = picks
            .iter()
            .map(|p| p.word.as_str())
            .collect::<Vec<_>>()
            .join(&config.settings.separator);
        return Ok(Idea { name, pattern: None, picks });
    }

    let pattern = candidates
        .choose(rng)
        .expect("pattern_mode implies non-empty candidates");
    let placeholders = parse_template(&pattern.template)?;

    // Pick each referenced namespace once, in template order.
    let mut picks: Vec<Pick> = Vec::new();
    let mut words: Vec<(String, String)> = Vec::new();
    for h in &placeholders {
        if words.iter().any(|(id, _)| id == &h.id) {
            continue; // repeated placeholder reuses the same pick
        }
        let n = pool
            .iter()
            .find(|n| n.id == h.id)
            .ok_or_else(|| format!("namespace {} vanished", h.id))?;
        let pick = pick_word(n, rng);
        words.push((h.id.clone(), pick.word.clone()));
        picks.push(pick);
    }

    let name = render_template(&pattern.template, &words)?;
    Ok(Idea {
        name,
        pattern: Some(pattern.name.clone()),
        picks,
    })
}

fn pick_word(n: &crate::config::Namespace, rng: &mut StdRng) -> Pick {
    let opt = n.options.choose(rng).expect("non-empty options");
    Pick {
        namespace_id: n.id.clone(),
        namespace: n.name.clone(),
        word: opt.word.clone(),
        description: opt.description.clone(),
    }
}

/// Human-readable multi-line rendering of ideas (shared by CLI and MCP).
pub fn render_ideas(ideas: &[Idea], names_only: bool) -> String {
    let mut out = String::new();
    for (i, idea) in ideas.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", i + 1, idea.name));
        if names_only {
            continue;
        }
        if let Some(p) = &idea.pattern {
            out.push_str(&format!("   via {p}\n"));
        }
        for pick in &idea.picks {
            if pick.description.is_empty() {
                out.push_str(&format!("   - {} [{}]\n", pick.word, pick.namespace));
            } else {
                out.push_str(&format!(
                    "   - {} [{}] — {}\n",
                    pick.word, pick.namespace, pick.description
                ));
            }
        }
        if i + 1 < ideas.len() {
            out.push('\n');
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Namespace, OptionEntry};

    fn toy_config() -> Config {
        Config {
            settings: Default::default(),
            namespaces: vec![
                Namespace {
                    id: "a".into(),
                    name: "A".into(),
                    description: String::new(),
                    enabled: true,
                    options: vec![
                        OptionEntry { word: "a1".into(), description: String::new() },
                        OptionEntry { word: "a2".into(), description: String::new() },
                    ],
                },
                Namespace {
                    id: "b".into(),
                    name: "B".into(),
                    description: String::new(),
                    enabled: true,
                    options: vec![OptionEntry {
                        word: "Bee".into(),
                        description: String::new(),
                    }],
                },
                Namespace {
                    id: "c".into(),
                    name: "C".into(),
                    description: String::new(),
                    enabled: false,
                    options: vec![OptionEntry {
                        word: "Cee".into(),
                        description: String::new(),
                    }],
                },
            ],
            presets: vec![],
            patterns: vec![
                Pattern { name: "Pair".into(), template: "{a} {b}".into() },
                Pattern { name: "Compound".into(), template: "{a}{b.lower}".into() },
                Pattern { name: "Broken".into(), template: "{a} {zzz}".into() },
            ],
        }
    }

    #[test]
    fn parse_template_forms() {
        let ph = parse_template("the {a} of {b.title}").unwrap();
        assert_eq!(ph[0].id, "a");
        assert!(ph[0].transform.is_none());
        assert_eq!(ph[1].id, "b");
        assert_eq!(ph[1].transform, Some(Transform::Title));
        assert!(parse_template("{a").is_err());
        assert!(parse_template("{a.widen}").is_err());
        assert!(parse_template("{}").is_err());
    }

    #[test]
    fn transforms_work() {
        assert_eq!(apply_transform("God", Some(Transform::Lower)), "god");
        assert_eq!(apply_transform("god", Some(Transform::Upper)), "GOD");
        assert_eq!(apply_transform("god slaying", Some(Transform::Title)), "God Slaying");
        assert_eq!(apply_transform("godSLAYING", Some(Transform::Cap)), "Godslaying");
    }

    #[test]
    fn pattern_mode_synthesizes_names() {
        let cfg = toy_config();
        let ideas = generate(&cfg, 4, None, Some("s"), Some("Pair")).unwrap();
        assert_eq!(ideas.len(), 4);
        for idea in &ideas {
            assert_eq!(idea.pattern.as_deref(), Some("Pair"));
            assert!(
                idea.name == "a1 Bee" || idea.name == "a2 Bee",
                "got {}",
                idea.name
            );
            assert_eq!(idea.picks.len(), 2);
        }
    }

    #[test]
    fn compound_pattern_lowercases_second_word() {
        let cfg = toy_config();
        let ideas = generate(&cfg, 1, None, Some("s"), Some("Compound")).unwrap();
        assert!(
            ideas[0].name == "a1bee" || ideas[0].name == "a2bee",
            "got {}",
            ideas[0].name
        );
    }

    #[test]
    fn adhoc_template_with_literals() {
        let cfg = toy_config();
        let ideas = generate(&cfg, 1, None, Some("s"), Some("the {b} of {a.upper}")).unwrap();
        assert!(
            ideas[0].name == "the Bee of A1" || ideas[0].name == "the Bee of A2",
            "got {}",
            ideas[0].name
        );
        assert_eq!(ideas[0].pattern.as_deref(), Some("custom"));
    }

    #[test]
    fn auto_mode_skips_unusable_patterns() {
        let cfg = toy_config();
        // "Broken" references zzz which doesn't exist -> never chosen.
        let ideas = generate(&cfg, 10, None, Some("s"), None).unwrap();
        for idea in &ideas {
            assert!(idea.pattern.as_deref() == Some("Pair") || idea.pattern.as_deref() == Some("Compound"));
        }
    }

    #[test]
    fn only_filter_scopes_patterns() {
        let cfg = toy_config();
        // {b} not in only -> Pair/Compound unusable -> falls back to join.
        let ideas = generate(&cfg, 1, Some(&["a".to_string()]), Some("s"), None).unwrap();
        assert_eq!(ideas[0].pattern, None);
        assert!(ideas[0].name == "a1" || ideas[0].name == "a2");
    }

    #[test]
    fn only_filter_can_use_disabled_namespaces() {
        let cfg = toy_config();
        let ideas = generate(&cfg, 1, Some(&["c".to_string()]), Some("s"), None).unwrap();
        assert_eq!(ideas[0].name, "Cee");
    }

    #[test]
    fn explicit_pattern_requiring_missing_namespace_errors() {
        let cfg = toy_config();
        assert!(generate(&cfg, 1, None, None, Some("Broken")).is_err());
        assert!(generate(&cfg, 1, None, None, Some("Nope")).is_err());
    }

    #[test]
    fn pattern_index_selection() {
        let cfg = toy_config();
        let ideas = generate(&cfg, 1, None, Some("s"), Some("2")).unwrap();
        assert_eq!(ideas[0].pattern.as_deref(), Some("Compound"));
    }

    #[test]
    fn no_patterns_falls_back_to_separator() {
        let mut cfg = toy_config();
        cfg.patterns.clear();
        let ideas = generate(&cfg, 2, None, Some("s"), None).unwrap();
        for idea in &ideas {
            assert_eq!(idea.pattern, None);
            assert!(idea.name == "a1 + Bee" || idea.name == "a2 + Bee");
        }
    }

    #[test]
    fn seeded_runs_are_reproducible() {
        let cfg = toy_config();
        let x = generate(&cfg, 5, None, Some("seed-1"), None).unwrap();
        let y = generate(&cfg, 5, None, Some("seed-1"), None).unwrap();
        let z = generate(&cfg, 5, None, Some("seed-2"), None).unwrap();
        assert_eq!(x, y);
        assert_ne!(x, z);
    }

    #[test]
    fn respects_enabled_flag() {
        let cfg = toy_config();
        let ideas = generate(&cfg, 4, None, Some("s"), None).unwrap();
        for idea in &ideas {
            assert_eq!(idea.picks.len(), 2, "only a and b are enabled");
        }
    }

    #[test]
    fn exhaustive_space_still_fills_count() {
        let cfg = toy_config();
        // 2 a-words × 1 b-word combos; ask for 5 -> duplicates must appear.
        let ideas = generate(&cfg, 5, None, Some("x"), Some("Pair")).unwrap();
        assert_eq!(ideas.len(), 5);
    }
}
