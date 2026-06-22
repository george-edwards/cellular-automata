/// Conway's Game of Life on a grid that wraps horizontally and is closed at
/// the top and bottom. Row 0 is the top of the region (touching the 3D
/// region); the last row is the bottom (touching Rule 30).
///
/// Besides the live/dead grid we keep a per-cell "age" used purely for
/// rendering fading trails of recently-dead cells.
pub struct Gol {
    pub width: usize,
    pub rows: usize,
    pub alive: Vec<u8>,
    pub trail: Vec<u8>,
}

#[derive(Clone)]
pub struct Snapshot {
    alive: Vec<u8>,
    rows: usize,
    width: usize,
}

impl Gol {
    pub fn new(width: usize, rows: usize) -> Self {
        Self {
            width,
            rows,
            alive: vec![0; width * rows],
            trail: vec![0; width * rows],
        }
    }

    pub fn top_row(&self) -> &[u8] {
        &self.alive[0..self.width]
    }

    /// One Life generation. If `feed` is given, it is OR-ed into the bottom
    /// row afterwards (this is Rule 30 pushing cells through the boundary).
    pub fn step(&mut self, feed: Option<&[u8]>) {
        let (w, h) = (self.width, self.rows);
        let mut next = vec![0u8; w * h];
        for y in 0..h {
            let y0 = y.checked_sub(1);
            let y1 = if y + 1 < h { Some(y + 1) } else { None };
            for x in 0..w {
                let xl = (x + w - 1) % w;
                let xr = (x + 1) % w;
                let mut n = 0u8;
                for yy in [y0, Some(y), y1].into_iter().flatten() {
                    let row = yy * w;
                    n += self.alive[row + xl];
                    if yy != y {
                        n += self.alive[row + x];
                    }
                    n += self.alive[row + xr];
                }
                let cell = self.alive[y * w + x];
                next[y * w + x] = u8::from(if cell == 1 { n == 2 || n == 3 } else { n == 3 });
            }
        }
        self.alive = next;
        if let Some(feed) = feed {
            let bottom = (h - 1) * w;
            for (cell, &f) in self.alive[bottom..].iter_mut().zip(feed) {
                *cell |= f;
            }
        }
        // Update trails: live cells glow at full strength, dead ones fade.
        for i in 0..w * h {
            if self.alive[i] == 1 {
                self.trail[i] = 255;
            } else {
                self.trail[i] = self.trail[i].saturating_sub(22);
            }
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot { alive: self.alive.clone(), rows: self.rows, width: self.width }
    }

    pub fn restore(&mut self, s: &Snapshot) {
        if s.width == self.width && s.rows == self.rows {
            self.alive = s.alive.clone();
            // Trails are cosmetic; rebuild them from the restored cells.
            for i in 0..self.alive.len() {
                self.trail[i] = if self.alive[i] == 1 { 255 } else { self.trail[i].saturating_sub(22) };
            }
        }
    }

    /// Resize, keeping content anchored to the bottom (the Rule 30 boundary).
    pub fn resize(&mut self, width: usize, rows: usize) {
        let rows = rows.max(2);
        if width == self.width && rows == self.rows {
            return;
        }
        let mut alive = vec![0u8; width * rows];
        let mut trail = vec![0u8; width * rows];
        let copy_rows = rows.min(self.rows);
        let copy_w = width.min(self.width);
        let src_xoff = (self.width - copy_w) / 2;
        let dst_xoff = (width - copy_w) / 2;
        for r in 0..copy_rows {
            let sy = self.rows - 1 - r;
            let dy = rows - 1 - r;
            for x in 0..copy_w {
                alive[dy * width + dst_xoff + x] = self.alive[sy * self.width + src_xoff + x];
                trail[dy * width + dst_xoff + x] = self.trail[sy * self.width + src_xoff + x];
            }
        }
        self.width = width;
        self.rows = rows;
        self.alive = alive;
        self.trail = trail;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blinker_oscillates() {
        let mut g = Gol::new(8, 8);
        for x in 2..5 {
            g.alive[3 * 8 + x] = 1; // horizontal blinker at row 3
        }
        g.step(None);
        // Should now be vertical at column 3, rows 2..5
        for y in 2..5 {
            assert_eq!(g.alive[y * 8 + 3], 1, "row {y}");
        }
        assert_eq!(g.alive.iter().map(|&c| c as u32).sum::<u32>(), 3);
        g.step(None);
        for x in 2..5 {
            assert_eq!(g.alive[3 * 8 + x], 1);
        }
    }

    #[test]
    fn feed_ors_into_bottom_row() {
        let mut g = Gol::new(6, 4);
        let feed = vec![1, 0, 1, 0, 1, 0];
        g.step(Some(&feed));
        assert_eq!(&g.alive[3 * 6..], &feed[..]);
    }
}
