pub mod ca3d;
pub mod gol;
pub mod rule30;

pub use ca3d::{Ca3d, Preset3d, PRESETS_3D};
pub use gol::Gol;
pub use rule30::Rule30;

/// The whole cascade: Rule 30 feeds Life feeds the 3D automaton.
pub struct Cascade {
    pub rule30: Rule30,
    pub gol: Gol,
    pub ca3d: Ca3d,
    pub tick: u64,
}

/// A full snapshot of simulation state, used for step-backward history.
#[derive(Clone)]
pub struct Snapshot {
    rule30: rule30::Snapshot,
    gol: gol::Snapshot,
    ca3d: ca3d::Snapshot,
    tick: u64,
}

impl Cascade {
    /// `width` is the cell width shared by the 1D and 2D automata.
    /// `rows30` / `rows_gol` are the visible cell heights of the two regions.
    pub fn new(width: usize, rows30: usize, rows_gol: usize, preset: usize) -> Self {
        Self {
            rule30: Rule30::new(width, rows30),
            gol: Gol::new(width, rows_gol),
            ca3d: Ca3d::new(PRESETS_3D[preset]),
            tick: 0,
        }
    }

    pub fn step(&mut self) {
        self.rule30.step();
        // The oldest visible Rule 30 row (at the region boundary) seeds Life's
        // bottom row, but only once the triangle has scrolled all the way up.
        let feed = if self.rule30.is_full() {
            Some(self.rule30.top_row())
        } else {
            None
        };
        self.gol.step(feed);
        // Whatever reaches Life's top row seeds the floor of the 3D world.
        self.ca3d.step(self.gol.top_row());
        self.tick += 1;
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            rule30: self.rule30.snapshot(),
            gol: self.gol.snapshot(),
            ca3d: self.ca3d.snapshot(),
            tick: self.tick,
        }
    }

    pub fn restore(&mut self, s: &Snapshot) {
        self.rule30.restore(&s.rule30);
        self.gol.restore(&s.gol);
        self.ca3d.restore(&s.ca3d);
        self.tick = s.tick;
    }

    /// Resize the visible regions, preserving as much state as possible.
    pub fn resize(&mut self, width: usize, rows30: usize, rows_gol: usize) {
        self.rule30.resize(width, rows30);
        self.gol.resize(width, rows_gol);
    }
}
