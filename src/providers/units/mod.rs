//! Unit conversion provider — converts between common units.
//!
//! Pattern: `<number> <from> in|to <to>` — e.g. `5 km in miles`,
//! `100 c in f`, `2 liters to gallons`. Matches by parsing only, so it never
//! hijacks queries it can't convert.

use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider, SearchResult};

/// Conversion factor and optional offset (temperature) to a dimension's base unit.
#[derive(Clone, Copy)]
struct Unit {
    /// Factor to multiply by to reach the base unit.
    factor: f64,
    /// Offset applied before scaling (used by temperature scales).
    offset: f64,
}

fn unit_table() -> Vec<(&'static str, &'static str, Unit)> {
    use Unit as U;
    vec![
        // Length (base: metre)
        (
            "m",
            "length",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "meter",
            "length",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "metre",
            "length",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "km",
            "length",
            U {
                factor: 1000.0,
                offset: 0.0,
            },
        ),
        (
            "kilometer",
            "length",
            U {
                factor: 1000.0,
                offset: 0.0,
            },
        ),
        (
            "kilometre",
            "length",
            U {
                factor: 1000.0,
                offset: 0.0,
            },
        ),
        (
            "cm",
            "length",
            U {
                factor: 0.01,
                offset: 0.0,
            },
        ),
        (
            "mm",
            "length",
            U {
                factor: 0.001,
                offset: 0.0,
            },
        ),
        (
            "mi",
            "length",
            U {
                factor: 1609.344,
                offset: 0.0,
            },
        ),
        (
            "mile",
            "length",
            U {
                factor: 1609.344,
                offset: 0.0,
            },
        ),
        (
            "yd",
            "length",
            U {
                factor: 0.9144,
                offset: 0.0,
            },
        ),
        (
            "yard",
            "length",
            U {
                factor: 0.9144,
                offset: 0.0,
            },
        ),
        (
            "ft",
            "length",
            U {
                factor: 0.3048,
                offset: 0.0,
            },
        ),
        (
            "foot",
            "length",
            U {
                factor: 0.3048,
                offset: 0.0,
            },
        ),
        (
            "feet",
            "length",
            U {
                factor: 0.3048,
                offset: 0.0,
            },
        ),
        (
            "in",
            "length",
            U {
                factor: 0.0254,
                offset: 0.0,
            },
        ),
        (
            "inch",
            "length",
            U {
                factor: 0.0254,
                offset: 0.0,
            },
        ),
        (
            "inches",
            "length",
            U {
                factor: 0.0254,
                offset: 0.0,
            },
        ),
        // Mass (base: kilogram)
        (
            "kg",
            "mass",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "kilogram",
            "mass",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "g",
            "mass",
            U {
                factor: 0.001,
                offset: 0.0,
            },
        ),
        (
            "gram",
            "mass",
            U {
                factor: 0.001,
                offset: 0.0,
            },
        ),
        (
            "mg",
            "mass",
            U {
                factor: 1e-6,
                offset: 0.0,
            },
        ),
        (
            "lb",
            "mass",
            U {
                factor: 0.45359237,
                offset: 0.0,
            },
        ),
        (
            "pound",
            "mass",
            U {
                factor: 0.45359237,
                offset: 0.0,
            },
        ),
        (
            "lbs",
            "mass",
            U {
                factor: 0.45359237,
                offset: 0.0,
            },
        ),
        (
            "oz",
            "mass",
            U {
                factor: 0.028349523,
                offset: 0.0,
            },
        ),
        (
            "ounce",
            "mass",
            U {
                factor: 0.028349523,
                offset: 0.0,
            },
        ),
        (
            "t",
            "mass",
            U {
                factor: 1000.0,
                offset: 0.0,
            },
        ),
        (
            "tonne",
            "mass",
            U {
                factor: 1000.0,
                offset: 0.0,
            },
        ),
        (
            "ton",
            "mass",
            U {
                factor: 907.18474,
                offset: 0.0,
            },
        ),
        // Volume (base: litre)
        (
            "l",
            "volume",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "liter",
            "volume",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "litre",
            "volume",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "ml",
            "volume",
            U {
                factor: 0.001,
                offset: 0.0,
            },
        ),
        (
            "gal",
            "volume",
            U {
                factor: 3.785411784,
                offset: 0.0,
            },
        ),
        (
            "gallon",
            "volume",
            U {
                factor: 3.785411784,
                offset: 0.0,
            },
        ),
        (
            "qt",
            "volume",
            U {
                factor: 0.946352946,
                offset: 0.0,
            },
        ),
        (
            "quart",
            "volume",
            U {
                factor: 0.946352946,
                offset: 0.0,
            },
        ),
        (
            "pt",
            "volume",
            U {
                factor: 0.473176473,
                offset: 0.0,
            },
        ),
        (
            "pint",
            "volume",
            U {
                factor: 0.473176473,
                offset: 0.0,
            },
        ),
        (
            "cup",
            "volume",
            U {
                factor: 0.2365882365,
                offset: 0.0,
            },
        ),
        (
            "fl oz",
            "volume",
            U {
                factor: 0.02957352956,
                offset: 0.0,
            },
        ),
        (
            "floz",
            "volume",
            U {
                factor: 0.02957352956,
                offset: 0.0,
            },
        ),
        // Speed (base: m/s)
        (
            "m/s",
            "speed",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "kmh",
            "speed",
            U {
                factor: 0.277777778,
                offset: 0.0,
            },
        ),
        (
            "km/h",
            "speed",
            U {
                factor: 0.277777778,
                offset: 0.0,
            },
        ),
        (
            "kph",
            "speed",
            U {
                factor: 0.277777778,
                offset: 0.0,
            },
        ),
        (
            "mph",
            "speed",
            U {
                factor: 0.44704,
                offset: 0.0,
            },
        ),
        (
            "knot",
            "speed",
            U {
                factor: 0.514444444,
                offset: 0.0,
            },
        ),
        (
            "knots",
            "speed",
            U {
                factor: 0.514444444,
                offset: 0.0,
            },
        ),
        // Data (base: byte, powers of 1024 — Windows convention)
        (
            "b",
            "data",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "byte",
            "data",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "kb",
            "data",
            U {
                factor: 1024.0,
                offset: 0.0,
            },
        ),
        (
            "mb",
            "data",
            U {
                factor: 1024.0 * 1024.0,
                offset: 0.0,
            },
        ),
        (
            "gb",
            "data",
            U {
                factor: 1024.0 * 1024.0 * 1024.0,
                offset: 0.0,
            },
        ),
        (
            "tb",
            "data",
            U {
                factor: 1024.0 * 1024.0 * 1024.0 * 1024.0,
                offset: 0.0,
            },
        ),
        (
            "bit",
            "data",
            U {
                factor: 0.125,
                offset: 0.0,
            },
        ),
        (
            "kbit",
            "data",
            U {
                factor: 128.0,
                offset: 0.0,
            },
        ),
        (
            "mbit",
            "data",
            U {
                factor: 128.0 * 1024.0,
                offset: 0.0,
            },
        ),
        // Time (base: second)
        (
            "ms",
            "time",
            U {
                factor: 0.001,
                offset: 0.0,
            },
        ),
        (
            "millisecond",
            "time",
            U {
                factor: 0.001,
                offset: 0.0,
            },
        ),
        (
            "s",
            "time",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "sec",
            "time",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "second",
            "time",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "min",
            "time",
            U {
                factor: 60.0,
                offset: 0.0,
            },
        ),
        (
            "minute",
            "time",
            U {
                factor: 60.0,
                offset: 0.0,
            },
        ),
        (
            "h",
            "time",
            U {
                factor: 3600.0,
                offset: 0.0,
            },
        ),
        (
            "hr",
            "time",
            U {
                factor: 3600.0,
                offset: 0.0,
            },
        ),
        (
            "hour",
            "time",
            U {
                factor: 3600.0,
                offset: 0.0,
            },
        ),
        (
            "day",
            "time",
            U {
                factor: 86400.0,
                offset: 0.0,
            },
        ),
        (
            "week",
            "time",
            U {
                factor: 604800.0,
                offset: 0.0,
            },
        ),
        // Temperature (offset scales — handled specially)
        (
            "c",
            "temp",
            U {
                factor: 1.0,
                offset: 273.15,
            },
        ),
        (
            "celsius",
            "temp",
            U {
                factor: 1.0,
                offset: 273.15,
            },
        ),
        (
            "f",
            "temp",
            U {
                factor: 5.0 / 9.0,
                offset: 459.67,
            },
        ),
        (
            "fahrenheit",
            "temp",
            U {
                factor: 5.0 / 9.0,
                offset: 459.67,
            },
        ),
        (
            "k",
            "temp",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
        (
            "kelvin",
            "temp",
            U {
                factor: 1.0,
                offset: 0.0,
            },
        ),
    ]
}

fn lookup_unit(name: &str) -> Option<(&'static str, Unit)> {
    let n = name.trim().to_ascii_lowercase();
    let found = |n: &str| {
        unit_table()
            .into_iter()
            .find(|(alias, _, _)| n == *alias)
            .map(|(_, dim, unit)| (dim, unit))
    };
    found(&n).or_else(|| n.strip_suffix('s').and_then(found))
}

/// Parse `"5 km in miles"` → `(5.0, "km", "miles")`.
///
/// Tries every `" in "` / `" to "` occurrence — `"5.5 in to cm"` has two
/// candidate splits and only one parses (inches → cm).
fn parse_query(query: &str) -> Option<(f64, &str, &str)> {
    let mut seps: Vec<usize> = query
        .match_indices(" in ")
        .chain(query.match_indices(" to "))
        .map(|(i, _)| i)
        .collect();
    seps.sort_unstable();
    for sep in seps {
        let left = &query[..sep];
        let right = &query[sep + 4..];
        let Some(num_end) = left.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-') else {
            continue;
        };
        let (num_str, from) = left.split_at(num_end);
        let Ok(value) = num_str.trim().parse() else {
            continue;
        };
        let from = from.trim();
        let to = right.trim();
        if !from.is_empty() && !to.is_empty() {
            return Some((value, from, to));
        }
    }
    None
}

fn format_value(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    let abs = v.abs();
    let s = if !(1e-4..1_000_000.0).contains(&abs) {
        format!("{v:.2e}")
    } else {
        format!("{v:.4}")
    };
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

pub struct UnitsProvider;

impl SearchProvider for UnitsProvider {
    fn id(&self) -> &'static str {
        "units"
    }

    fn priority(&self) -> i32 {
        20
    }

    fn should_run(&self, query: &str) -> bool {
        query.chars().any(|c| c.is_ascii_digit())
            && (query.contains(" in ") || query.contains(" to "))
    }

    fn search(&self, _ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let Some((value, from, to)) = parse_query(query) else {
            return Vec::new();
        };
        let Some((from_dim, from_unit)) = lookup_unit(from) else {
            return Vec::new();
        };
        let Some((to_dim, to_unit)) = lookup_unit(to) else {
            return Vec::new();
        };
        if from_dim != to_dim {
            return Vec::new();
        }

        let base = (value + from_unit.offset) * from_unit.factor;
        let result = if from_dim == "temp" {
            base / to_unit.factor - to_unit.offset
        } else {
            base / to_unit.factor
        };
        let result = if result.abs() < 1e-10 { 0.0 } else { result };

        let output = format_value(result);
        vec![SearchResult {
            title: format!("= {output}"),
            subtitle: format!("Units: {value} {from} → {to}"),
            kind: "calc".into(),
            provider_id: "units".into(),
            action: output.clone(),
            icon_rgba: None,
            score: 900.0,
        }]
    }

    fn activate(&self, _ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        arboard::Clipboard::new()
            .and_then(|mut c| c.set_text(&result.action))
            .map_err(|e| ElementError::Other(format!("clipboard error: {:?}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert(q: &str) -> Option<String> {
        let provider = UnitsProvider;
        let config = crate::config::Config::default();
        let db = crate::database::Database::new_in_memory();
        let results = provider.search(
            &SearchContext {
                config: &config,
                db: &db,
            },
            q,
        );
        results.first().map(|r| r.action.clone())
    }

    #[test]
    fn length_conversion() {
        assert_eq!(convert("5 km in miles"), Some("3.1069".into()));
        assert_eq!(convert("1 mi to km"), Some("1.6093".into()));
        assert_eq!(convert("12 in to ft"), Some("1".into()));
    }

    #[test]
    fn temperature_conversion() {
        assert_eq!(convert("100 c in f"), Some("212".into()));
        assert_eq!(convert("32 f to c"), Some("0".into()));
        assert_eq!(convert("0 c to k"), Some("273.15".into()));
    }

    #[test]
    fn mass_and_volume() {
        assert_eq!(convert("1 lb to kg"), Some("0.4536".into()));
        assert_eq!(convert("2 gallons in liters"), Some("7.5708".into()));
        assert_eq!(convert("1 cup to ml"), Some("236.5882".into()));
    }

    #[test]
    fn data_and_time() {
        assert_eq!(convert("1 gb in mb"), Some("1024".into()));
        assert_eq!(convert("90 min in hours"), Some("1.5".into()));
    }

    #[test]
    fn rejects_bad_queries() {
        assert_eq!(convert("hello world"), None);
        assert_eq!(convert("5 km in apples"), None);
        assert_eq!(convert("km in miles"), None);
        assert_eq!(convert("5 in 3"), None);
    }

    #[test]
    fn ambiguous_separators() {
        assert_eq!(convert("5.5 in to cm"), Some("13.97".into()));
        assert_eq!(convert("100 kmh to mph"), Some("62.1371".into()));
    }
}
