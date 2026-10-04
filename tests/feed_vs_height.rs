//! How Life's band height affects what reaches the 3D floor (--ignored).
use cascade_ca::sim::Cascade;

#[test]
#[ignore]
fn feed_vs_height() {
    // 1280x800 laptop: 320 cells wide, Rule 30 band fixed at 28% (56 rows)
    for rows_gol in [64usize, 48, 40, 32] {
        let mut c = Cascade::new(320, 56, rows_gol, 0);
        let mut prev = vec![0u8; 320];
        let (mut first, mut events, mut quiet, mut gap) = (None, 0usize, 0usize, 0usize);
        for t in 1..=6000u64 {
            c.step();
            let top = c.gol.top_row();
            let n = top.iter().zip(&prev).filter(|(&a, &b)| a == 1 && b == 0).count();
            prev = top.to_vec();
            if n > 0 { first.get_or_insert(t); events += n; gap = 0; } else { gap += 1; }
            if first.is_some() && gap >= 150 { quiet += 1; } // ticks inside a 5s+ silence
        }
        let f = first.unwrap_or(0);
        println!("Life {rows_gol:2} rows ({:2.0}% of 800px): first seed at {:5.1}s, {:.2} seeds/tick after, {:3.0}% of ticks in a 5s+ lull",
            rows_gol as f64 * 4.0 / 8.0, f as f64 / 30.0, events as f64 / (6000 - f) as f64, 100.0 * quiet as f64 / (6000 - f) as f64);
    }
}
