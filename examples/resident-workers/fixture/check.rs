//! Exercise the guest inference implementation on the host before boot.
mod model;
fn main() {
    let m = model::Model::load();
    let mut ctx = model::Context::new();
    println!("[");
    for prompt in 0..16 {
        let answer = m.predict(&mut ctx, prompt);
        println!("  {:?}{}", answer, if prompt == 15 { "" } else { "," });
    }
    println!("]");
}
