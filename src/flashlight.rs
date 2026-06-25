use crate::config::FlashlightConfig;

const FL_STEP: f32 = 40.0;
const FL_MIN_RADIUS: f32 = 20.0;
const FL_MAX_RADIUS: f32 = 4000.0;
const FL_LERP: f32 = 10.0;

#[derive(Clone, Debug)]
pub struct Flashlight {
    pub active: bool,
    pub target_radius: f32,
    pub max_shadow: f32,
    pub radius: f32,
}

impl Flashlight {
    pub fn from_config(cfg: &FlashlightConfig) -> Self {
        Self {
            active: cfg.enabled,
            target_radius: cfg.radius,
            max_shadow: cfg.shadow,
            radius: cfg.radius,
        }
    }

    pub fn restart(&mut self, cfg: &FlashlightConfig, offscreen_radius: f32) {
        *self = Self::from_config(cfg);
        if self.active {
            self.radius = offscreen_radius;
        }
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn toggle(&mut self, offscreen_radius: f32) {
        self.active = !self.active;
        if self.active {
            self.radius = offscreen_radius;
        }
    }

    pub fn adjust_radius(&mut self, grow: bool) {
        if !self.active {
            return;
        }
        let delta = if grow { FL_STEP } else { -FL_STEP };
        self.target_radius = (self.target_radius + delta).clamp(FL_MIN_RADIUS, FL_MAX_RADIUS);
    }

    pub fn update(&mut self, dt: f32, offscreen_radius: f32) {
        let target = if self.active {
            self.target_radius
        } else {
            offscreen_radius
        };
        let t = 1.0 - (-FL_LERP * dt).exp();
        self.radius += (target - self.radius) * t;
        if (target - self.radius).abs() < 0.5 {
            self.radius = target;
        }
    }

    pub fn visible(&self, offscreen_radius: f32) -> bool {
        self.active || self.radius < offscreen_radius - 0.5
    }
}
