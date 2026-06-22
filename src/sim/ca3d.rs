/// A 3D "generations"-style cellular automaton on a closed (non-wrapping)
/// X×Y×Z grid, Y pointing up. Cell values: 0 = empty, 1 = alive, 2..states-1
/// = refractory/decaying (these no longer count as neighbours and fade out).
///
/// Rules follow the survival/birth/states/neighbourhood convention used by
/// most 3D CA explorers (e.g. "445", "Pyroclastic", "Clouds").
pub const X3: usize = 72;
pub const Y3: usize = 48;
pub const Z3: usize = 72;

#[derive(Clone, Copy, PartialEq, Eq)]
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

pub const PRESETS_3D: &[Preset3d] = &[
    Preset3d {
        name: "445",
        rule_str: "4/4/5/Moore",
        blurb: "A slow builder: seeds grow into sparse, crystalline towers and arches that hold their shape.",
        survival: mask(&[4]),
        birth: mask(&[4]),
        states: 5,
        nbhd: Neighborhood::Moore,
        stamp: 1,
    },
    Preset3d {
        name: "Pyroclastic",
        rule_str: "4-7/6-8/10/Moore",
        blurb: "Erupts into billowing, smoke-like plumes that collapse and re-ignite wherever fresh debris lands.",
        survival: range_mask(4, 7),
        birth: range_mask(6, 8),
        states: 10,
        nbhd: Neighborhood::Moore,
        stamp: 1,
    },
    Preset3d {
        name: "Crystal Growth",
        rule_str: "1-2/1,3/5/von Neumann",
        blurb: "Grows sharp, branching crystals — like frost spreading across a window, but in 3D.",
        survival: range_mask(1, 2),
        birth: mask(&[1, 3]),
        states: 5,
        nbhd: Neighborhood::VonNeumann,
        stamp: 0,
    },
    Preset3d {
        name: "Coral",
        rule_str: "5-8/6-7,9,12/4/Moore",
        blurb: "Builds reef-like shells: crowded interiors die off, so only the living surface keeps growing outward.",
        survival: range_mask(5, 8),
        birth: mask(&[6, 7, 9, 12]),
        states: 4,
        nbhd: Neighborhood::Moore,
        stamp: 1,
    },
    Preset3d {
        name: "Builder",
        rule_str: "2,6,9/4,6,8-9/10/Moore",
        blurb: "Sparse scaffolding that endlessly assembles, dissolves and re-assembles itself.",
        survival: mask(&[2, 6, 9]),
        birth: mask(&[4, 6, 8, 9]),
        states: 10,
        nbhd: Neighborhood::Moore,
        stamp: 1,
    },
];

pub struct Ca3d {
    pub preset: Preset3d,
    pub cells: Vec<u8>, // x + z*X3 + y*X3*Z3
}

#[derive(Clone)]
pub struct Snapshot {
    cells: Vec<u8>,
}

#[inline]
pub fn idx(x: usize, y: usize, z: usize) -> usize {
    x + z * X3 + y * X3 * Z3
}

impl Ca3d {
    pub fn new(preset: Preset3d) -> Self {
        Self { preset, cells: vec![0; X3 * Y3 * Z3] }
    }

    pub fn set_preset(&mut self, preset: Preset3d) {
        self.preset = preset;
        self.cells.fill(0);
    }

    /// One generation, then stamp seeds for every live cell in `feed`
    /// (Life's top row, `feed.len()` cells wide, mapped onto the X axis).
    pub fn step(&mut self, feed: &[u8]) {
        let p = self.preset;
        let mut next = vec![0u8; self.cells.len()];
        for y in 0..Y3 {
            for z in 0..Z3 {
                for x in 0..X3 {
                    let i = idx(x, y, z);
                    let v = self.cells[i];
                    if v > 1 {
                        // refractory: keep fading regardless of neighbours
                        let nv = v + 1;
                        next[i] = if nv >= p.states { 0 } else { nv };
                        continue;
                    }
                    let n = self.count_neighbors(x, y, z, p.nbhd);
                    next[i] = if v == 1 {
                        if p.survival >> n & 1 == 1 {
                            1
                        } else if p.states == 2 {
                            0
                        } else {
                            2
                        }
                    } else {
                        u8::from(p.birth >> n & 1 == 1)
                    };
                }
            }
        }
        self.cells = next;
        self.inject(feed);
    }

    fn inject(&mut self, feed: &[u8]) {
        if feed.is_empty() {
            return;
        }
        let s = self.preset.stamp;
        let zc = Z3 / 2;
        for (fx, &v) in feed.iter().enumerate() {
            if v == 0 {
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

    fn count_neighbors(&self, x: usize, y: usize, z: usize, nbhd: Neighborhood) -> u32 {
        let (x, y, z) = (x as isize, y as isize, z as isize);
        let live = |xx: isize, yy: isize, zz: isize| -> u32 {
            if xx < 0 || yy < 0 || zz < 0 || xx >= X3 as isize || yy >= Y3 as isize || zz >= Z3 as isize {
                return 0;
            }
            u32::from(self.cells[idx(xx as usize, yy as usize, zz as usize)] == 1)
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

    pub fn population(&self) -> usize {
        self.cells.iter().filter(|&&c| c == 1).count()
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot { cells: self.cells.clone() }
    }

    pub fn restore(&mut self, s: &Snapshot) {
        self.cells = s.cells.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
