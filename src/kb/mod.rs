mod builtin;
pub mod update;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::model::{Category, Proto, ProtoSet, ServiceEntry, Tier};
use crate::paths;

#[derive(Debug, Deserialize)]
pub struct Envelope {
    pub schema: u32,
    #[serde(default)]
    pub source: Option<String>,
    pub entries: Vec<RawEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawEntry {
    pub port: u16,
    #[serde(default)]
    pub protos: Vec<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub tier: Option<String>,
}

pub struct Kb {
    entries: Vec<ServiceEntry>,
    by_port: HashMap<(u16, Proto), Vec<usize>>,
}

impl Kb {
    pub fn load() -> Result<(Self, Vec<String>)> {
        let iana = parse_envelope(builtin::IANA, "built-in IANA data")?;
        let curated = parse_envelope(builtin::CURATED, "built-in curated data")?;

        let mut warnings = Vec::new();
        let cache = read_optional(paths::cache_file(), "cache", &mut warnings);
        let user = read_optional(paths::overrides_file(), "overrides", &mut warnings);

        let kb = Self::from_envelopes(
            &iana,
            &curated,
            cache.as_ref(),
            user.as_ref(),
            &mut warnings,
        );
        Ok((kb, warnings))
    }

    fn from_envelopes(
        iana: &Envelope,
        curated: &Envelope,
        cache: Option<&Envelope>,
        user: Option<&Envelope>,
        warnings: &mut Vec<String>,
    ) -> Self {
        let mut entries: HashMap<(u16, String), ServiceEntry> = HashMap::new();

        for raw in &iana.entries {
            if let Some(entry) = convert(raw.clone(), Tier::Iana, Some(Tier::Iana), warnings) {
                entries.insert(key(&entry), entry);
            }
        }

        for raw in &curated.entries {
            if let Some(entry) = convert(raw.clone(), Tier::Curated, Some(Tier::Curated), warnings)
            {
                merge_entry(&mut entries, entry);
            }
        }

        if let Some(cache) = cache {
            for raw in &cache.entries {
                if let Some(mut entry) = convert(raw.clone(), Tier::Curated, None, warnings) {
                    if entry.tier == Tier::User {
                        warnings.push(format!(
                            "cache entry on port {} has tier 'user'; treating it as curated",
                            entry.port
                        ));
                        entry.tier = Tier::Curated;
                    }
                    merge_entry(&mut entries, entry);
                }
            }
        }

        if let Some(user) = user {
            for raw in &user.entries {
                if let Some(entry) = convert(raw.clone(), Tier::User, Some(Tier::User), warnings) {
                    merge_entry(&mut entries, entry);
                }
            }
        }

        let mut entries: Vec<ServiceEntry> = entries.into_values().collect();
        entries.sort_by_key(|entry| (entry.port, entry.name.to_lowercase()));

        let by_port = build_index(&entries);
        Self { entries, by_port }
    }

    pub fn hints_for(&self, port: u16, proto: Proto) -> Vec<ServiceEntry> {
        self.by_port
            .get(&(port, proto))
            .map(|indices| {
                indices
                    .iter()
                    .map(|&index| self.entries[index].clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn lookup(&self, port: u16, proto: Option<Proto>) -> Vec<&ServiceEntry> {
        let mut matches: Vec<&ServiceEntry> = self
            .entries
            .iter()
            .filter(|entry| {
                entry.port == port
                    && proto.is_none_or(|proto| entry.protos.contains(ProtoSet::from_proto(proto)))
            })
            .collect();
        matches.sort_by_key(|entry| (entry.tier.rank(), entry.name.to_lowercase()));
        matches
    }

    pub fn search(&self, term: &str, limit: usize) -> (Vec<&ServiceEntry>, usize) {
        let needle = term.trim().to_lowercase();
        if needle.is_empty() {
            return (Vec::new(), 0);
        }
        let port_query = needle.parse::<u16>().ok();

        let mut scored: Vec<(u8, &ServiceEntry)> = Vec::new();
        for entry in &self.entries {
            let name = entry.name.to_lowercase();
            let aliases: Vec<String> = entry.aliases.iter().map(|a| a.to_lowercase()).collect();
            let description = entry.description.to_lowercase();

            let score = if name == needle {
                0
            } else if name.starts_with(&needle) {
                1
            } else if port_query == Some(entry.port) {
                2
            } else if aliases.iter().any(|alias| alias == &needle) {
                3
            } else if aliases.iter().any(|alias| alias.starts_with(&needle)) {
                4
            } else if name.contains(&needle)
                || aliases.iter().any(|alias| alias.contains(&needle))
                || description.contains(&needle)
            {
                5
            } else {
                continue;
            };
            scored.push((score, entry));
        }

        scored.sort_by_key(|(score, entry)| {
            (
                *score,
                entry.tier.rank(),
                entry.port,
                entry.name.to_lowercase(),
            )
        });
        let total = scored.len();
        if limit > 0 {
            scored.truncate(limit);
        }
        (scored.into_iter().map(|(_, entry)| entry).collect(), total)
    }

    pub fn browse(
        &self,
        category: Option<Category>,
        include_iana: bool,
        offset: usize,
        limit: usize,
    ) -> (Vec<&ServiceEntry>, usize) {
        let filtered: Vec<&ServiceEntry> = self
            .entries
            .iter()
            .filter(|entry| {
                (include_iana || entry.tier != Tier::Iana)
                    && category.is_none_or(|category| entry.category == Some(category))
            })
            .collect();

        let total = filtered.len();
        let start = offset.min(total);
        let end = if limit == 0 {
            total
        } else {
            start.saturating_add(limit).min(total)
        };
        (filtered[start..end].to_vec(), total)
    }
}

pub fn parse_envelope(text: &str, label: &str) -> Result<Envelope> {
    let envelope: Envelope =
        serde_json::from_str(text).map_err(|err| Error::Data(format!("{label}: {err}")))?;
    if envelope.schema != 1 {
        return Err(Error::Schema(envelope.schema));
    }
    Ok(envelope)
}

pub fn validate_envelope(envelope: &Envelope) -> Result<()> {
    let mut warnings = Vec::new();
    for raw in &envelope.entries {
        if convert(raw.clone(), Tier::Curated, None, &mut warnings).is_none() {
            let detail = warnings
                .last()
                .cloned()
                .unwrap_or_else(|| format!("invalid entry on port {}", raw.port));
            return Err(Error::Data(detail));
        }
    }
    Ok(())
}

fn read_optional(
    path: Option<PathBuf>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Option<Envelope> {
    let path = path?;
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => {
            warnings.push(format!("ignoring {label} {}: {err}", path.display()));
            return None;
        }
    };
    match parse_envelope(&text, label) {
        Ok(envelope) => Some(envelope),
        Err(err) => {
            warnings.push(format!("ignoring {label} {}: {err}", path.display()));
            None
        }
    }
}

fn key(entry: &ServiceEntry) -> (u16, String) {
    (entry.port, entry.name.to_lowercase())
}

fn merge_entry(entries: &mut HashMap<(u16, String), ServiceEntry>, overlay: ServiceEntry) {
    match entries.get_mut(&key(&overlay)) {
        Some(base) => merge_into(base, overlay),
        None => {
            entries.insert(key(&overlay), overlay);
        }
    }
}

fn merge_into(base: &mut ServiceEntry, overlay: ServiceEntry) {
    base.protos |= overlay.protos;

    for alias in overlay.aliases {
        push_alias(base, alias);
    }

    if !overlay.description.is_empty() {
        base.description = overlay.description;
    }
    if overlay.category.is_some() {
        base.category = overlay.category;
    }
    if overlay.homepage.is_some() {
        base.homepage = overlay.homepage;
    }
    if overlay.notes.is_some() {
        base.notes = overlay.notes;
    }
    if overlay.tier.rank() < base.tier.rank() {
        base.tier = overlay.tier;
    }

    let old_name = std::mem::replace(&mut base.name, overlay.name);
    push_alias(base, old_name);
}

fn push_alias(entry: &mut ServiceEntry, alias: String) {
    let alias = alias.trim().to_string();
    if alias.is_empty() || alias.chars().count() > 64 {
        return;
    }
    if entry.name.eq_ignore_ascii_case(&alias)
        || entry
            .aliases
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&alias))
        || entry.aliases.len() >= 16
    {
        return;
    }
    entry.aliases.push(alias);
}

fn convert(
    raw: RawEntry,
    default_tier: Tier,
    force_tier: Option<Tier>,
    warnings: &mut Vec<String>,
) -> Option<ServiceEntry> {
    if raw.port == 0 {
        warnings.push("ignoring entry with port 0".to_string());
        return None;
    }

    let Some(protos) = ProtoSet::from_raw(&raw.protos) else {
        warnings.push(format!(
            "ignoring entry on port {}: invalid or empty protos",
            raw.port
        ));
        return None;
    };

    let name = raw.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        warnings.push(format!("ignoring entry on port {}: invalid name", raw.port));
        return None;
    }

    let mut aliases = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(name.to_lowercase());
    for alias in raw.aliases {
        let alias = alias.trim().to_string();
        if alias.is_empty() || alias.chars().count() > 64 || alias.chars().any(char::is_control) {
            continue;
        }
        if seen.insert(alias.to_lowercase()) {
            aliases.push(alias);
        }
        if aliases.len() >= 16 {
            break;
        }
    }

    let description = raw.description.trim().to_string();
    if description.chars().count() > 512 {
        warnings.push(format!(
            "ignoring entry on port {}: description is too long",
            raw.port
        ));
        return None;
    }

    let category = raw.category.as_deref().and_then(|value| {
        let parsed = Category::parse(value);
        if parsed.is_none() {
            warnings.push(format!(
                "unknown category '{value}' on port {}; leaving it uncategorized",
                raw.port
            ));
        }
        parsed
    });

    let homepage = raw
        .homepage
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| {
            if value.starts_with("http://") || value.starts_with("https://") {
                Some(value.to_string())
            } else {
                warnings.push(format!(
                    "ignoring invalid homepage '{value}' on port {}",
                    raw.port
                ));
                None
            }
        });

    let notes = raw
        .notes
        .map(|notes| notes.trim().to_string())
        .filter(|notes| !notes.is_empty())
        .map(|notes| notes.chars().take(512).collect::<String>());

    let tier = force_tier
        .or_else(|| {
            raw.tier.as_deref().and_then(|value| {
                let parsed = Tier::parse(value);
                if parsed.is_none() {
                    warnings.push(format!(
                        "unknown tier '{value}' on port {}; using default",
                        raw.port
                    ));
                }
                parsed
            })
        })
        .unwrap_or(default_tier);

    Some(ServiceEntry {
        port: raw.port,
        protos,
        name,
        aliases,
        description,
        category,
        homepage,
        notes,
        tier,
    })
}

fn build_index(entries: &[ServiceEntry]) -> HashMap<(u16, Proto), Vec<usize>> {
    let mut index: HashMap<(u16, Proto), Vec<usize>> = HashMap::new();
    for (position, entry) in entries.iter().enumerate() {
        if entry.protos.contains(ProtoSet::TCP) {
            index
                .entry((entry.port, Proto::Tcp))
                .or_default()
                .push(position);
        }
        if entry.protos.contains(ProtoSet::UDP) {
            index
                .entry((entry.port, Proto::Udp))
                .or_default()
                .push(position);
        }
    }
    for indices in index.values_mut() {
        indices.sort_by_key(|&position| {
            (
                entries[position].tier.rank(),
                entries[position].name.to_lowercase(),
            )
        });
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(entries: &str) -> Envelope {
        parse_envelope(&format!(r#"{{"schema":1,"entries":[{entries}]}}"#), "test").unwrap()
    }

    fn empty_envelope() -> Envelope {
        envelope("")
    }

    fn build(
        iana: &Envelope,
        curated: &Envelope,
        cache: Option<&Envelope>,
        user: Option<&Envelope>,
    ) -> Kb {
        let mut warnings = Vec::new();
        let kb = Kb::from_envelopes(iana, curated, cache, user, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        kb
    }

    #[test]
    fn curated_overlay_wins_and_preserves_aliases() {
        let iana = envelope(
            r#"{"port":5432,"protos":["tcp","udp"],"name":"postgresql","description":"PostgreSQL Database"}"#,
        );
        let curated = envelope(
            r#"{"port":5432,"protos":["tcp"],"name":"postgresql","aliases":["postgres"],"description":"PostgreSQL","category":"database"}"#,
        );
        let kb = build(&iana, &curated, None, None);
        let matches = kb.lookup(5432, Some(Proto::Tcp));
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].tier, Tier::Curated);
        assert_eq!(matches[0].category, Some(Category::Database));
        assert!(matches[0].protos.contains(ProtoSet::UDP));
        assert!(matches[0].aliases.iter().any(|alias| alias == "postgres"));
    }

    #[test]
    fn cache_overlays_builtin_and_user_augments() {
        let iana = envelope(
            r#"{"port":9000,"protos":["tcp"],"name":"svc","aliases":["legacy"],"description":"base"}"#,
        );
        let cache =
            envelope(r#"{"port":9000,"protos":["tcp"],"name":"svc","description":"cached"}"#);
        let user = envelope(r#"{"port":9000,"protos":["tcp"],"name":"svc","notes":"mine"}"#);
        let kb = build(&iana, &empty_envelope(), Some(&cache), Some(&user));
        let hints = kb.hints_for(9000, Proto::Tcp);
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].description, "cached");
        assert_eq!(hints[0].notes.as_deref(), Some("mine"));
        assert_eq!(hints[0].tier, Tier::User);
        assert!(hints[0].aliases.iter().any(|alias| alias == "legacy"));
    }

    #[test]
    fn distinct_curated_entry_is_ranked_before_iana() {
        let iana =
            envelope(r#"{"port":8000,"protos":["tcp"],"name":"irdmi","description":"iRDMI"}"#);
        let curated =
            envelope(r#"{"port":8000,"protos":["tcp"],"name":"dev-http","description":"dev"}"#);
        let kb = build(&iana, &curated, None, None);
        let hints = kb.hints_for(8000, Proto::Tcp);
        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0].name, "dev-http");
        assert_eq!(hints[0].tier, Tier::Curated);
        assert_eq!(hints[1].tier, Tier::Iana);
    }

    #[test]
    fn search_ranks_names_before_descriptions() {
        let iana = envelope(
            r#"{"port":6379,"protos":["tcp"],"name":"redis","description":"An advanced key-value cache"},
               {"port":6380,"protos":["tcp"],"name":"alt","description":"redis-compatible"}"#,
        );
        let kb = build(&iana, &empty_envelope(), None, None);
        let (matches, total) = kb.search("redis", 10);
        assert_eq!(total, 2);
        assert_eq!(matches[0].name, "redis");
    }

    #[test]
    fn browse_hides_iana_by_default() {
        let iana = envelope(r#"{"port":80,"protos":["tcp"],"name":"http","description":"web"}"#);
        let curated = envelope(
            r#"{"port":5432,"protos":["tcp"],"name":"postgresql","description":"db","category":"database"}"#,
        );
        let kb = build(&iana, &curated, None, None);
        let (entries, total) = kb.browse(None, false, 0, 50);
        assert_eq!(total, 1);
        assert_eq!(entries[0].port, 5432);
        let (all, total) = kb.browse(None, true, 0, 50);
        assert_eq!(total, 2);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn builtin_data_parses_without_warnings() {
        let iana = parse_envelope(builtin::IANA, "iana").unwrap();
        let curated = parse_envelope(builtin::CURATED, "curated").unwrap();
        let mut warnings = Vec::new();
        let kb = Kb::from_envelopes(&iana, &curated, None, None, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(kb.entries.len() > 6000);

        let hints = kb.hints_for(8000, Proto::Tcp);
        assert_eq!(hints[0].tier, Tier::Curated);
        assert!(hints.iter().any(|hint| hint.tier == Tier::Iana));

        let postgres = kb.lookup(5432, Some(Proto::Tcp));
        assert_eq!(postgres.len(), 1);
        assert_eq!(postgres[0].category, Some(Category::Database));
        assert!(!kb.hints_for(5173, Proto::Tcp).is_empty());
    }

    #[test]
    fn update_validation_rejects_invalid_entries() {
        let port_zero = envelope(r#"{"port":0,"protos":["tcp"],"name":"x","description":""}"#);
        assert!(validate_envelope(&port_zero).is_err());

        let bad_proto = envelope(r#"{"port":1234,"protos":["sctp"],"name":"x","description":""}"#);
        assert!(validate_envelope(&bad_proto).is_err());

        let valid = envelope(r#"{"port":1234,"protos":["tcp"],"name":"x","description":""}"#);
        assert!(validate_envelope(&valid).is_ok());
    }
}
