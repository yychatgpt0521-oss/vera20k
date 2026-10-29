//! Ares per-super paradrop keys on a `Type=ParaDrop`/`Type=AmerParaDrop`
//! super: `ParaDrop.Count=`, and `ParaDrop[.<scope>].Aircraft=`, `.Types=`
//! and `.Num=` for the scopes `<Country>.Plane<N>`, `<Side>.Plane<N>`,
//! `Plane<N>`, `<Country>`, `<Side>` and none.
//!
//! Source: Ares 3.0 documentation, Type=ParaDrop and Type=AmerParaDrop
//! (https://ares-developers.github.io/Ares-docs/new/superweapons/types/paradrop.html):
//! for plane N the first scope in that order that sets a value wins; the
//! aircraft and the contents resolve independently, and `Types` and `Num` are
//! defined together. Ares 3.0 is closed source; this follows the documentation.

use crate::rules::ini_parser::IniSection;
use std::collections::BTreeMap;

/// One scope's values. `types`/`nums` are only meaningful together.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
struct Scope {
    aircraft: Option<String>,
    types: Option<Vec<String>>,
    nums: Option<Vec<i32>>,
}

/// A super's own paradrop configuration. Empty when it authors no
/// `ParaDrop.*` key, in which case the country, side and stock lists apply.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct AresSuperParaDrop {
    /// `ParaDrop.Count=`: planes sent, default 1.
    count: Option<i32>,
    /// Lower-case scope (`""`, `"ussr"`, `"plane2"`, `"nod.plane2"`, ...).
    scopes: BTreeMap<String, Scope>,
}

impl AresSuperParaDrop {
    pub fn from_section(section: &IniSection) -> Self {
        let mut settings = Self::default();
        for key in section.keys() {
            let lower = key.to_ascii_lowercase();
            let Some(rest) = lower.strip_prefix("paradrop.") else {
                continue;
            };
            if rest == "count" {
                settings.count = Some(section.read_ares_int(key, 1));
                continue;
            }
            let (scope, field) = match rest.rsplit_once('.') {
                Some((scope, field)) => (scope.to_owned(), field),
                None => (String::new(), rest),
            };
            match field {
                "aircraft" => {
                    if let Some(name) = section.read_name(key, 0x800) {
                        settings.scopes.entry(scope).or_default().aircraft = Some(name.to_owned());
                    }
                }
                "types" => {
                    let types = section
                        .read_list(key, 0x800)
                        .unwrap_or_default()
                        .into_iter()
                        .map(str::to_owned)
                        .collect();
                    settings.scopes.entry(scope).or_default().types = Some(types);
                }
                "num" => {
                    let nums = section.read_ares_int_list(key).unwrap_or_default();
                    settings.scopes.entry(scope).or_default().nums = Some(nums);
                }
                _ => {}
            }
        }
        settings
    }

    /// Whether the super authors any of its own paradrop keys.
    pub fn is_authored(&self) -> bool {
        self.count.is_some() || !self.scopes.is_empty()
    }

    /// Planes sent: `ParaDrop.Count=`, default 1.
    pub fn count(&self) -> i32 {
        self.count.unwrap_or(1)
    }

    fn lookup<'a, T>(
        &'a self,
        country: Option<&str>,
        side: Option<&str>,
        plane: i32,
        field: impl Fn(&'a Scope) -> Option<T>,
    ) -> Option<T> {
        let plane = format!("plane{plane}");
        let scoped = |prefix: Option<&str>, suffix: Option<&str>| match (prefix, suffix) {
            (Some(prefix), Some(suffix)) => {
                Some(format!("{}.{suffix}", prefix.to_ascii_lowercase()))
            }
            (Some(prefix), None) => Some(prefix.to_ascii_lowercase()),
            (None, Some(suffix)) => Some(suffix.to_owned()),
            (None, None) => None,
        };
        [
            scoped(country, Some(&plane)),
            scoped(side, Some(&plane)),
            Some(plane.clone()),
            scoped(country, None),
            scoped(side, None),
            Some(String::new()),
        ]
        .into_iter()
        .flatten()
        .find_map(|scope| self.scopes.get(&scope).and_then(&field))
    }

    /// The aircraft type plane `plane` (1-based) uses, if any scope sets one.
    pub fn aircraft(&self, country: Option<&str>, side: Option<&str>, plane: i32) -> Option<&str> {
        self.lookup(country, side, plane, |scope| scope.aircraft.as_deref())
    }

    /// Plane `plane`'s types and their counts, if any scope sets the types.
    pub fn contents(
        &self,
        country: Option<&str>,
        side: Option<&str>,
        plane: i32,
    ) -> Option<(&[String], &[i32])> {
        self.lookup(country, side, plane, |scope| {
            let types = scope.types.as_deref()?;
            Some((types, scope.nums.as_deref().unwrap_or(&[])))
        })
    }
}
