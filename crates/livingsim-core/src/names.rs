//! Procedural names from syllables (`data/names`). Never real people: names are assembled
//! from invented syllables and filtered by a blacklist.

use crate::rng::{in_range, keyed};
use livingsim_content::{NamesDef, SyllableSet};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Index into a [`NamePool`].
pub type NameRef = u32;
pub const NO_NAME: NameRef = u32::MAX;

/// Append-only, de-duplicated string arena (one per region). Agents store `NameRef`s,
/// never `String`s.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NamePool {
    data: String,
    spans: Vec<(u32, u16)>,
    #[serde(skip)]
    index: BTreeMap<String, u32>,
}

impl NamePool {
    pub fn intern(&mut self, s: &str) -> NameRef {
        if self.index.is_empty() && !self.spans.is_empty() {
            self.rebuild_index();
        }
        if let Some(i) = self.index.get(s) {
            return *i;
        }
        let id = self.spans.len() as u32;
        self.spans.push((self.data.len() as u32, s.len() as u16));
        self.data.push_str(s);
        self.index.insert(s.to_string(), id);
        id
    }
    pub fn get(&self, r: NameRef) -> &str {
        match self.spans.get(r as usize) {
            Some((o, l)) => &self.data[*o as usize..*o as usize + *l as usize],
            None => "",
        }
    }
    fn rebuild_index(&mut self) {
        for i in 0..self.spans.len() {
            let s = self.get(i as u32).to_string();
            self.index.insert(s, i as u32);
        }
    }
    pub fn len(&self) -> usize {
        self.spans.len()
    }
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
}

fn capitalise(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut c = lower.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn assemble(set: &SyllableSet, h: u64) -> String {
    let n = in_range(h, set.min_syllables as i64, set.max_syllables as i64) as usize;
    let mut s = String::new();
    let pick = |v: &Vec<String>, k: u64| -> String {
        v[(keyed(h, k, 0, 0) % v.len() as u64) as usize].clone()
    };
    s.push_str(&pick(&set.first, 1));
    for i in 0..n.saturating_sub(2) {
        if !set.middle.is_empty() {
            s.push_str(&pick(&set.middle, 10 + i as u64));
        }
    }
    s.push_str(&pick(&set.last, 2));
    // Avoid triple letters ("Annna").
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if i >= 2 && chars[i - 1].eq_ignore_ascii_case(c) && chars[i - 2].eq_ignore_ascii_case(c) {
            continue;
        }
        out.push(*c);
    }
    capitalise(&out)
}

fn allowed(names: &NamesDef, s: &str) -> bool {
    let l = s.to_lowercase();
    !names
        .blacklist
        .iter()
        .any(|b| l == *b || (b.len() >= 4 && l.contains(b.as_str())))
}

/// Generates a name from a syllable set; deterministic in `h`.
pub fn generate(names: &NamesDef, set: &SyllableSet, h: u64) -> String {
    for attempt in 0..16u64 {
        let s = assemble(set, keyed(h, attempt, 0x4e41_4d45, 0));
        if allowed(names, &s) && s.chars().count() >= 3 {
            return s;
        }
    }
    "Nameless".to_string()
}

/// Generates a given name for a baby, biased towards the family: with some probability it
/// reuses the first syllable of a relative's name (Phase 3 uses this for births).
pub fn generate_for_baby(
    names: &NamesDef,
    set: &SyllableSet,
    relatives: &[&str],
    h: u64,
) -> String {
    if !relatives.is_empty() && keyed(h, 0xBABE, 0, 0) % 100 < 35 {
        let rel = relatives[(keyed(h, 0xBABE, 1, 0) % relatives.len() as u64) as usize];
        if let Some(prefix) = set.first.iter().find(|p| rel.starts_with(p.as_str())) {
            let mut biased = set.clone();
            biased.first = vec![prefix.clone()];
            return generate(names, &biased, h);
        }
        if keyed(h, 0xBABE, 2, 0) % 100 < 15 && allowed(names, rel) {
            // Named after a relative.
            return rel.to_string();
        }
    }
    generate(names, set, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::content;

    #[test]
    fn names_are_deterministic_and_clean() {
        let c = content();
        let style = &c.names.styles[0];
        let a: Vec<String> = (0..200)
            .map(|i| generate(&c.names, &style.female, i))
            .collect();
        let b: Vec<String> = (0..200)
            .map(|i| generate(&c.names, &style.female, i))
            .collect();
        assert_eq!(a, b);
        for n in &a {
            assert!(allowed(&c.names, n), "{n}");
            assert!(n.chars().next().unwrap().is_uppercase());
        }
        let distinct: std::collections::BTreeSet<_> = a.iter().collect();
        assert!(distinct.len() > 100, "names should be varied");
    }

    #[test]
    fn pool_dedupes() {
        let mut p = NamePool::default();
        let a = p.intern("Ana");
        let b = p.intern("Bo");
        assert_eq!(p.intern("Ana"), a);
        assert_eq!(p.get(b), "Bo");
        let bytes = bincode::serialize(&p).unwrap();
        let mut q: NamePool = bincode::deserialize(&bytes).unwrap();
        assert_eq!(q.intern("Bo"), b);
        assert_eq!(q.intern("New"), 2);
    }
}
