use std::collections::VecDeque;

/// Wolfram's Rule 30, displayed as a scrolling history: the newest generation
/// is the bottom row, older generations scroll upward. Wraps horizontally.
pub struct Rule30 {
    pub width: usize,
    /// Maximum number of visible rows (the region height in cells).
    pub rows: usize,
    /// Visible history; front = oldest (top), back = newest (bottom).
    pub history: VecDeque<Vec<u8>>,
    /// The latest generation (equal to history.back()).
    current: Vec<u8>,
}

#[derive(Clone)]
pub struct Snapshot {
    history: VecDeque<Vec<u8>>,
    current: Vec<u8>,
}

impl Rule30 {
    pub fn new(width: usize, rows: usize) -> Self {
        // Classic start: a single live cell in the middle.
        let mut current = vec![0u8; width];
        current[width / 2] = 1;
        let mut history = VecDeque::new();
        history.push_back(current.clone());
        Self { width, rows, history, current }
    }

    /// True once the history fills the region, i.e. the oldest row has
    /// scrolled all the way up to the boundary with the region above.
    pub fn is_full(&self) -> bool {
        self.history.len() >= self.rows
    }

    /// The oldest visible row — the one touching the boundary above.
    pub fn top_row(&self) -> &[u8] {
        self.history.front().expect("history is never empty")
    }

    pub fn step(&mut self) {
        let w = self.width;
        let prev = &self.current;
        let mut next = vec![0u8; w];
        for x in 0..w {
            let l = prev[(x + w - 1) % w];
            let c = prev[x];
            let r = prev[(x + 1) % w];
            // Rule 30: new = left XOR (center OR right)
            next[x] = l ^ (c | r);
        }
        self.current = next.clone();
        self.history.push_back(next);
        while self.history.len() > self.rows {
            self.history.pop_front();
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot { history: self.history.clone(), current: self.current.clone() }
    }

    pub fn restore(&mut self, s: &Snapshot) {
        self.history = s.history.clone();
        self.current = s.current.clone();
    }

    pub fn resize(&mut self, width: usize, rows: usize) {
        self.rows = rows.max(2);
        if width != self.width {
            // Width changed: keep the chaos going by cropping/padding each row.
            let remap = |row: &Vec<u8>| -> Vec<u8> {
                let mut out = vec![0u8; width];
                let n = width.min(row.len());
                let src_off = (row.len() - n) / 2;
                let dst_off = (width - n) / 2;
                out[dst_off..dst_off + n].copy_from_slice(&row[src_off..src_off + n]);
                out
            };
            self.history = self.history.iter().map(remap).collect();
            self.current = remap(&self.current);
            self.width = width;
        }
        while self.history.len() > self.rows {
            self.history.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_rule30_evolution() {
        // Centered single cell; known first generations of Rule 30:
        //   t1: XX X  (relative to the seed at offset 0: -1..=1 -> 1,1,?)
        let mut r = Rule30::new(11, 8);
        r.step();
        let c = 5;
        // After one step: cells c-1, c, c+1 should be 1,1,0? Actual rule 30
        // row 1 from single cell is "111" centered? Known: t=1 is 1110? No —
        // canonical rule 30 second row is `XXX` shifted: cells -1,0,1 = 1,1,1?
        // Compute by truth table: left^(c|r):
        //  x=c-1: l=0,c=0,r=1 -> 0^(0|1)=1
        //  x=c:   l=0,c=1,r=0 -> 0^(1|0)=1
        //  x=c+1: l=1,c=0,r=0 -> 1^(0|0)=1
        assert_eq!(&r.current[c - 1..=c + 1], &[1, 1, 1]);
        r.step();
        //  t=2 from 0..0111 0..:
        //  x=c-2: l=0,c=0,r=1 -> 1
        //  x=c-1: l=0,c=1,r=1 -> 1
        //  x=c:   l=1,c=1,r=1 -> 1^1=0
        //  x=c+1: l=1,c=1,r=0 -> 1^1=0
        //  x=c+2: l=1,c=0,r=0 -> 1
        assert_eq!(&r.current[c - 2..=c + 2], &[1, 1, 0, 0, 1]);
    }

    #[test]
    fn scrolls_and_feeds_top_row() {
        let mut r = Rule30::new(32, 4);
        for _ in 0..10 {
            r.step();
        }
        assert!(r.is_full());
        assert_eq!(r.history.len(), 4);
        // Top row is the oldest = generation (10 - 3)
        let mut reference = Rule30::new(32, 1000);
        for _ in 0..7 {
            reference.step();
        }
        assert_eq!(r.top_row(), &reference.current[..]);
    }
}
