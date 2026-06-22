//! Exploratory harness (run with --ignored) to evaluate candidate 3D rules
//! under the cascade's boundary-feeding regime: population trend over time.
use cascade_ca::sim::ca3d::{Ca3d, Neighborhood, Preset3d, X3, Y3, Z3};

fn synthetic_feed(tick: u64, width: usize, density_pct: u64) -> Vec<u8> {
    let mut row = vec![0u8; width];
    let mut state = tick.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    for cell in row.iter_mut() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *cell = u8::from((state >> 33) % 100 < density_pct);
    }
    row
}

const fn range_mask(lo: u32, hi: u32) -> u32 {
    let mut m = 0u32;
    let mut i = lo;
    while i <= hi {
        m |= 1 << i;
        i += 1;
    }
    m
}

const fn mask(bits: &[u32]) -> u32 {
    let mut m = 0u32;
    let mut i = 0;
    while i < bits.len() {
        m |= 1 << bits[i];
        i += 1;
    }
    m
}

#[test]
#[ignore]
fn explore() {
    let volume = (X3 * Y3 * Z3) as f64;
    let candidates: Vec<Preset3d> = vec![
        Preset3d { name: "445", rule_str: "", blurb: "", survival: mask(&[4]), birth: mask(&[4]), states: 5, nbhd: Neighborhood::Moore, stamp: 1 },
        Preset3d { name: "Pyroclastic", rule_str: "", blurb: "", survival: range_mask(4, 7), birth: range_mask(6, 8), states: 10, nbhd: Neighborhood::Moore, stamp: 1 },
        Preset3d { name: "CrystalVN", rule_str: "", blurb: "", survival: range_mask(1, 2), birth: mask(&[1, 3]), states: 5, nbhd: Neighborhood::VonNeumann, stamp: 0 },
        Preset3d { name: "Coral", rule_str: "", blurb: "", survival: range_mask(5, 8), birth: mask(&[6, 7, 9, 12]), states: 4, nbhd: Neighborhood::Moore, stamp: 1 },
        Preset3d { name: "Brain3D-B4", rule_str: "", blurb: "", survival: 0, birth: mask(&[4]), states: 5, nbhd: Neighborhood::Moore, stamp: 1 },
        Preset3d { name: "Brain3D-B4-s2", rule_str: "", blurb: "", survival: 0, birth: mask(&[4]), states: 2, nbhd: Neighborhood::Moore, stamp: 1 },
        Preset3d { name: "Slow445-s2stamp2", rule_str: "", blurb: "", survival: mask(&[4]), birth: mask(&[4]), states: 5, nbhd: Neighborhood::Moore, stamp: 2 },
        Preset3d { name: "Builder", rule_str: "", blurb: "", survival: mask(&[2, 6, 9]), birth: mask(&[4, 6, 8, 9]), states: 10, nbhd: Neighborhood::Moore, stamp: 1 },
        Preset3d { name: "ExpandShell", rule_str: "", blurb: "", survival: range_mask(0, 26), birth: mask(&[6]), states: 3, nbhd: Neighborhood::Moore, stamp: 1 },
    ];
    for preset in candidates {
        let mut ca = Ca3d::new(preset);
        let mut checkpoints = Vec::new();
        for t in 0..600u64 {
            let feed = if t % 4 == 0 { synthetic_feed(t, 320, 8) } else { vec![0u8; 320] };
            ca.step(&feed);
            if (t + 1) % 100 == 0 {
                checkpoints.push(ca.population());
            }
        }
        let pcts: Vec<String> = checkpoints.iter().map(|p| format!("{:5.1}%", 100.0 * *p as f64 / volume)).collect();
        println!("{:>18}: pop @t=100..600: {}", preset.name, pcts.join(" "));
    }
}
