//! Circuit Eight transmits a two-part station call each tide. A signed casebook
//! opens the line. A successful intercept quotes a copy of the actual market.

use crate::{economy::Economy, freight::TIDE_COUNT, manifest::Ending};

const A: &str = ".-.";
const B: &str = "-..";
const C: &str = "..-";

/// In exchange order: Salt, Crab, Timber, Provisions, Fittings.
pub const CALLS: [(&str, &str); 5] = [(A, B), (A, C), (B, A), (B, C), (C, A)];
const STATIONS: [usize; TIDE_COUNT] = [0, 3, 1, 4, 2, 1, 0, 2, 3, 4, 0, 1];

pub fn wire_good(tide: u64) -> usize {
    STATIONS[(tide % TIDE_COUNT as u64) as usize]
}

/// One-crate bids now and at the next three clearings. Zero means a full
/// merchant hold. The projection includes outside traders and omits all
/// prospective player trades, including those on a standing sale ticket.
pub fn intercept(game: &Economy, guess: usize) -> Option<[u64; 4]> {
    if game.manifest.ending == Ending::None || guess != wire_good(game.freight.tide) {
        return None;
    }
    let mut book = game.freight.market.clone();
    let mut phase = game.freight.tide % TIDE_COUNT as u64;
    let mut bids = [0; 4];
    bids[0] = book.bid_lot(guess, 1, phase).coins;
    for next in &mut bids[1..] {
        book.advance(phase, 1);
        phase = (phase + 1) % TIDE_COUNT as u64;
        *next = book.bid_lot(guess, 1, phase).coins;
    }
    Some(bids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_decodes_each_call_and_all_five_stations_get_airtime() {
        let mut airtime = [0; 5];
        for tide in 0..12 {
            let station = wire_good(tide);
            airtime[station] += 1;
            assert_eq!(station, wire_good(tide + 12));
            assert_ne!(station, wire_good(tide + 1));
        }
        for (i, &(prefix, suffix)) in CALLS.iter().enumerate() {
            assert!((2..=3).contains(&airtime[i]));
            assert!(CALLS[..i].iter().all(|&call| call != (prefix, suffix)));
            for group in [prefix, suffix] {
                assert_eq!(group.len(), 3);
                assert!(group.bytes().all(|p| matches!(p, b'.' | b'-')));
            }
        }
    }

    #[test]
    fn either_ending_opens_the_wire_and_correct_calls_match_three_actual_clearings() {
        let mut game = Economy::new();
        assert_eq!(intercept(&game, wire_good(0)), None);
        for ending in [Ending::Print, Ending::Kitchen] {
            game.manifest.ending = ending;
            for tide in [0, 1, 5, 10, 11, u64::MAX - 4] {
                game.freight.tide = tide;
                game.freight.market.stock = [14, 77, 69, 17, 4];
                let before = game.clone();
                let station = wire_good(tide);
                assert!(intercept(&game, (station + 1) % 5).is_none());
                assert!(intercept(&game, usize::MAX).is_none());
                let quotes = intercept(&game, station).unwrap();
                assert_eq!(intercept(&game, station), Some(quotes));
                assert_eq!(game, before);
                let mut actual = before;
                for (i, projected) in quotes.into_iter().enumerate() {
                    if i > 0 {
                        actual.advance(30_000);
                    }
                    assert_eq!(projected, actual.freight.price(station));
                }
            }
        }
    }

    #[test]
    fn intervening_trades_move_the_forecast_and_full_docks_have_no_bid() {
        let mut game = Economy::new();
        game.manifest.ending = Ending::Print;
        game.freight.tide = 2; // Crab importer.
        game.cash = 50_000;
        game.freight.market.stock[1] = 96;
        assert_eq!(intercept(&game, 1).unwrap()[0], 0);
        game.freight.market.stock[1] = 40;
        let before = intercept(&game, 1).unwrap();
        assert_eq!(game.trade_lot(1, true, 18).qty, 18);
        let after = intercept(&game, 1).unwrap();
        assert!(after[0] > before[0]);
        assert_ne!(after[1], before[1]);
        assert!(game.place_sale(1, 1, 18));
        assert_eq!(intercept(&game, 1), Some(after));
        game.advance(30_000);
        assert!(game.freight.price(1) < after[1]);
    }
}
