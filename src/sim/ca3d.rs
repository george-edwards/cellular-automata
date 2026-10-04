/// A 3D "generations"-style cellular automaton on a closed (non-wrapping)
/// X×Y×Z grid, Y pointing up. Cell values: 0 = empty, 1 = alive, 2..states-1
/// = refractory/decaying (these no longer count as neighbours and fade out).
///
/// Rules follow the survival/birth/states/neighbourhood convention used by
/// most 3D CA explorers (e.g. "445", "Pyroclastic", "Clouds").
pub const X3: usize = 72;
pub const Y3: usize = 48;
pub const Z3: usize = 72;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Neighborhood {
    Moore,      // 26 neighbours
    VonNeumann, // 6 face neighbours
}

#[derive(Clone, Copy)]
pub struct Preset3d {
    pub name: &'static str,
    pub rule_str: &'static str,
    pub blurb: &'static str,
    /// Bit i set => a live cell with i live neighbours survives.
    pub survival: u32,
    /// Bit i set => an empty cell with i live neighbours is born.
    pub birth: u32,
    pub states: u8,
    pub nbhd: Neighborhood,
    /// Half-extent of the solid block stamped into the floor for every live
    /// Game of Life cell that touches the boundary (denser rules need more).
    pub stamp: usize,
    /// The whole volume drifts up one layer every `rise` ticks, and the top
    /// layer leaves the box (0 = no drift). This gives the closed box a way
    /// out, so continuous input can't simply fill it.
    pub rise: u32,
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

const fn range_mask(lo: u32, hi: u32) -> u32 {
    let mut m = 0u32;
    let mut i = lo;
    while i <= hi {
        m |= 1 << i;
        i += 1;
    }
    m
}

/// Chosen with tests/rule_search.rs against the cascade's real input. Every
/// preset drifts upward: without a way out, continuous input either dies
/// away or fills the closed box, and almost nothing sits in between.
/// `rise` was tuned per rule so each stays busy without filling the box.
pub const PRESETS_3D: &[Preset3d] = &[
    Preset3d {
        name: "Builder",
        rule_str: "2,6,9/4,6,8-9/10/Moore",
        blurb: "Scaffolding that keeps assembling, collapsing and re-assembling as it drifts upward.",
        survival: mask(&[2, 6, 9]),
        birth: mask(&[4, 6, 8, 9]),
        states: 10,
        nbhd: Neighborhood::Moore,
        stamp: 1,
        rise: 12,
    },
    Preset3d {
        name: "Cumulus",
        rule_str: "0-7/7-10/10/Moore",
        blurb: "Each arrival swells into a rounded cloud that rises and slowly burns out from within.",
        survival: range_mask(0, 7),
        birth: range_mask(7, 10),
        states: 10,
        nbhd: Neighborhood::Moore,
        stamp: 1,
        rise: 7,
    },
    Preset3d {
        name: "Puffs",
        rule_str: "0-6/6/3/Moore",
        blurb: "Short bursts of growth that break away from the floor as separate puffs of smoke.",
        survival: range_mask(0, 6),
        birth: mask(&[6]),
        states: 3,
        nbhd: Neighborhood::Moore,
        stamp: 1,
        rise: 3,
    },
    Preset3d {
        name: "Foam",
        rule_str: "/4/2/Moore",
        blurb: "No cell survives a second tick, so activity travels as waves that leave a rising sheet of foam.",
        survival: 0,
        birth: mask(&[4]),
        states: 2,
        nbhd: Neighborhood::Moore,
        stamp: 1,
        rise: 4,
    },
];

pub struct Ca3d {
    pub preset: Preset3d,
    pub cells: Vec<u8>, // x + z*X3 + y*X3*Z3
    /// Life's top row on the previous tick: only cells that have just
    /// switched on seed the floor, so a still life stuck against the
    /// boundary seeds once rather than every tick forever.
    prev_feed: Vec<u8>,
    tick: u64,
    // scratch buffers for `step`, kept to avoid reallocating every tick
    next: Vec<u8>,
    sum_a: Vec<u8>,
    sum_b: Vec<u8>,
}

#[derive(Clone)]
pub struct Snapshot {
    cells: Vec<u8>,
    prev_feed: Vec<u8>,
    tick: u64,
}

#[inline]
pub fn idx(x: usize, y: usize, z: usize) -> usize {
    x + z * X3 + y * X3 * Z3
}

impl Ca3d {
    pub fn new(preset: Preset3d) -> Self {
        let n = X3 * Y3 * Z3;
        Self {
            preset,
            cells: vec![0; n],
            prev_feed: Vec::new(),
            tick: 0,
            next: vec![0; n],
            sum_a: vec![0; n],
            sum_b: vec![0; n],
        }
    }

    pub fn set_preset(&mut self, preset: Preset3d) {
        self.preset = preset;
        self.cells.fill(0);
        self.prev_feed.clear();
        self.tick = 0;
    }

    /// One generation, then drift (see `Preset3d::rise`), then stamp seeds
    /// for every cell of `feed` (Life's top row, `feed.len()` cells wide,
    /// mapped onto the X axis) that has switched on since the last tick.
    pub fn step(&mut self, feed: &[u8]) {
        let p = self.preset;
        self.count_all(p.nbhd);
        let counts = &self.sum_b;
        for (i, (&v, out)) in self.cells.iter().zip(self.next.iter_mut()).enumerate() {
            *out = if v > 1 {
                // refractory: keep fading regardless of neighbours
                let nv = v + 1;
                if nv >= p.states { 0 } else { nv }
            } else {
                let n = counts[i] as u32;
                if v == 1 {
                    if p.survival >> n & 1 == 1 {
                        1
                    } else if p.states == 2 {
                        0
                    } else {
                        2
                    }
                } else {
                    u8::from(p.birth >> n & 1 == 1)
                }
            };
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        self.tick += 1;
        if p.rise > 0 && self.tick % p.rise as u64 == 0 {
            let layer = X3 * Z3;
            self.cells.copy_within(0..(Y3 - 1) * layer, layer);
            self.cells[..layer].fill(0);
        }
        self.inject(feed);
    }

    /// Live-neighbour count for every cell, left in `sum_b`. Moore uses
    /// running sums along X, then Z, then Y (a 3×3×3 box sum, minus the cell
    /// itself), which is far cheaper than visiting 26 neighbours per cell.
    fn count_all(&mut self, nbhd: Neighborhood) {
        let live = |v: u8| u8::from(v == 1);
        let (cells, a, b) = (&self.cells, &mut self.sum_a, &mut self.sum_b);
        match nbhd {
            Neighborhood::Moore => {
                // X: a = live[x-1] + live[x] + live[x+1]
                for row in 0..Y3 * Z3 {
                    let r = row * X3;
                    for x in 0..X3 {
                        let mut s = live(cells[r + x]);
                        if x > 0 { s += live(cells[r + x - 1]); }
                        if x + 1 < X3 { s += live(cells[r + x + 1]); }
                        a[r + x] = s;
                    }
                }
                // Z: b = a[z-1] + a[z] + a[z+1]
                for y in 0..Y3 {
                    for z in 0..Z3 {
                        let r = idx(0, y, z);
                        for x in 0..X3 {
                            let mut s = a[r + x];
                            if z > 0 { s += a[r + x - X3]; }
                            if z + 1 < Z3 { s += a[r + x + X3]; }
                            b[r + x] = s;
                        }
                    }
                }
                // Y: a = b[y-1] + b[y] + b[y+1], then drop the cell itself
                let layer = X3 * Z3;
                for y in 0..Y3 {
                    for i in y * layer..(y + 1) * layer {
                        let mut s = b[i];
                        if y > 0 { s += b[i - layer]; }
                        if y + 1 < Y3 { s += b[i + layer]; }
                        a[i] = s - live(cells[i]);
                    }
                }
                std::mem::swap(a, b);
            }
            Neighborhood::VonNeumann => {
                for y in 0..Y3 {
                    for z in 0..Z3 {
                        for x in 0..X3 {
                            let at = |x: usize, y: usize, z: usize| live(cells[idx(x, y, z)]);
                            let mut n = 0;
                            if x > 0 { n += at(x - 1, y, z); }
                            if x + 1 < X3 { n += at(x + 1, y, z); }
                            if y > 0 { n += at(x, y - 1, z); }
                            if y + 1 < Y3 { n += at(x, y + 1, z); }
                            if z > 0 { n += at(x, y, z - 1); }
                            if z + 1 < Z3 { n += at(x, y, z + 1); }
                            b[idx(x, y, z)] = n;
                        }
                    }
                }
            }
        }
    }

    fn inject(&mut self, feed: &[u8]) {
        if feed.is_empty() {
            return;
        }
        if self.prev_feed.len() != feed.len() {
            // first tick, or the page was resized: nothing counts as "new"
            // until we've seen one row at this width
            self.prev_feed = feed.to_vec();
            return;
        }
        let s = self.preset.stamp;
        let zc = Z3 / 2;
        for (fx, (&v, prev)) in feed.iter().zip(self.prev_feed.iter_mut()).enumerate() {
            let switched_on = v == 1 && *prev == 0;
            *prev = v;
            if !switched_on {
                continue;
            }
            let x3 = fx * X3 / feed.len();
            for y in 0..=s {
                for dz in zc.saturating_sub(s)..=(zc + s).min(Z3 - 1) {
                    for dx in x3.saturating_sub(s)..=(x3 + s).min(X3 - 1) {
                        self.cells[idx(dx, y, dz)] = 1;
                    }
                }
            }
        }
    }

    pub fn population(&self) -> usize {
        self.cells.iter().filter(|&&c| c == 1).count()
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot { cells: self.cells.clone(), prev_feed: self.prev_feed.clone(), tick: self.tick }
    }

    pub fn restore(&mut self, s: &Snapshot) {
        self.cells = s.cells.clone();
        self.prev_feed = s.prev_feed.clone();
        self.tick = s.tick;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The straightforward per-cell neighbour count the fast path replaced.
    fn count_neighbors(cells: &[u8], x: usize, y: usize, z: usize, nbhd: Neighborhood) -> u32 {
        let (x, y, z) = (x as isize, y as isize, z as isize);
        let live = |xx: isize, yy: isize, zz: isize| -> u32 {
            if xx < 0 || yy < 0 || zz < 0 || xx >= X3 as isize || yy >= Y3 as isize || zz >= Z3 as isize {
                return 0;
            }
            u32::from(cells[idx(xx as usize, yy as usize, zz as usize)] == 1)
        };
        let mut n = 0u32;
        match nbhd {
            Neighborhood::Moore => {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            if dx == 0 && dy == 0 && dz == 0 {
                                continue;
                            }
                            n += live(x + dx, y + dy, z + dz);
                        }
                    }
                }
            }
            Neighborhood::VonNeumann => {
                n = live(x - 1, y, z)
                    + live(x + 1, y, z)
                    + live(x, y - 1, z)
                    + live(x, y + 1, z)
                    + live(x, y, z - 1)
                    + live(x, y, z + 1);
            }
        }
        n
    }

    #[test]
    fn fast_counts_match_reference() {
        // a pseudo-random soup with live, empty and fading cells, edges included
        let mut state = 12345u64;
        for nbhd in [Neighborhood::Moore, Neighborhood::VonNeumann] {
            let mut ca = Ca3d::new(PRESETS_3D[0]);
            for c in ca.cells.iter_mut() {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                *c = ((state >> 33) % 4) as u8;
            }
            ca.count_all(nbhd);
            for y in 0..Y3 {
                for z in 0..Z3 {
                    for x in 0..X3 {
                        assert_eq!(ca.sum_b[idx(x, y, z)] as u32, count_neighbors(&ca.cells, x, y, z, nbhd),
                            "{nbhd:?} at ({x},{y},{z})");
                    }
                }
            }
        }
    }

    /// Deterministic stand-in for Life's top row: sparse bursts of cells.
    fn synthetic_feed(tick: u64, width: usize, density_pct: u64) -> Vec<u8> {
        let mut row = vec![0u8; width];
        let mut state = tick.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        for cell in row.iter_mut() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            *cell = u8::from((state >> 33) % 100 < density_pct);
        }
        row
    }

    #[test]
    fn presets_stay_alive_and_bounded() {
        let volume = X3 * Y3 * Z3;
        for preset in PRESETS_3D {
            let mut ca = Ca3d::new(*preset);
            let mut recent = Vec::new();
            for t in 0..240u64 {
                // Bursty input: active rows only every few ticks, like
                // occasional Life debris reaching the boundary.
                let feed = if t % 4 == 0 {
                    synthetic_feed(t, 320, 8)
                } else {
                    vec![0u8; 320]
                };
                ca.step(&feed);
                if t >= 200 {
                    recent.push(ca.population());
                }
            }
            let avg = recent.iter().sum::<usize>() / recent.len();
            println!("{:>15}: avg live pop over last 40 ticks = {avg} ({:.1}% of volume)",
                preset.name, 100.0 * avg as f64 / volume as f64);
            assert!(avg > 50, "{} nearly died out (avg {avg})", preset.name);
            assert!(avg < volume * 6 / 10, "{} fills the volume (avg {avg})", preset.name);
        }
    }
}
