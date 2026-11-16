use super::*;

#[test]
fn a_full_jump_outlasts_a_tap_and_a_flap_is_single_use() {
    let flight = |held: bool| {
        let mut g = Game::new();
        g.step(JUMP);
        g.step(0);
        g.step(JUMP);
        let mut peak = g.foot();
        for _ in 0..32 {
            g.step(if held { JUMP } else { 0 });
            peak = peak.min(g.foot());
        }
        peak
    };
    assert!(flight(true) + 20 < flight(false));
    let mut g = Game::new();
    g.step(JUMP);
    g.step(0);
    g.step(JUMP);
    for _ in 0..7 {
        g.step(0);
    }
    assert_eq!(g.step(JUMP), Sfx::Wing);
    g.step(0);
    assert_ne!(g.step(JUMP), Sfx::Wing);
}
#[test]
fn landing_on_a_crab_rebounds_but_contact_at_the_side_resets() {
    for stomp in [true, false] {
        let mut g = Game::new();
        g.step(JUMP);
        g.step(0);
        g.flash = 0;
        g.x = g.crab_x(0) * U;
        g.y = (if stomp { FLOOR - 11 } else { FLOOR }) * U;
        g.vy = 4 * U;
        g.grounded = false;
        let fx = g.step(0);
        if stomp {
            assert_eq!(fx, Sfx::Bounce);
            assert_eq!(g.dead, 1);
            assert!(g.vy < 0)
        } else {
            assert_eq!(fx, Sfx::Splash);
            assert_eq!(g.attempts, 1);
            assert_eq!(g.pos(), 30)
        }
    }
}
#[test]
fn checkpoint_retry_keeps_postmarks_and_the_air_spring_works_without_input() {
    let mut g = Game::new();
    g.step(JUMP);
    g.step(0);
    g.x = g.route().checkpoint * U;
    g.y = FLOOR * U;
    assert_eq!(g.step(0), Sfx::Flag);
    assert!(g.checkpoint);
    g.mask = 3;
    g.y = (H + 15) * U;
    assert_eq!(g.step(0), Sfx::Splash);
    assert_eq!(g.pos(), g.route().checkpoint);
    assert_eq!(g.mask, 3);
    g.x = g.route().springs[0] * U;
    assert_eq!(g.step(0), Sfx::Bounce);
    for _ in 0..12 {
        g.step(0);
    }
    assert!(g.foot() < FLOOR - 70);
}
