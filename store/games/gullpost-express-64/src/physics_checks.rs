use super::*;
#[test]
fn air_flap_rearms_on_landing() {
    let mut g = Game::new();
    g.step(JUMP);
    g.step(0);
    assert_eq!(g.step(JUMP), Sfx::Hop);
    for _ in 0..4 {
        g.step(0);
    }
    assert_eq!(g.step(JUMP), Sfx::Wing);
    g.step(0);
    assert_eq!(g.step(JUMP), Sfx::Quiet);
    for _ in 0..70 {
        g.step(0);
    }
    assert!(g.grounded && !g.used_wing);
}
#[test]
fn tide_restores_the_checkpoint_and_keeps_seals() {
    let mut g = Game::new();
    g.step(TOUR);
    for _ in 0..160 {
        g.step(0);
        if g.checkpoint {
            break;
        }
    }
    assert!(g.checkpoint);
    assert_eq!(g.mask, 1);
    g.demo = false;
    g.pos = v(1400, -104, 1200);
    g.velocity.y = -10;
    assert_eq!(g.step(0), Sfx::Splash);
    assert_eq!((g.pos.x, g.pos.y, g.pos.z), (POST.x, POST.y, POST.z));
    assert_eq!(g.mask, 1);
}
#[test]
fn keepers_box_requires_all_three() {
    let mut g = Game::new();
    g.step(JUMP);
    g.step(0);
    g.pos = BOX;
    g.mask = 3;
    g.step(0);
    assert_eq!(g.mode, Mode::Run);
    g.mask = 7;
    assert_eq!(g.step(0), Sfx::Deliver);
    assert_eq!(g.mode, Mode::Delivered);
    g.step(JUMP);
    assert_eq!(g.mode, Mode::Cover);
    g.step(JUMP);
    assert_eq!(g.mode, Mode::Cover);
}
