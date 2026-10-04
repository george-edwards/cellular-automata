//! Rough cost of one 3D step (run with --ignored --nocapture).
use cascade_ca::sim::ca3d::{Ca3d, PRESETS_3D};
use std::time::Instant;

#[test]
#[ignore]
fn bench_step() {
    for p in PRESETS_3D {
        let mut ca = Ca3d::new(*p);
        let feed: Vec<u8> = (0..400).map(|i| u8::from(i % 37 == 0)).collect();
        let off = vec![0u8; 400];
        let t = Instant::now();
        for i in 0..300 { ca.step(if i % 6 == 0 { &feed } else { &off }); }
        println!("{:>8}: {:.2} ms/step ({} cells)", p.name, t.elapsed().as_secs_f64() * 1000.0 / 300.0, ca.cells.len());
    }
}
