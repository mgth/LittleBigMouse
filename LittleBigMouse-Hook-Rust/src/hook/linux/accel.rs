//! Pointer acceleration for the evdev router — a faithful port of libinput's
//! mouse filters (refs/libinput/src/filter-mouse.c, filter-flat.c, filter.c).
//!
//! The router drives an ABSOLUTE virtual device, which deliberately bypasses
//! libinput's acceleration (emitted position == real position, engine and
//! cursor stay locked). The flip side is that the user's pointer feel dies the
//! moment we grab: raw 1:1 deltas replace the adaptive curve (which boosts
//! anything faster than a crawl up to ~2x by default). So we reproduce the
//! curve ourselves on the deltas BEFORE they feed the authoritative cursor —
//! geometry stays exact, feel matches the ungrabbed desktop (#511).
//!
//! Per-device settings come from KDE's `~/.config/kcminputrc`
//! (`[Libinput][<vid>][<pid>][<name>]`, keys `PointerAcceleration` -1..1 and
//! `PointerAccelerationProfile` using libinput enum values 1=flat 2=adaptive).
//! No entry / no file (GNOME, bare compositors) falls back to libinput's own
//! default: adaptive profile, speed 0. Deltas are assumed 1000 dpi (libinput's
//! normalization baseline; it only deviates when a udev MOUSE_DPI quirk says
//! so, which we don't read).
//!
//! Everything here is pure math on the routing thread — no I/O, no locks
//! (see the module rule in evdev.rs). Config is read once, at arm time.

use std::path::Path;

/// libinput filter-mouse.c constants, in units/µs (1000dpi-normalized).
const DEFAULT_THRESHOLD: f64 = 0.4 / 1000.0; // v_usec_from_millis(0.4)
const MINIMUM_THRESHOLD: f64 = 0.2 / 1000.0;
const DEFAULT_ACCELERATION: f64 = 2.0;
const DEFAULT_INCLINE: f64 = 1.1;
/// filter.c: events older than this no longer contribute to velocity.
const MOTION_TIMEOUT_US: u64 = 1_000_000;
/// filter.c trackers_velocity: max deviation from the initial velocity.
const MAX_VELOCITY_DIFF: f64 = 1.0 / 1000.0; // v_usec_from_millis(1)
/// Mice don't use velocity averaging (no quirk): 2 trackers, like libinput.
const NTRACKERS: usize = 2;

const UNDEFINED_DIRECTION: u32 = 0xff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Raw deltas (the pre-#511 behavior), for debugging via LBM_EVDEV_ACCEL=none.
    None,
    /// Constant factor, no curve.
    Flat,
    /// libinput's default "linear" double-incline curve.
    Adaptive,
}

/// libinput-private.h xy_get_direction: one or two of 8 octant bits.
fn xy_get_direction(x: f64, y: f64) -> u32 {
    // N=1 NE=2 E=4 SE=8 S=16 SW=32 W=64 NW=128
    if x.abs() < 2.0 && y.abs() < 2.0 {
        if x > 0.0 && y > 0.0 { 16 | 8 | 4 }          // S | SE | E
        else if x > 0.0 && y < 0.0 { 1 | 2 | 4 }      // N | NE | E
        else if x < 0.0 && y > 0.0 { 16 | 32 | 64 }   // S | SW | W
        else if x < 0.0 && y < 0.0 { 1 | 128 | 64 }   // N | NW | W
        else if x > 0.0 { 2 | 4 | 8 }                 // NE | E | SE
        else if x < 0.0 { 128 | 64 | 32 }             // NW | W | SW
        else if y > 0.0 { 8 | 16 | 32 }               // SE | S | SW
        else if y < 0.0 { 2 | 1 | 128 }               // NE | N | NW
        else { UNDEFINED_DIRECTION }
    } else {
        let mut r = y.atan2(x);
        r = (r + 2.5 * std::f64::consts::PI) % (2.0 * std::f64::consts::PI);
        r *= 4.0 * std::f64::consts::FRAC_1_PI;
        let d1 = ((r + 0.9) as i32) % 8;
        let d2 = ((r + 0.1) as i32) % 8;
        (1u32 << d1) | (1u32 << d2)
    }
}

#[derive(Clone, Copy, Default)]
struct Tracker {
    dx: f64,
    dy: f64,
    time: u64, // µs
    dir: u32,
}

/// One accelerator per grabbed mouse (velocities must not mix across devices).
pub struct PointerAccel {
    profile: Profile,
    // Adaptive parameters, derived from the -1..1 speed setting.
    threshold: f64,
    accel: f64,
    incline: f64,
    // Flat factor.
    flat_factor: f64,
    trackers: [Tracker; NTRACKERS],
    cur: usize,
    last_velocity: f64,
}

impl PointerAccel {
    pub fn new(profile: Profile, speed: f64) -> Self {
        let speed = speed.clamp(-1.0, 1.0);
        // filter-mouse.c accelerator_set_speed ("trial-and-error magic").
        let threshold = (DEFAULT_THRESHOLD - 0.25 / 1000.0 * speed).max(MINIMUM_THRESHOLD);
        let accel = DEFAULT_ACCELERATION + speed * 1.5;
        let incline = DEFAULT_INCLINE + speed * 0.75;
        // filter-flat.c: 0..200% of nominal speed.
        let flat_factor = (1.0 + speed).max(0.005);
        PointerAccel {
            profile,
            threshold,
            accel,
            incline,
            flat_factor,
            trackers: [Tracker { dir: UNDEFINED_DIRECTION, ..Default::default() }; NTRACKERS],
            cur: 0,
            last_velocity: 0.0,
        }
    }

    /// filter-mouse.c pointer_accel_profile_linear.
    fn profile_factor(&self, speed_in: f64) -> f64 {
        let speed_ms = speed_in * 1000.0; // v_us2ms
        let factor = if speed_ms < 0.07 {
            10.0 * speed_ms + 0.3
        } else if speed_in < self.threshold {
            1.0
        } else {
            self.incline * (speed_in - self.threshold) * 1000.0 + 1.0
        };
        factor.min(self.accel)
    }

    fn tracker(&self, offset: usize) -> &Tracker {
        &self.trackers[(self.cur + NTRACKERS - offset) % NTRACKERS]
    }

    /// filter.c trackers_feed.
    fn feed(&mut self, dx: f64, dy: f64, time: u64) {
        for t in self.trackers.iter_mut() {
            t.dx += dx;
            t.dy += dy;
        }
        self.cur = (self.cur + 1) % NTRACKERS;
        self.trackers[self.cur] =
            Tracker { dx: 0.0, dy: 0.0, time, dir: xy_get_direction(dx, dy) };
    }

    fn tracker_velocity(t: &Tracker, time: u64) -> f64 {
        let tdelta = time.saturating_sub(t.time) + 1;
        (t.dx.hypot(t.dy)) / tdelta as f64 // units/µs
    }

    /// filter.c trackers_velocity (no smoothener: mice don't have one).
    fn velocity(&self, time: u64) -> f64 {
        let mut result = 0.0;
        let mut initial_velocity = 0.0;
        let mut dir = self.tracker(0).dir;

        for offset in 1..NTRACKERS {
            let t = self.tracker(offset);
            if t.time > time {
                break; // time running backwards
            }
            if time - t.time > MOTION_TIMEOUT_US {
                if offset == 1 {
                    // First movement after a pause: err on the fast side.
                    result = Self::tracker_velocity(t, t.time + MOTION_TIMEOUT_US);
                }
                break;
            }
            let velocity = Self::tracker_velocity(t, time);
            dir &= t.dir;
            if dir == 0 {
                if offset == 1 {
                    result = velocity;
                }
                break;
            }
            if initial_velocity == 0.0 || offset <= 2 {
                initial_velocity = velocity;
                result = velocity;
            } else if (initial_velocity - velocity).abs() > MAX_VELOCITY_DIFF {
                break;
            } else {
                result = velocity;
            }
        }
        result
    }

    /// Accelerate one motion frame. `time` is a monotonic µs clock.
    /// Returns the factored delta (unitless factor applied to both axes).
    pub fn apply(&mut self, dx: f64, dy: f64, time: u64) -> (f64, f64) {
        match self.profile {
            Profile::None => (dx, dy),
            Profile::Flat => (dx * self.flat_factor, dy * self.flat_factor),
            Profile::Adaptive => {
                self.feed(dx, dy, time);
                let velocity = self.velocity(time);
                // filter.c calculate_acceleration_simpsons.
                let factor = (self.profile_factor(velocity)
                    + self.profile_factor(self.last_velocity)
                    + 4.0 * self.profile_factor((self.last_velocity + velocity) / 2.0))
                    / 6.0;
                self.last_velocity = velocity;
                (dx * factor, dy * factor)
            }
        }
    }
}

/// Per-device acceleration settings resolved at arm time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccelSettings {
    pub profile: Profile,
    pub speed: f64,
}

impl Default for AccelSettings {
    /// libinput's default for mice: adaptive, speed 0.
    fn default() -> Self {
        AccelSettings { profile: Profile::Adaptive, speed: 0.0 }
    }
}

/// The parsed `kcminputrc` [Libinput] sections plus env overrides.
pub struct AccelConfig {
    /// (vendor, product, name) -> settings, from kcminputrc.
    devices: Vec<((u16, u16, String), AccelSettings)>,
    /// LBM_EVDEV_ACCEL / LBM_EVDEV_ACCEL_SPEED force a global answer.
    force: Option<AccelSettings>,
}

impl AccelConfig {
    /// Read once at arm time (file I/O is forbidden on the routing thread).
    pub fn load() -> AccelConfig {
        let force_profile = std::env::var("LBM_EVDEV_ACCEL").ok().and_then(|v| {
            match v.to_ascii_lowercase().as_str() {
                "none" => Some(Profile::None),
                "flat" => Some(Profile::Flat),
                "adaptive" => Some(Profile::Adaptive),
                _ => None,
            }
        });
        let force_speed = std::env::var("LBM_EVDEV_ACCEL_SPEED")
            .ok()
            .and_then(|v| v.parse::<f64>().ok());
        let force = match (force_profile, force_speed) {
            (None, None) => None,
            (p, s) => Some(AccelSettings {
                profile: p.unwrap_or(Profile::Adaptive),
                speed: s.unwrap_or(0.0).clamp(-1.0, 1.0),
            }),
        };

        let home = std::env::var("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                Path::new(&std::env::var("HOME").unwrap_or_default()).join(".config")
            });
        let devices = std::fs::read_to_string(home.join("kcminputrc"))
            .map(|s| Self::parse_kcminputrc(&s))
            .unwrap_or_default();

        AccelConfig { devices, force }
    }

    /// Resolve the settings for one grabbed device.
    pub fn for_device(&self, vendor: u16, product: u16, name: &str) -> AccelSettings {
        if let Some(f) = self.force {
            return f;
        }
        self.devices
            .iter()
            .find(|((v, p, n), _)| *v == vendor && *p == product && n == name)
            // Name drifts between kernel and KDE on some receivers: fall back
            // to a vid/pid-only match before giving up.
            .or_else(|| {
                self.devices.iter().find(|((v, p, _), _)| *v == vendor && *p == product)
            })
            .map(|(_, s)| *s)
            .unwrap_or_default()
    }

    /// KDE per-device sections look like:
    /// `[Libinput][1133][50503][Logitech USB Receiver Mouse]`
    /// with `PointerAcceleration=0.2` (string float, -1..1) and
    /// `PointerAccelerationProfile=1|2` (libinput enum: 1 flat, 2 adaptive).
    fn parse_kcminputrc(content: &str) -> Vec<((u16, u16, String), AccelSettings)> {
        let mut out: Vec<((u16, u16, String), AccelSettings)> = Vec::new();
        let mut current: Option<(u16, u16, String)> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                current = None;
                let parts: Vec<&str> = line
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split("][")
                    .collect();
                if parts.len() == 4 && parts[0] == "Libinput" {
                    if let (Ok(v), Ok(p)) = (parts[1].parse::<u16>(), parts[2].parse::<u16>()) {
                        current = Some((v, p, parts[3].to_string()));
                    }
                }
                continue;
            }
            let Some(key) = current.clone() else { continue };
            let Some((k, v)) = line.split_once('=') else { continue };
            let entry = match out.iter_mut().find(|(id, _)| *id == key) {
                Some(e) => e,
                None => {
                    out.push((key, AccelSettings::default()));
                    out.last_mut().unwrap()
                }
            };
            match k.trim() {
                "PointerAcceleration" => {
                    if let Ok(s) = v.trim().parse::<f64>() {
                        entry.1.speed = s.clamp(-1.0, 1.0);
                    }
                }
                "PointerAccelerationProfile" => {
                    entry.1.profile = match v.trim() {
                        "1" => Profile::Flat,
                        _ => Profile::Adaptive,
                    };
                }
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adaptive0() -> PointerAccel {
        PointerAccel::new(Profile::Adaptive, 0.0)
    }

    /// Reference values computed from filter-mouse.c with speed 0:
    /// threshold 0.4u/ms, accel 2.0, incline 1.1.
    #[test]
    fn adaptive_profile_matches_libinput_reference() {
        let a = adaptive0();
        // Deceleration zone: 0.05 u/ms -> 10*0.05 + 0.3 = 0.8
        assert!((a.profile_factor(0.05 / 1000.0) - 0.8).abs() < 1e-9);
        // Plateau: 0.2 u/ms -> 1.0
        assert!((a.profile_factor(0.2 / 1000.0) - 1.0).abs() < 1e-9);
        // Incline: 1.0 u/ms -> 1.1*(1.0-0.4)+1 = 1.66
        assert!((a.profile_factor(1.0 / 1000.0) - 1.66).abs() < 1e-9);
        // Cap: 10 u/ms -> min(2.0, ...) = 2.0
        assert!((a.profile_factor(10.0 / 1000.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn speed_setting_shifts_the_curve() {
        let fast = PointerAccel::new(Profile::Adaptive, 1.0);
        // threshold = 0.4-0.25 = 0.15 u/ms, below the 0.2 minimum -> clamped
        assert!((fast.threshold - MINIMUM_THRESHOLD).abs() < 1e-12);
        assert!((fast.accel - 3.5).abs() < 1e-9);
        assert!((fast.incline - 1.85).abs() < 1e-9);
        let slow = PointerAccel::new(Profile::Adaptive, -1.0);
        // Negative speed raises the threshold: accel kicks in later.
        assert!((slow.threshold - 0.65 / 1000.0).abs() < 1e-12);
        assert!((slow.accel - 0.5).abs() < 1e-9);
    }

    #[test]
    fn flat_profile_is_a_constant_factor() {
        let mut a = PointerAccel::new(Profile::Flat, 0.5);
        assert_eq!(a.apply(10.0, -4.0, 0), (15.0, -6.0));
        let mut slow = PointerAccel::new(Profile::Flat, -1.0);
        // filter-flat.c floors the factor at 0.005
        let (x, _) = slow.apply(1000.0, 0.0, 0);
        assert!((x - 5.0).abs() < 1e-9);
    }

    #[test]
    fn none_profile_passes_through() {
        let mut a = PointerAccel::new(Profile::None, 0.7);
        assert_eq!(a.apply(3.0, 4.0, 123), (3.0, 4.0));
    }

    /// A steady fast drag must end up accelerated (~2x by default), a crawl
    /// decelerated — the two ends of the double-incline.
    #[test]
    fn steady_motion_converges_on_the_curve() {
        let mut a = adaptive0();
        let mut t = 0u64;
        let mut last = (0.0, 0.0);
        for _ in 0..50 {
            t += 8_000; // 125 Hz frames
            last = a.apply(16.0, 0.0, t); // 2 u/ms: brisk swipe
        }
        assert!((last.0 / 16.0 - 2.0).abs() < 0.01, "fast swipe caps at 2x, got {}", last.0 / 16.0);

        let mut a = adaptive0();
        let mut t = 0u64;
        for _ in 0..50 {
            t += 20_000;
            last = a.apply(1.0, 0.0, t); // 0.05 u/ms: crawl
        }
        assert!(last.0 / 1.0 < 0.85, "crawl decelerates, got {}", last.0);
    }

    /// After a pause longer than MOTION_TIMEOUT the stale velocity must not
    /// keep the pointer accelerated. Reference semantics: libinput never
    /// resets last_velocity on a pause, so the FIRST frame is still Simpson-
    /// averaged with the pre-pause speed — the crawl settles from the second
    /// frame on.
    #[test]
    fn pause_resets_velocity() {
        let mut a = adaptive0();
        let mut t = 0u64;
        for _ in 0..20 {
            t += 8_000;
            a.apply(16.0, 0.0, t); // 2 u/ms: factor pinned at the 2.0 cap
        }
        t += 5_000_000; // 5s pause, then a crawl
        let (first, _) = a.apply(1.0, 0.0, t);
        assert!(first < 2.0, "post-pause frame must leave the cap, got {first}");
        t += 20_000;
        let (second, _) = a.apply(1.0, 0.0, t);
        assert!(second < 0.85, "second crawl frame must decelerate, got {second}");
    }

    #[test]
    fn kcminputrc_sections_are_parsed() {
        let content = "\
[Keyboard]\nRepeatDelay=600\n\n\
[Libinput][1133][50503][Logitech USB Receiver Mouse]\n\
PointerAcceleration=0.2\nPointerAccelerationProfile=1\n\n\
[Libinput][6127][24647][PixArt Dell MS116 USB Optical Mouse]\n\
PointerAcceleration=-0.4\n";
        let cfg = AccelConfig { devices: AccelConfig::parse_kcminputrc(content), force: None };

        let s = cfg.for_device(1133, 50503, "Logitech USB Receiver Mouse");
        assert_eq!(s.profile, Profile::Flat);
        assert!((s.speed - 0.2).abs() < 1e-9);

        // Name mismatch still matches on vid/pid.
        let s = cfg.for_device(6127, 24647, "kernel-side name");
        assert_eq!(s.profile, Profile::Adaptive);
        assert!((s.speed + 0.4).abs() < 1e-9);

        // Unknown device: libinput defaults.
        assert_eq!(cfg.for_device(1, 2, "nope"), AccelSettings::default());
    }
}
