//! Squared norm and odd gcd of integer numerators in each named-by-index bin.
use super::{gcd, Outcome};
use pcpos_core::value::Value;
use std::fmt::Write;

const MAX_BINS: usize = 64;
const MAX_ENTRIES: usize = 4096;

#[derive(Debug, PartialEq)]
struct Bin {
    squared_norm: u128,
    odd_gcd: u128,
}

struct Column {
    bins: Vec<Bin>,
    total: u128,
}

impl Column {
    fn read(input: &Value, name: &str) -> Result<Self, String> {
        let Some(Value::List(rows)) = input.get(name) else {
            return Err(format!("{name} must be an ordered list of bins"));
        };
        if !(1..=MAX_BINS).contains(&rows.len()) {
            return Err(format!("{name} must contain 1 to {MAX_BINS} bins"));
        }
        let mut bins = Vec::with_capacity(rows.len());
        let mut entries = 0;
        let mut total = 0_u128;
        for (index, row) in rows.iter().enumerate() {
            let Value::List(values) = row else {
                return Err(format!("{name} bin {index} must be a list"));
            };
            if values.len() > MAX_ENTRIES - entries {
                return Err(format!("{name} exceeds {MAX_ENTRIES} entries"));
            }
            entries += values.len();
            let mut squared_norm = 0_u128;
            let mut divisor = 0_i128;
            for (cell, value) in values.iter().enumerate() {
                let Some(n) = value.as_int() else {
                    return Err(format!("{name} bin {index}, entry {cell} must be a signed 64-bit JSON integer"));
                };
                // The magnitude of an i64 fits u64, even for i64::MIN. Its square fits u128.
                let magnitude = u128::from(n.unsigned_abs());
                squared_norm = squared_norm.checked_add(magnitude * magnitude)
                    .ok_or_else(|| format!("{name} bin {index}: squared norm exceeds u128"))?;
                divisor = gcd(divisor, i128::from(n)) as i128;
            }
            // Zero and empty bins have gcd zero. The nonzero gcd cannot exceed 2^63.
            let odd_gcd = if divisor == 0 { 0 } else { (divisor >> divisor.trailing_zeros()) as u128 };
            total = total.checked_add(squared_norm)
                .ok_or_else(|| format!("{name}: total squared norm exceeds u128"))?;
            bins.push(Bin { squared_norm, odd_gcd });
        }
        Ok(Self { bins, total })
    }
}

pub(super) fn compare(input: &Value) -> Result<Outcome, String> {
    let Value::Map(fields) = input else {
        return Err("input must be an object".into());
    };
    if fields.keys().any(|k| !matches!(k.as_str(), "scale_exponent" | "source" | "target")) {
        return Err("input accepts only scale_exponent, source and target".into());
    }
    let Some(scale) = input.get("scale_exponent").and_then(Value::as_int).filter(|n| (0..=127).contains(n)) else {
        return Err("scale_exponent must be a JSON integer from 0 through 127".into());
    };
    let source = Column::read(input, "source")?;
    let target = Column::read(input, "target")?;
    if source.bins.len() != target.bins.len() {
        return Err("source and target must name the same number of bins in the same order".into());
    }
    let required = 1_u128 << scale;
    let normalized = source.total == required && target.total == required;
    let holds = normalized && source.bins == target.bins;
    let mut text = format!(
        "Required squared total: {required}. Source: {}. Target: {}.\n",
        source.total, target.total
    );
    for (index, (a, b)) in source.bins.iter().zip(&target.bins).enumerate() {
        writeln!(text, "Bin {index}: squared norms {} / {}; odd gcds {} / {}.",
                 a.squared_norm, b.squared_norm, a.odd_gcd, b.odd_gcd).unwrap();
    }
    text.push_str(if holds {
        "Both columns have the requested normalization and all protected-bin invariants agree."
    } else {
        "The requested normalization or the protected-bin invariants differ."
    });
    Ok(Outcome { holds, text })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> Result<Outcome, String> {
        compare(&Value::parse(input.as_bytes()).unwrap())
    }

    #[test]
    fn four_tick_position_and_separately_protected_rail() {
        let whole = run(r#"{"scale_exponent":4,"source":[[3,1],[-1,1,1,1,1,1],[]],"target":[[1,1,1,1,1,1,1,1,1,1],[1,1,1,1,1,1],[0,0]]}"#).unwrap();
        assert!(whole.holds);
        assert!(whole.text.contains("Bin 0: squared norms 10 / 10; odd gcds 1 / 1."));
        assert!(whole.text.contains("Bin 2: squared norms 0 / 0; odd gcds 0 / 0."));
        let rail = run(r#"{"scale_exponent":4,"source":[[3],[1,1,1,1,1,1,1]],"target":[[2,2,1,0],[1,1,1,1,1,1,1]]}"#).unwrap();
        assert!(!rail.holds);
        assert!(rail.text.contains("Bin 0: squared norms 9 / 9; odd gcds 3 / 1."));
    }

    #[test]
    fn equal_global_norm_and_equal_partitions_are_separate_requirements() {
        assert!(!run(r#"{"scale_exponent":0,"source":[[1],[]],"target":[[],[1]]}"#).unwrap().holds);
        assert!(!run(r#"{"scale_exponent":2,"source":[[1]],"target":[[-1]]}"#).unwrap().holds);
        assert!(run(r#"{"scale_exponent":0,"source":[[1]],"target":[[-1]]}"#).unwrap().holds);
    }

    #[test]
    fn signed_min_high_scale_and_overflow_refusal() {
        let n = i64::MIN;
        let row = format!("[[{n},{n}]]");
        let check = |col: &str| run(&format!(r#"{{"scale_exponent":127,"source":{col},"target":{row}}}"#));
        assert!(check(&row).unwrap().holds);
        assert!(check(&format!("[[{n},{n},{n},{n}]]")).is_err());
        assert!(check(&format!("[[{n},{n}],[{n},{n}]]")).is_err());
    }

    #[test]
    fn malformed_and_oversized_inputs_are_refused() {
        for bad in [
            "{}", r#"{"scale_exponent":-1,"source":[[]],"target":[[]]}"#,
            r#"{"scale_exponent":128,"source":[[]],"target":[[]]}"#,
            r#"{"scale_exponent":"1","source":[[]],"target":[[]]}"#,
            r#"{"scale_exponent":0,"source":[],"target":[]}"#,
            r#"{"scale_exponent":0,"source":[[1]],"target":[],"targte":[]}"#,
            r#"{"scale_exponent":0,"source":[[1]],"target":[[],[1]]}"#,
            r#"{"scale_exponent":0,"source":[1],"target":[[]]}"#,
            r#"{"scale_exponent":0,"source":[[true]],"target":[[]]}"#,
        ] {
            assert!(run(bad).is_err(), "{bad}");
        }
        let zeros = vec!["0"; MAX_ENTRIES+1].join(",");
        assert!(run(&format!(r#"{{"scale_exponent":0,"source":[[{zeros}]],"target":[[]]}}"#)).is_err());
        let bins = vec!["[]"; MAX_BINS+1].join(",");
        assert!(run(&format!(r#"{{"scale_exponent":0,"source":[{bins}],"target":[[]]}}"#)).is_err());
    }
}
