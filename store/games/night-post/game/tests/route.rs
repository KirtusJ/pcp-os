use night_post::{Game, Scene, Sound, ACTION, DOWN, LEFT, PAUSE, POSTS, RIGHT, ROUTE, UP};

#[test]
fn first_lap_deliveries_and_fresh_replay() {
    let mut g = Game::new();
    g.step(ACTION);
    for _ in 0..ROUTE {
        let over_approach = (0..POSTS.len())
            .any(|id| g.delivered & (1 << id) == 0 && (g.post_x(id) - g.x - 25).abs() <= 1);
        g.step(if over_approach && g.letter.is_none() {
            ACTION
        } else {
            0
        });
        if g.scene == Scene::Arrived {
            break;
        }
    }
    assert_eq!(
        (g.scene, g.count(), g.drops, g.laps),
        (Scene::Arrived, 3, 3, 1)
    );
    g.step(0);
    g.step(ACTION);
    assert_eq!(
        (g.scene, g.count(), g.drops, g.distance, g.laps),
        (Scene::Flying, 0, 0, 0, 1)
    );
}

#[test]
fn a_miss_can_be_retried_on_another_lap() {
    let mut g = Game::new();
    g.step(ACTION);
    g.step(0);
    g.step(ACTION);
    let mut missed = false;
    while g.laps == 1 {
        missed |= g.step(0) == Sound::Missed;
    }
    assert!(missed && g.letter.is_none());
    assert_eq!(g.count(), 0);
    while g.post_x(0) - g.x > 27 {
        g.step(0);
    }
    g.step(ACTION);
    for _ in 0..50 {
        g.step(0);
    }
    assert_eq!(g.delivered, 1);
}

#[test]
fn pause_freezes_the_letter_and_a_held_button_drops_once() {
    let mut g = Game::new();
    g.step(ACTION);
    g.step(0);
    g.step(ACTION);
    g.step(PAUSE);
    let at = (g.distance, g.letter.unwrap().age);
    for _ in 0..80 {
        g.step(RIGHT);
    }
    assert_eq!((g.distance, g.letter.unwrap().age), at);
    g.step(PAUSE);
    g.step(0);
    for _ in 0..200 {
        g.step(ACTION);
    }
    assert_eq!(g.drops, 1);
}

#[test]
fn steering_reaches_the_edges_and_storm_pushes_the_bird() {
    let mut g = Game::new();
    g.step(ACTION);
    for _ in 0..130 {
        g.step(UP | LEFT);
    }
    assert_eq!((g.x, g.y), (32, 49));
    for _ in 0..150 {
        g.step(DOWN | RIGHT);
    }
    assert_eq!((g.x, g.y), (322, 229));
    let (cx, cy) = g.clouds()[0];
    // Place the bird at a cloud using a normal route position.
    g.distance = (g.distance + cx - 118).rem_euclid(ROUTE);
    g.x = 118;
    g.y = cy;
    assert!(g.step(0) == Sound::Thunder);
    let before = g.y;
    for _ in 0..6 {
        g.step(0);
    }
    assert!(g.y > before);
}
