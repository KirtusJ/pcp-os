use night_post::{clean_album, Game, Route, Scene, ACTION, BRAKE, LEFT, PAUSE, RIGHT, UP};

#[test]
fn each_route_can_earn_all_three_gold_postmarks() {
    let mut g = Game::new();
    for route in [Route::Coast, Route::Flotilla, Route::Aurora] {
        assert_eq!(g.route, route);
        g.step(ACTION);
        g.step(0);
        for _ in 0..2200 {
            let mut input = if g.y > 68 { UP } else { 0 };
            if g.letter.is_none() {
                for i in 0..3 {
                    if g.delivered & (1 << i) != 0 {
                        continue;
                    }
                    let mut l = g.drop_origin();
                    let speed = g.route.speed();
                    let mut camera = g.distance + speed;
                    for dt in 1..55 {
                        let prior = l.y;
                        l.advance(speed);
                        camera += speed;
                        let (wx, wy) = g.route.position(i, g.flight_ticks + 1 + dt);
                        if prior <= wy && l.y >= wy {
                            if ((wx - camera + 200).rem_euclid(g.route.length()) - 200 - l.x).abs()
                                <= 2
                            {
                                input |= ACTION;
                            }
                            break;
                        }
                    }
                }
            }
            g.step(input);
            if g.scene == Scene::Arrived {
                break;
            }
        }
        assert_eq!((g.scene, g.centered, g.drops), (Scene::Arrived, 7, 3));
        g.step(RIGHT);
        g.step(0);
    }
    assert_eq!(g.album, 0x3ffff);
}
#[test]
fn album_is_monotonic_across_chart_and_replay_and_defends_its_format() {
    assert_eq!(clean_album(1 << 9), 0);
    let mut g = Game::new();
    g.restore_album(1 | (1 << 9) | (1 << 25));
    assert_eq!(g.album, 513);
    g.step(LEFT);
    assert_eq!(g.route, Route::Aurora);
    g.step(ACTION);
    g.step(PAUSE);
    g.step(ACTION);
    assert_eq!(
        (g.scene, g.album, g.route),
        (Scene::Title, 513, Route::Aurora)
    );
    g.step(RIGHT);
    g.step(0);
    g.step(RIGHT);
    assert_eq!(g.route, Route::Flotilla);
}
#[test]
fn airbrake_changes_scroll_and_letter_drift_and_lap_counts_on_overshoot() {
    let mut g = Game::new();
    g.step(LEFT);
    g.step(ACTION);
    g.distance = g.route.length() - 1;
    g.step(0);
    assert_eq!((g.distance, g.laps), (2, 2));
    g.step(BRAKE | ACTION);
    let start = g.letter.unwrap();
    let d = g.distance;
    g.step(BRAKE);
    assert_eq!((g.distance, g.letter.unwrap().x), (d + 1, start.x));
    g.step(0);
    assert_eq!(g.letter.unwrap().x, start.x - 2);
}

#[test]
fn a_loaded_book_cannot_claim_gold_without_a_delivery() {
    let value = clean_album(0xffff_ffff);
    assert_eq!(value, 0x3ffff);
    assert_eq!(clean_album((5 << 9) | 6), (4 << 9) | 6);
}
