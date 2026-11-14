use night_post::{
    mail::{Card, Mailbox, Side, CARD_COUNT},
    Game, Route, Scene, ACTION, LEFT, MAIL, PAUSE, RIGHT,
};

fn tap(g: &mut Game, key: u8) {
    g.step(0);
    g.step(key);
}
#[test]
fn welcome_card_flips_without_launching_and_locked_cards_stay_outside() {
    let mut g = Game::new();
    tap(&mut g, MAIL);
    assert_eq!(g.scene, Scene::Mail);
    tap(&mut g, ACTION);
    assert_eq!(g.mailbox.side, Side::Message);
    for _ in 0..10 {
        tap(&mut g, RIGHT);
        assert_eq!(g.mailbox.card, 0);
    }
    tap(&mut g, MAIL);
    assert_eq!(g.scene, Scene::Title);
}
#[test]
fn older_stamps_receive_the_corresponding_letters_in_both_directions() {
    let mut g = Game::new();
    g.restore_album(Route::Aurora.bit(2) | Route::Coast.bit(0));
    tap(&mut g, MAIL);
    tap(&mut g, LEFT);
    assert_eq!(g.mailbox.card, 9);
    tap(&mut g, LEFT);
    assert_eq!(g.mailbox.card, 1);
    tap(&mut g, RIGHT);
    assert_eq!(g.mailbox.card, 9);
    tap(&mut g, RIGHT);
    assert_eq!(g.mailbox.card, 0);
    for i in 1..CARD_COUNT {
        let (r, id) = Card::address(i).unwrap();
        assert!(Mailbox::received(i, r.bit(id)));
    }
}
#[test]
fn reading_in_flight_freezes_the_envelope_and_restores_the_same_scene() {
    let mut g = Game::new();
    tap(&mut g, ACTION);
    tap(&mut g, ACTION);
    g.step(MAIL);
    let snapshot = (g.distance, g.flight_ticks, g.letter.unwrap().age);
    for _ in 0..30 {
        g.step(0);
    }
    tap(&mut g, ACTION);
    tap(&mut g, MAIL);
    assert_eq!(g.scene, Scene::Flying);
    assert_eq!(
        snapshot,
        (g.distance, g.flight_ticks, g.letter.unwrap().age)
    );
    tap(&mut g, PAUSE);
    tap(&mut g, MAIL);
    tap(&mut g, MAIL);
    assert_eq!(g.scene, Scene::Paused);
    g.scene = Scene::Arrived;
    tap(&mut g, MAIL);
    tap(&mut g, MAIL);
    assert_eq!(g.scene, Scene::Arrived);
}
#[test]
fn every_reply_fits_the_postcard_and_has_a_distinct_front_and_back() {
    let mut g = Game::new();
    g.restore_album(511);
    tap(&mut g, MAIL);
    let mut frame = vec![0; night_post::W * night_post::H];
    let mut hashes = std::collections::HashSet::new();
    for i in 0..CARD_COUNT {
        assert_eq!(i, g.mailbox.card);
        let card = Card::for_index(i);
        assert!(card.lines.len() <= 9 && card.lines.iter().all(|s| s.len() * 6 <= 250));
        for _ in 0..2 {
            let pixels = frame.as_mut_slice().try_into().unwrap();
            g.draw(pixels);
            let hash = frame
                .iter()
                .fold(2166136261u32, |a, &p| (a ^ p).wrapping_mul(16777619));
            assert!(hashes.insert(hash));
            tap(&mut g, ACTION);
        }
        tap(&mut g, RIGHT);
    }
}
