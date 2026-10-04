//! Wasm entry point and JS-facing API. JS owns the DOM (buttons, drag
//! handles, modal); this side owns simulation, history and rendering.
use crate::layout::{dims_3d, framing_camera, Camera, View};
use crate::render::{CubeInstance, Palette, RegionColors, Renderer};
use crate::sim::{Cascade, Snapshot, PRESETS_3D};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

/// Cell size of the 1D/2D automata in CSS pixels.
const CELL_PX: f64 = 4.0;
/// How many ticks of undo history to keep (~330 KB per snapshot).
const HISTORY: usize = 100;
const MIN_REGION_FRAC: f64 = 0.08;
struct App {
    renderer: Renderer,
    cascade: Cascade,
    history: VecDeque<Snapshot>,
    paused: bool,
    tps: f64,
    acc: f64,
    last_ms: f64,
    view: View,
    // layout
    css_w: f64,
    css_h: f64,
    dpr: f64,
    b1_frac: f64, // boundary between 3D (above) and Life (below), fraction of height
    b2_frac: f64, // boundary between Life and Rule 30
    preset_idx: usize,
    dirty: bool,
    instances: Vec<CubeInstance>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.borrow_mut().as_mut().map(f))
}

impl App {
    fn grid_dims(&self) -> (usize, usize, usize) {
        let width = ((self.css_w / CELL_PX) as usize).max(16);
        let rows30 = (((1.0 - self.b2_frac) * self.css_h / CELL_PX) as usize).max(2);
        let rows_gol = (((self.b2_frac - self.b1_frac) * self.css_h / CELL_PX) as usize).max(2);
        (width, rows30, rows_gol)
    }

    fn tick(&mut self) {
        self.history.push_back(self.cascade.snapshot());
        while self.history.len() > HISTORY {
            self.history.pop_front();
        }
        self.cascade.step();
        self.dirty = true;
    }

    fn step_back(&mut self) {
        if let Some(s) = self.history.pop_back() {
            self.cascade.restore(&s);
            self.dirty = true;
        }
    }

    fn upload(&mut self) {
        // Rule 30: render history rows anchored to the bottom of the region.
        let r30 = &self.cascade.rule30;
        let (w, rows) = (r30.width, r30.rows);
        let mut data = vec![0u8; w * rows];
        let n = r30.history.len().min(rows);
        for (i, row) in r30.history.iter().rev().take(n).enumerate() {
            // newest row at the very bottom
            let y = rows - 1 - i;
            let dst = &mut data[y * w..(y + 1) * w];
            for (d, &c) in dst.iter_mut().zip(row.iter()) {
                *d = if c == 1 { 255 } else { 0 };
            }
        }
        self.renderer.upload_rule30(w as u32, rows as u32, &data);

        // Life: alive = 255, otherwise the fading trail value (< 240).
        let gol = &self.cascade.gol;
        let mut gdata = vec![0u8; gol.width * gol.rows];
        for (i, d) in gdata.iter_mut().enumerate() {
            *d = if gol.alive[i] == 1 {
                255
            } else {
                (gol.trail[i] as u32 * 230 / 255) as u8
            };
        }
        self.renderer.upload_gol(gol.width as u32, gol.rows as u32, &gdata);

        // 3D instances
        self.instances.clear();
        let c3 = &self.cascade.ca3d;
        let cells = &c3.cells;
        for y in 0..c3.ny {
            for z in 0..c3.nz {
                let base = y * c3.nx * c3.nz + z * c3.nx;
                for x in 0..c3.nx {
                    let v = cells[base + x];
                    if v != 0 {
                        self.instances.push(CubeInstance {
                            pos: [x as f32, y as f32, z as f32],
                            state: v as f32,
                        });
                    }
                }
            }
        }
        self.renderer.upload_instances(&self.instances);
        self.dirty = false;
    }

    fn frame(&mut self, now_ms: f64) {
        let dt = ((now_ms - self.last_ms) / 1000.0).clamp(0.0, 0.25);
        self.last_ms = now_ms;
        if !self.paused {
            self.acc += dt;
            let tick_dt = 1.0 / self.tps;
            let mut steps = 0;
            while self.acc >= tick_dt && steps < 4 {
                self.tick();
                self.acc -= tick_dt;
                steps += 1;
            }
            if steps == 4 {
                self.acc = 0.0; // can't keep up; don't spiral
            }
        }
        if self.dirty {
            self.upload();
        }
        let h_dev = self.css_h * self.dpr;
        self.renderer.render(
            (self.b1_frac * h_dev) as f32,
            (self.b2_frac * h_dev) as f32,
            self.view.camera,
            self.cascade.ca3d.preset.states,
            (self.cascade.ca3d.nx, self.cascade.ca3d.ny, self.cascade.ca3d.nz),
        );
    }

    fn band_3d(&self) -> (f64, f64) {
        (self.css_w, self.b1_frac * self.css_h)
    }

    /// The camera that frames the box in its band, as of the last layout.
    fn default_camera(&self) -> Camera {
        self.view.frame()
    }

    fn apply_layout(&mut self) {
        let (w, rows30, rows_gol) = self.grid_dims();
        self.cascade.resize(w, rows30, rows_gol);
        // Reshape the 3D box to the band and re-frame the camera (see View).
        let (bw, bh) = self.band_3d();
        let dims = dims_3d(bw, bh);
        self.cascade.ca3d.resize(dims.0, dims.1, dims.2);
        self.view.reframe(framing_camera(dims, bw / bh.max(1.0)));
        self.history.clear();
        self.dirty = true;
    }
}

#[wasm_bindgen]
pub async fn start(canvas_id: String, css_w: f64, css_h: f64, dpr: f64) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let doc = web_sys::window().unwrap().document().unwrap();
    let canvas: web_sys::HtmlCanvasElement = doc
        .get_element_by_id(&canvas_id)
        .ok_or("canvas not found")?
        .dyn_into()?;
    canvas.set_width((css_w * dpr) as u32);
    canvas.set_height((css_h * dpr) as u32);

    let renderer = Renderer::new(canvas).await.map_err(|e| JsValue::from_str(&e))?;

    // 3D gets the most room; a short Life band means more of Life's activity
    // reaches the 3D floor, sooner (with 20% of an 800px screen, the first
    // seed arrives in ~9s rather than ~18s, with no long lulls after)
    let b1_frac = 0.52;
    let b2_frac = 0.72;
    let width = ((css_w / CELL_PX) as usize).max(16);
    let rows30 = (((1.0 - b2_frac) * css_h / CELL_PX) as usize).max(2);
    let rows_gol = (((b2_frac - b1_frac) * css_h / CELL_PX) as usize).max(2);

    let mut app = App {
        renderer,
        cascade: Cascade::new(width, rows30, rows_gol, 0),
        history: VecDeque::new(),
        paused: false,
        tps: 30.0,
        acc: 0.0,
        last_ms: 0.0,
        view: View::new(Camera::default()),
        css_w,
        css_h,
        dpr,
        b1_frac,
        b2_frac,
        preset_idx: 0,
        dirty: true,
        instances: Vec::new(),
    };
    app.apply_layout();
    app.view = View::new(app.default_camera());
    APP.with(|a| *a.borrow_mut() = Some(app));

    // requestAnimationFrame loop
    let cb: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let cb2 = cb.clone();
    *cb2.borrow_mut() = Some(Closure::new(move |t: f64| {
        with_app(|app| app.frame(t));
        request_frame(cb.borrow().as_ref().unwrap());
    }));
    request_frame(cb2.borrow().as_ref().unwrap());
    Ok(())
}

fn request_frame(c: &Closure<dyn FnMut(f64)>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(c.as_ref().unchecked_ref())
        .unwrap();
}

#[wasm_bindgen]
pub fn toggle_pause() -> bool {
    with_app(|a| {
        a.paused = !a.paused;
        a.acc = 0.0;
        a.paused
    })
    .unwrap_or(true)
}

#[wasm_bindgen]
pub fn is_paused() -> bool {
    with_app(|a| a.paused).unwrap_or(true)
}

#[wasm_bindgen]
pub fn step_forward() {
    with_app(|a| {
        a.paused = true;
        a.tick();
    });
}

#[wasm_bindgen]
pub fn step_backward() {
    with_app(|a| {
        a.paused = true;
        a.step_back();
    });
}

#[wasm_bindgen]
pub fn set_speed(tps: f64) {
    with_app(|a| a.tps = tps.clamp(1.0, 60.0));
}

#[wasm_bindgen]
pub fn reset() {
    with_app(|a| {
        let (w, rows30, rows_gol) = a.grid_dims();
        let c3 = &a.cascade.ca3d;
        let dims = (c3.nx, c3.ny, c3.nz);
        a.cascade = Cascade::new(w, rows30, rows_gol, a.preset_idx);
        a.cascade.ca3d.resize(dims.0, dims.1, dims.2);
        a.history.clear();
        a.dirty = true;
    });
}

/// Camera controls for the 3D region. Order matches `camera_values`.
#[wasm_bindgen]
pub fn set_camera(azimuth: f64, elevation: f64, distance: f64, target_y: f64, fov: f64) {
    with_app(|a| {
        a.view.camera = Camera {
            azimuth_deg: azimuth as f32,
            elevation_deg: elevation as f32,
            distance: distance as f32,
            target_y: target_y as f32,
            fov_deg: fov as f32,
        };
    });
}

fn camera_to_vec(c: Camera) -> Vec<f64> {
    vec![
        c.azimuth_deg as f64,
        c.elevation_deg as f64,
        c.distance as f64,
        c.target_y as f64,
        c.fov_deg as f64,
    ]
}

/// Returns [azimuth, elevation, distance, target_y, fov] for the current camera.
#[wasm_bindgen]
pub fn camera_values() -> Vec<f64> {
    camera_to_vec(with_app(|a| a.view.camera).unwrap_or_default())
}

/// The built-in default camera, in the same order as `camera_values`.
#[wasm_bindgen]
pub fn camera_defaults() -> Vec<f64> {
    camera_to_vec(with_app(|a| a.default_camera()).unwrap_or_default())
}

#[wasm_bindgen]
pub fn preset_count() -> usize {
    PRESETS_3D.len()
}

#[wasm_bindgen]
pub fn preset_name(i: usize) -> String {
    PRESETS_3D[i % PRESETS_3D.len()].name.to_string()
}

#[wasm_bindgen]
pub fn set_preset(i: usize) {
    with_app(|a| {
        a.preset_idx = i % PRESETS_3D.len();
        a.cascade.ca3d.set_preset(PRESETS_3D[a.preset_idx]);
        a.history.clear();
        a.dirty = true;
    });
}

/// Preset details for the JS-built info popups.
#[wasm_bindgen]
pub fn preset_rule_str(i: usize) -> String {
    PRESETS_3D[i % PRESETS_3D.len()].rule_str.to_string()
}

#[wasm_bindgen]
pub fn preset_blurb(i: usize) -> String {
    PRESETS_3D[i % PRESETS_3D.len()].blurb.to_string()
}

#[wasm_bindgen]
pub fn current_preset() -> usize {
    with_app(|a| a.preset_idx).unwrap_or(0)
}

/// Canvas colours from the page theme, as a flat list of 0–1 floats:
/// `[clear rgb, rule30 fg/bg/trail rgb, rule30 depth cue, gol fg/bg/trail rgb,
/// gol depth cue, cube alive/fade_a/fade_b/tint rgb, cube ambient]` (36 values).
#[wasm_bindgen]
pub fn set_palette(v: Vec<f32>) -> Result<(), JsValue> {
    if v.len() != 36 {
        return Err(JsValue::from_str(&format!("set_palette: expected 36 values, got {}", v.len())));
    }
    let rgb = |i: usize| [v[i], v[i + 1], v[i + 2]];
    let region = |i: usize| RegionColors { fg: rgb(i), bg: rgb(i + 3), trail: rgb(i + 6), depth_cue: v[i + 9] };
    let palette = Palette {
        clear: rgb(0),
        rule30: region(3),
        gol: region(13),
        cube_alive: rgb(23),
        cube_fade_a: rgb(26),
        cube_fade_b: rgb(29),
        cube_tint: rgb(32),
        cube_ambient: v[35],
    };
    with_app(|a| a.renderer.set_palette(palette));
    Ok(())
}

/// The 3D grid's current [width, height, depth] in cells.
#[wasm_bindgen]
pub fn grid_size_3d() -> Vec<usize> {
    with_app(|a| vec![a.cascade.ca3d.nx, a.cascade.ca3d.ny, a.cascade.ca3d.nz]).unwrap_or_default()
}

/// Returns [b1, b2] as fractions of the canvas height.
#[wasm_bindgen]
pub fn boundaries() -> Vec<f64> {
    with_app(|a| vec![a.b1_frac, a.b2_frac]).unwrap_or_else(|| vec![0.52, 0.72])
}

#[wasm_bindgen]
pub fn set_boundaries(b1: f64, b2: f64) {
    with_app(|a| {
        a.b1_frac = b1.clamp(MIN_REGION_FRAC, 1.0 - 2.0 * MIN_REGION_FRAC);
        a.b2_frac = b2.clamp(a.b1_frac + MIN_REGION_FRAC, 1.0 - MIN_REGION_FRAC);
        a.apply_layout();
    });
}

#[wasm_bindgen]
pub fn resize(css_w: f64, css_h: f64, dpr: f64) {
    let doc = web_sys::window().unwrap().document().unwrap();
    if let Some(canvas) = doc.get_element_by_id("ca-canvas") {
        if let Ok(canvas) = canvas.dyn_into::<web_sys::HtmlCanvasElement>() {
            canvas.set_width((css_w * dpr) as u32);
            canvas.set_height((css_h * dpr) as u32);
        }
    }
    with_app(|a| {
        a.css_w = css_w;
        a.css_h = css_h;
        a.dpr = dpr;
        a.renderer.resize((css_w * dpr) as u32, (css_h * dpr) as u32);
        a.apply_layout();
    });
}
