//! Exhaustive attacks on a public 16-bit teaching permutation.
use super::{Outcome, Value};
use std::sync::OnceLock;

const SBOX: [u16; 16] = [14, 4, 13, 1, 2, 15, 11, 8, 3, 10, 6, 12, 5, 9, 0, 7];
const PLAINTEXTS: [u16; 4] = [0, 0x1234, 0xabcd, 0xffff];
const FIXTURE_KEY: u16 = 0xb7e1;
const DOMAIN: usize = 1 << 16;

fn substitute_and_permute(word: u16) -> u16 {
    let mut out = 0;
    for nibble in 0..4 {
        let s = SBOX[((word >> (4 * nibble)) & 15) as usize];
        for bit in 0..4 {
            out |= ((s >> bit) & 1) << (4 * bit + nibble);
        }
    }
    out
}

fn layer() -> &'static [u16] {
    static TABLE: OnceLock<Vec<u16>> = OnceLock::new();
    TABLE.get_or_init(|| (0..=u16::MAX).map(substitute_and_permute).collect())
}

fn round_key(key: u16, round: u16) -> u16 {
    key.rotate_left(u32::from(3 * round)) ^ round.wrapping_mul(0x9e37)
}

fn encrypt(mut word: u16, key: u16, rounds: u16, table: &[u16]) -> u16 {
    for i in 0..rounds {
        word = table[usize::from(word ^ round_key(key, i))];
    }
    word ^ round_key(key, rounds)
}

/// The attack only receives public pairs and searches every possible key.
fn recover(pairs: &[(u16, u16)], rounds: u16, table: &[u16]) -> Vec<u16> {
    (0..=u16::MAX)
        .filter(|&key| pairs.iter().all(|&(x, y)| encrypt(x, key, rounds, table) == y))
        .collect()
}

fn differences(codebook: &[u16], delta: u16) -> Vec<u32> {
    let mut counts = vec![0; DOMAIN];
    for x in 0..DOMAIN {
        let difference = codebook[x] ^ codebook[x ^ usize::from(delta)];
        counts[usize::from(difference)] += 1;
    }
    counts
}

pub(super) fn run(input: &Value) -> Result<Outcome, String> {
    let Value::Map(fields) = input else { return Err("expected an object with rounds and optional delta".into()) };
    if fields.keys().any(|k| k != "rounds" && k != "delta") {
        return Err("accepted fields are rounds and optional delta; only public fixtures are supported".into());
    }
    let rounds = match input.get("rounds").and_then(Value::as_int) {
        Some(n @ (1 | 2 | 4 | 8)) => n as u16,
        _ => return Err("rounds must be one of 1, 2, 4, 8".into()),
    };
    let delta = match input.get("delta") {
        None => 1,
        Some(value) => value.as_int().and_then(|n| u16::try_from(n).ok()).filter(|&d| d != 0)
            .ok_or("delta must be an integer between 1 and 65535")?,
    };
    let table = layer();
    let codebook: Vec<_> = (0..=u16::MAX).map(|x| encrypt(x, FIXTURE_KEY, rounds, table)).collect();
    let pairs = PLAINTEXTS.map(|x| (x, codebook[usize::from(x)]));
    let keys = recover(&pairs, rounds, table);
    let counts = differences(&codebook, delta);
    let peak = counts.iter().copied().max().unwrap();
    let mut maxima = counts.iter().enumerate().filter(|(_, c)| **c == peak);
    let first = maxima.next().unwrap().0;
    let ties = 1 + maxima.count();
    let pairs_text = pairs.map(|(x, y)| format!("{x:04x}/{y:04x}")).join(" ");
    let unique = match keys.as_slice() {
        [key] => format!("{key:04x}"),
        _ => "none".to_owned(),
    };
    Ok(Outcome {
        holds: keys.as_slice() == [FIXTURE_KEY],
        text: format!("Public 16-bit teaching cipher, {rounds} rounds. Known plaintext/ciphertext pairs {pairs_text}. Tested 65536 keys; survivors {}; unique key {unique}. Fixed-key XOR delta {delta:04x}: largest count {peak}/65536 ordered inputs, {ties} tied output differences, first {first:04x}.", keys.len()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_fixture_vectors_and_delta_one() {
        for (rounds, answers, peak, first, ties) in [
            (1, [0xe83a, 0x5ced, 0x0e3d, 0x23fe], 16384, 0x1010, 2),
            (2, [0xc555, 0x588e, 0x5c80, 0x4e2d], 1024, 0xa0a0, 8),
            (4, [0x907f, 0xc530, 0x437a, 0xbf9a], 56, 0x8808, 1),
            (8, [0x7127, 0x7537, 0x1c9f, 0x8ce8], 12, 0xede6, 1),
        ] {
            let pairs = std::array::from_fn::<_, 4, _>(|i| (PLAINTEXTS[i], answers[i]));
            assert_eq!(recover(&pairs, rounds, layer()), [FIXTURE_KEY]);
            let result = run(&Value::Map([("rounds".into(), Value::Int(rounds.into()))].into()));
            let outcome = result.unwrap();
            assert!(outcome.holds);
            assert!(outcome.text.contains(&format!("largest count {peak}/65536 ordered inputs, {ties} tied output differences, first {first:04x}")));
        }
    }

    #[test]
    fn accepts_only_the_documented_public_inputs() {
        let good = ("rounds".to_string(), Value::Int(4));
        for (key, value) in [
            ("delta", Value::Int(0)), ("delta", Value::Int(65536)), ("delta", Value::Int(-1)),
            ("delta", Value::Str("1".into())), ("key", Value::Int(3)), ("rounds", Value::Int(3)),
        ] {
            let args = Value::Map([good.clone(), (key.into(), value)].into());
            assert!(run(&args).is_err());
        }
        assert!(run(&Value::Null).is_err());
        assert!(run(&Value::Map(Default::default())).is_err());
    }

    #[test]
    fn each_fixture_is_a_permutation_and_preserves_difference_counts() {
        let table = layer();
        for rounds in [1, 2, 4, 8] {
            let codebook: Vec<_> = (0..=u16::MAX).map(|x| encrypt(x, FIXTURE_KEY, rounds, table)).collect();
            let mut sorted = codebook.clone();
            sorted.sort_unstable();
            assert!(sorted.into_iter().eq(0..=u16::MAX));
            for delta in [1, 0xffff] {
                let counts = differences(&codebook, delta);
                assert_eq!(counts[0], 0);
                assert_eq!(counts.iter().sum::<u32>(), 65536);
                assert!(counts.iter().all(|n| n % 2 == 0));
            }
        }
    }

    #[test]
    fn key_search_preserves_ambiguity_and_inconsistency() {
        assert_eq!(recover(&[], 1, layer()).len(), DOMAIN);
        assert!(recover(&[(1, 0), (1, 1)], 1, layer()).is_empty());
    }
}
