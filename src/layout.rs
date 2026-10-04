//! Shaping the 3D box and its camera to the band it's drawn in. Kept free
//! of wasm/GPU types so it can be tested on the host.
use crate::sim::ca3d::CELL_BUDGET;

/// Depth of the 3D box in cells. Width and height follow the band's shape.
pub const DEPTH_3D: usize = 32;

/// Static orbit camera for the 3D region. The target sits at the centre of
/// the X/Z footprint at height `target_y`; the eye orbits it on a sphere.
/// `azimuth_deg` 0 looks along +X; `elevation_deg` lifts the eye above the
/// horizontal plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub distance: f32,
    pub target_y: f32,
    pub fov_deg: f32,
}

impl Default for Camera {
    fn default() -> Self {
        framing_camera(dims_3d(1280.0, 416.0), 1280.0 / 416.0)
    }
}

/// 3D grid dimensions for a band `w`×`h` CSS pixels: the box takes the
/// band's aspect ratio, as many cells as `CELL_BUDGET` allows.
pub fn dims_3d(w: f64, h: f64) -> (usize, usize, usize) {
    let area = (CELL_BUDGET / DEPTH_3D) as f64; // nx * ny
    let aspect = (w / h.max(1.0)).clamp(0.2, 12.0);
    let ny = ((area / aspect).sqrt() as usize).max(12);
    let nx = ((area / ny as f64) as usize).max(12);
    (nx, ny, DEPTH_3D)
}

/// A camera that looks squarely at the box's front face and pulls back
/// just far enough to fit it into a viewport of the given aspect ratio.
pub fn framing_camera((nx, ny, nz): (usize, usize, usize), aspect: f64) -> Camera {
    let fov = 20.0f32;
    let tv = (fov.to_radians() / 2.0).tan();
    let th = tv * aspect.max(0.05) as f32;
    let fit_w = nx as f32 / 2.0 / th;
    let fit_h = ny as f32 / 2.0 / tv;
    Camera {
        azimuth_deg: 90.0,
        elevation_deg: 10.0,
        distance: fit_w.max(fit_h) + nz as f32 / 2.0,
        target_y: ny as f32 / 2.0,
        fov_deg: fov,
    }
}

/// The camera in use plus the framing it's measured against. The user can
/// orbit and zoom freely; when the band is reshaped, distance and target
/// height scale by how much the *framing* changed, so a zoom-in stays the
/// same zoom-in. Scaling against the remembered framing (not one
/// recomputed after the band has already changed) makes any sequence of
/// resizes exactly reversible instead of compounding.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub camera: Camera,
    frame: Camera,
}

impl View {
    pub fn new(frame: Camera) -> Self {
        Self { camera: frame, frame }
    }

    /// The framing for the current band: what "Reset camera" returns to.
    pub fn frame(&self) -> Camera {
        self.frame
    }

    pub fn reframe(&mut self, frame: Camera) {
        self.camera.distance *= frame.distance / self.frame.distance;
        self.camera.target_y *= frame.target_y / self.frame.target_y;
        self.frame = frame;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_for(w: f64, h: f64) -> Camera {
        framing_camera(dims_3d(w, h), w / h)
    }

    const SIZES: [(f64, f64); 7] =
        [(1000.0, 364.0), (1400.0, 312.0), (800.0, 468.0), (1600.0, 260.0), (600.0, 416.0), (390.0, 439.0), (1280.0, 416.0)];

    #[test]
    fn resizing_round_trip_returns_to_the_same_camera() {
        let mut v = View::new(frame_for(1280.0, 416.0));
        let start = v.camera;
        for _ in 0..50 {
            for (w, h) in SIZES {
                v.reframe(frame_for(w, h));
            }
        }
        assert!((v.camera.distance - start.distance).abs() < 1e-2, "{:?} vs {:?}", v.camera, start);
        assert!((v.camera.target_y - start.target_y).abs() < 1e-2);
    }

    #[test]
    fn untouched_camera_always_matches_the_framing() {
        let mut v = View::new(frame_for(1280.0, 416.0));
        for (w, h) in SIZES {
            v.reframe(frame_for(w, h));
            assert!((v.camera.distance - v.frame().distance).abs() < 1e-3);
            assert!((v.camera.target_y - v.frame().target_y).abs() < 1e-3);
        }
    }

    #[test]
    fn a_users_zoom_survives_resizes() {
        let mut v = View::new(frame_for(1280.0, 416.0));
        v.camera.distance *= 0.5; // zoomed in 2x
        v.camera.azimuth_deg = 30.0;
        for (w, h) in SIZES {
            v.reframe(frame_for(w, h));
            assert!((v.camera.distance / v.frame().distance - 0.5).abs() < 1e-4);
            assert_eq!(v.camera.azimuth_deg, 30.0);
        }
    }
}
