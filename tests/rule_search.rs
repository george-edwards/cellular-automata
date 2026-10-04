//! Search for 3D rules that behave well on the cascade's *real* input
//! (run with: cargo test --release --test rule_search -- --ignored --nocapture).
use cascade_ca::sim::ca3d::{Ca3d, Neighborhood, Preset3d, X3, Y3, Z3};
use cascade_ca::sim::Cascade;
use std::sync::{Arc, Mutex};

/// Parse "survival/birth/states/M|VN", ranges like "4-7,9".
fn parse(rule: &str) -> (u32, u32, u8, Neighborhood) {
    let parts: Vec<&str> = rule.split('/').collect();
    let set = |s: &str| -> u32 {
        let mut m = 0;
        for tok in s.split(',').filter(|t| !t.is_empty()) {
            let (lo, hi) = match tok.split_once('-') {
                Some((a, b)) => (a.parse::<u32>().unwrap(), b.parse::<u32>().unwrap()),
                None => (tok.parse().unwrap(), tok.parse().unwrap()),
            };
            for i in lo..=hi { m |= 1 << i; }
        }
        m
    };
    let nb = if parts[3].starts_with('V') { Neighborhood::VonNeumann } else { Neighborhood::Moore };
    (set(parts[0]), set(parts[1]), parts[2].parse().unwrap(), nb)
}

fn preset(rule: &'static str, stamp: usize, rise: u32) -> Preset3d {
    let (survival, birth, states, nbhd) = parse(rule);
    Preset3d { name: rule, rule_str: rule, blurb: "", survival, birth, states, nbhd, stamp, rise }
}

/// Top rows of Life from a real cascade run, starting at the first tick
/// anything reaches the top.
fn record_trace(w: usize, r30: usize, rg: usize, ticks: usize) -> Vec<Vec<u8>> {
    let mut c = Cascade::new(w, r30, rg, 0);
    let mut out = Vec::new();
    while out.len() < ticks {
        c.step();
        let top = c.gol.top_row();
        if !out.is_empty() || top.iter().any(|&v| v == 1) {
            out.push(top.to_vec());
        }
    }
    out
}

struct Stats { mean_vis: f64, max_vis: f64, empty: f64, height: f64, end_vis: f64, solid: f64 }

fn run(p: Preset3d, feed: &[Vec<u8>]) -> Stats {
    let mut ca = Ca3d::new(p);
    let (mut vis_sum, mut max_vis, mut empty, mut h_sum, mut n) = (0f64, 0f64, 0usize, 0f64, 0usize);
    let vol = (X3 * Y3 * Z3) as f64;
    let mut end_vis = 0.0;
    let (mut solid_sum, mut solid_n) = (0f64, 0usize);
    for (t, row) in feed.iter().enumerate() {
        ca.step(row);
        if t % 10 == 9 && t >= feed.len() / 4 {
            let vis = ca.cells.iter().filter(|&&c| c != 0).count() as f64;
            let top = (0..Y3).rev().find(|&y| ca.cells[y * X3 * Z3..(y + 1) * X3 * Z3].iter().any(|&c| c == 1)).map_or(0, |y| y + 1);
            vis_sum += vis; max_vis = max_vis.max(vis); h_sum += top as f64; n += 1;
            if vis < 150.0 { empty += 1; }
            // share of visible cubes buried inside a lump (all 6 faces covered):
            // high means solid blobs, low means lacy, structured growth
            if t % 50 == 49 && vis >= 150.0 {
                let c = &ca.cells;
                let (layer, mut inner) = (X3 * Z3, 0usize);
                for y in 1..Y3 - 1 { for z in 1..Z3 - 1 { for x in 1..X3 - 1 {
                    let i = x + z * X3 + y * layer;
                    if c[i] != 0 && c[i - 1] != 0 && c[i + 1] != 0 && c[i - X3] != 0 && c[i + X3] != 0
                        && c[i - layer] != 0 && c[i + layer] != 0 { inner += 1; }
                }}}
                solid_sum += inner as f64 / vis; solid_n += 1;
            }
            end_vis = vis;
        }
    }
    Stats { mean_vis: vis_sum / n as f64 / vol * 100.0, max_vis: max_vis / vol * 100.0,
            empty: empty as f64 / n as f64 * 100.0, height: h_sum / n as f64, end_vis: end_vis / vol * 100.0,
            solid: if solid_n > 0 { solid_sum / solid_n as f64 * 100.0 } else { 0.0 } }
}

/// Known 3D rules (survival/birth/states/neighbourhood), mostly from the
/// rule lists that circulate with 3D CA explorers.
const KNOWN: &[&str] = &[
    "4/4/5/M", "4-7/6-8/10/M", "1-2/1,3/5/VN", "5-8/6-7,9,12/4/M", "2,6,9/4,6,8-9/10/M",
    "9-26/5-7,12-13,15/5/M", "4-6/3/2/M", "13-26/13-14,17-19/2/M", "12-26/13-14/2/M",
    "0-6/1,3/2/VN", "5-6/1-3/7/VN", "7-26/4/4/M", "3/1-3/10/M", "10-26/5,8-26/4/M",
    "13-26/10-26/3/M", "1,4,8,11,13-26/13-26/5/M", "13-26/14-19/2/M", "/2/10/M", "1-3/1,4-5/5/VN",
    "5-7/1/2/M", "6-8/4-5/6/M", "4-5/5/10/M", "3-5/4/8/M", "4-6/4/6/M", "5-7/5-6/8/M", "2-4/4/12/M",
    "4/4/3/M", "4-5/4/5/M", "3-4/4/6/M", "5-6/4-5/10/M",
];

fn random_rules(n: usize) -> Vec<&'static str> {
    let mut st = 0x9e3779b97f4a7c15u64;
    let mut r = |m: u64| { st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (st >> 33) % m };
    let states = [2u64, 3, 4, 5, 6, 8, 10, 14];
    (0..n).map(|_| {
        let b1 = 2 + r(7); let b2 = b1 + r(4);
        let surv = if r(5) == 0 { String::new() } else { let s1 = r(11); format!("{}-{}", s1, s1 + r(9)) };
        let s = format!("{surv}/{b1}-{b2}/{}/M", states[r(states.len() as u64) as usize]);
        &*Box::leak(s.into_boxed_str())
    }).collect()
}

#[test]
#[ignore]
fn search() {
    let ticks = 4000;
    // the 3D automaton does its own "switched on" detection, so feed it the raw rows
    let (w, r30, rg) = if std::env::var("PHONE").is_ok() { (97, 59, 67) } else { (400, 63, 72) };
    let trace = Arc::new(record_trace(w, r30, rg, ticks));
    let rises: Vec<u32> = std::env::var("RISES").map_or(vec![0, 2, 3, 5], |v| v.split(',').map(|r| r.parse().unwrap()).collect());
    let mut rules: Vec<&'static str> = match std::env::var("ONLY") {
        Ok(list) => list.split(';').map(|r| &*Box::leak(r.to_string().into_boxed_str())).collect(),
        Err(_) => KNOWN.to_vec(),
    };
    if std::env::var("ONLY").is_err() {
        rules.extend(random_rules(std::env::var("N").map_or(400, |v| v.parse().unwrap())));
    }
    let mut jobs = Vec::new();
    for &r in &rules {
        for stamp in [0, 1] {
            for &rise in &rises {
                jobs.push((r, stamp, rise));
            }
        }
    }
    let jobs = Arc::new(Mutex::new(jobs));
    let results = Arc::new(Mutex::new(Vec::new()));
    let handles: Vec<_> = (0..20).map(|_| {
        let (jobs, results, trace) = (jobs.clone(), results.clone(), trace.clone());
        std::thread::spawn(move || loop {
            let Some((rule, stamp, rise)) = jobs.lock().unwrap().pop() else { break };
            let s = run(preset(rule, stamp, rise), &trace);
            results.lock().unwrap().push((rule, stamp, rise, s));
        })
    }).collect();
    for h in handles { h.join().unwrap(); }
    let mut res = std::mem::take(&mut *results.lock().unwrap());
    if let Ok(path) = std::env::var("CSV") {
        let mut out = String::from("rule\tstamp\trise\tmean\tmax\tempty\theight\tend\tsolid\n");
        for (rule, stamp, rise, s) in &res {
            out += &format!("{rule}\t{stamp}\t{rise}\t{:.3}\t{:.3}\t{:.1}\t{:.1}\t{:.3}\t{:.1}\n",
                s.mean_vis, s.max_vis, s.empty, s.height, s.end_vis, s.solid);
        }
        std::fs::write(path, out).unwrap();
    }
    if std::env::var("ALL").is_err() {
        // visibly busy, rarely empty, never close to full
        res.retain(|(_, _, _, s)| s.max_vis < 12.0 && s.mean_vis > 0.5 && s.empty < 20.0);
    }
    res.sort_by(|a, b| b.3.mean_vis.partial_cmp(&a.3.mean_vis).unwrap());
    println!("{:<30} {:>5} {:>4} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}", "rule", "stamp", "rise", "mean%", "max%", "empty%", "height", "end%", "solid%");
    for (rule, stamp, rise, s) in res.iter().take(200) {
        println!("{:<30} {:>5} {:>4} {:>7.2} {:>7.2} {:>7.1} {:>7.1} {:>7.2} {:>7.1}", rule, stamp, rise, s.mean_vis, s.max_vis, s.empty, s.height, s.end_vis, s.solid);
    }
}

/// Dump voxel snapshots for rendering: RULES="rule@stamp@rise;..."
/// TICKS="1000,2000" OUT=dir [PHONE=1]. Files: OUT/<i>_<tick>.bin (raw bytes).
#[test]
#[ignore]
fn dump() {
    let rules = std::env::var("RULES").unwrap();
    let ticks: Vec<usize> = std::env::var("TICKS").unwrap().split(',').map(|t| t.parse().unwrap()).collect();
    let out = std::env::var("OUT").unwrap();
    let (w, r30, rg) = if std::env::var("PHONE").is_ok() { (97, 59, 67) } else { (400, 63, 72) };
    let trace = record_trace(w, r30, rg, *ticks.iter().max().unwrap());
    let specs: Vec<(String, usize, u32)> = rules.split(';').map(|r| {
        let f: Vec<&str> = r.split('@').collect();
        (f[0].to_string(), f[1].parse().unwrap(), f.get(2).map_or(0, |v| v.parse().unwrap()))
    }).collect();
    std::thread::scope(|sc| {
        for (i, (rule, stamp, rise)) in specs.iter().enumerate() {
            let (trace, ticks, out) = (&trace, &ticks, &out);
            sc.spawn(move || {
                let mut ca = Ca3d::new(preset(Box::leak(rule.clone().into_boxed_str()), *stamp, *rise));
                for (t, row) in trace.iter().enumerate() {
                    ca.step(row);
                    if ticks.contains(&(t + 1)) {
                        std::fs::write(format!("{out}/{i}_{}.bin", t + 1), &ca.cells).unwrap();
                    }
                }
            });
        }
    });
}
