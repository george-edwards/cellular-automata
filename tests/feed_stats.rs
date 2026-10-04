//! What does the real cascade actually feed into the 3D floor? (--ignored)
use cascade_ca::sim::Cascade;

#[derive(Clone, Copy, Debug)]
enum Mode {
    /// current behaviour: every live top-row cell feeds, every tick
    Level,
    /// feed only when a top-row cell switches on
    Edge,
    /// top-row cells pass through into 3D and are removed from Life
    PassThrough,
}

#[test]
#[ignore]
fn feed_stats() {
    for mode in [Mode::Level, Mode::Edge, Mode::PassThrough] {
        // (label, width, rows30, rows_gol) for the default split at 4px cells
        for (label, w, r30, rg) in [("desktop", 400, 63, 72), ("phone", 97, 59, 67)] {
            let mut c = Cascade::new(w, r30, rg, 0);
            let mut prev = vec![0u8; w];
            let mut first = None;
            let (mut cells, mut ticks_with) = (0usize, 0usize);
            let mut line = format!("{mode:?} {label:8}");
            for t in 1..=6000u64 {
                c.step();
                let top: Vec<u8> = c.gol.top_row().to_vec();
                let fed: Vec<u8> = match mode {
                    Mode::Level | Mode::PassThrough => top.clone(),
                    Mode::Edge => top.iter().zip(&prev).map(|(&a, &b)| a & !b & 1).collect(),
                };
                prev = top;
                if let Mode::PassThrough = mode {
                    c.gol.alive[..w].fill(0);
                }
                let n = fed.iter().filter(|&&v| v == 1).count();
                if n > 0 && first.is_none() { first = Some(t); }
                cells += n;
                if n > 0 { ticks_with += 1; }
                if t % 1000 == 0 {
                    line += &format!(" | {:5.2}/tick {:3.0}%", cells as f64 / 1000.0, 100.0 * ticks_with as f64 / 1000.0);
                    cells = 0; ticks_with = 0;
                }
            }
            println!("{line}  (first at {first:?})");
        }
    }
}
