//! Mouse traversal engine — faithful port of `Engine/MouseEngine`.
//!
//! The algorithm is pure: every OS interaction goes through [`CursorEnv`], so the
//! whole thing runs deterministically under a fake cursor in tests — the primary
//! defense against silent floating-point parity regressions.

pub mod cursor;
pub mod event;

use cursor::CursorEnv;
use event::MouseEventArg;

use crate::geometry::{Point, Rect, Segment, Side};
use crate::zones::zone_link::{at_physical_index, at_pixel_index};
use crate::zones::{Algorithm, ZoneId, ZonesLayout};

/// Which per-move handler is active (C++ `_onMouseMoveFunc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    ExtFirst,
    Straight,
    Cross,
}

/// Identity of a zone-border link, replacing the C++ `const ZoneLink*` pointer
/// used to detect "same border as last event" for resistance tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResistanceKey {
    zone: ZoneId,
    side: Side,
    index: usize,
}

pub struct MouseEngine {
    pub layout: ZonesLayout,

    old_point: Point<i32>,
    old_zone: Option<ZoneId>,
    old_clip_rect: Rect<i32>,
    mode: Mode,

    current_resistance: Option<ResistanceKey>,
    border_resistance: f64,
    border_resistance_px: i32,

    was_freelook: bool,
    last_freelook_check: u64,
}

impl MouseEngine {
    pub fn new() -> Self {
        MouseEngine {
            layout: ZonesLayout::default(),
            old_point: Point::default(),
            old_zone: None,
            old_clip_rect: Rect::empty(),
            mode: Mode::ExtFirst,
            current_resistance: None,
            border_resistance: 0.0,
            border_resistance_px: 0,
            was_freelook: false,
            last_freelook_check: 0,
        }
    }

    /// C++ `MouseEngine::Layout.Load`. Replacing the layout drops the old arena;
    /// `reset` then clears `old_zone` so no stale `ZoneId` is used before the next
    /// `ExtFirst` re-resolves it (the C++ left `_oldZone` dangling here).
    pub fn load(&mut self, layout: ZonesLayout) {
        self.layout = layout;
        self.reset();
    }

    /// C++ `MouseEngine::Reset`.
    fn reset(&mut self) {
        self.mode = Mode::ExtFirst;
        self.old_zone = None;
    }

    // --- clip save/restore ---------------------------------------------------

    fn save_clip(&mut self, env: &impl CursorEnv) {
        self.old_clip_rect = env.get_clip();
    }

    fn reset_clip(&mut self, env: &mut impl CursorEnv) {
        if !self.old_clip_rect.is_empty() {
            env.set_clip(self.old_clip_rect);
            self.old_clip_rect = Rect::empty();
        }
    }

    // --- entry: freelook gate + dispatch (C++ MouseEngine::OnMouseMove) -------

    pub fn on_mouse_move<E: CursorEnv>(&mut self, env: &mut E, e: &mut MouseEventArg) {
        let check_freelook = if !self.layout.freelook_enabled {
            // detection switched off (#502): resume immediately if it had fired
            self.was_freelook = false;
            false
        } else if self.was_freelook {
            env.tick_count().wrapping_sub(self.last_freelook_check)
                >= self.layout.freelook_check_interval_ms as u64
        } else if self.mode == Mode::ExtFirst || self.old_zone.is_none() {
            true
        } else {
            let bounds = self.layout.arena[self.old_zone.unwrap()].pixels_bounds();
            e.point.x() <= bounds.left()
                || e.point.x() >= bounds.right() - 1
                || e.point.y() <= bounds.top()
                || e.point.y() >= bounds.bottom() - 1
        };

        if check_freelook {
            self.last_freelook_check = env.tick_count();
            let freelook = self.is_freelook_active(env);
            if freelook != self.was_freelook {
                if freelook {
                    // Entering freelook: hand the game a clean cursor environment.
                    self.reset_clip(env);
                    self.reset();
                }
                self.was_freelook = freelook;
            }
        }

        if self.was_freelook {
            e.handled = false;
        } else {
            match self.mode {
                Mode::ExtFirst => self.on_mouse_move_ext_first(env, e),
                Mode::Straight => self.on_mouse_move_straight(env, e),
                Mode::Cross => self.on_mouse_move_cross(env, e),
            }
        }
    }

    fn is_freelook_active(&self, env: &impl CursorEnv) -> bool {
        if env.cursor_hidden() {
            return true;
        }
        // Skip the clip check when LBM itself set the clip.
        if self.old_clip_rect.is_empty() && env.clip_is_subrect_of_virtual_screen() {
            return true;
        }
        false
    }

    /// C++ `CheckForStopped`.
    fn check_for_stopped(&mut self, env: &mut impl CursorEnv, e: &MouseEventArg) -> bool {
        self.reset_clip(env);
        if e.running {
            return false;
        }
        self.mode = Mode::ExtFirst;
        true
    }

    // --- ExtFirst ------------------------------------------------------------

    fn on_mouse_move_ext_first(&mut self, env: &mut impl CursorEnv, e: &mut MouseEventArg) {
        if self.check_for_stopped(env, e) {
            return;
        }
        self.old_point = e.point;
        self.old_zone = self.layout.containing_pixel(self.old_point);
        if self.old_zone.is_none() {
            return;
        }
        self.mode = match self.layout.algorithm {
            Algorithm::Strait => Mode::Straight,
            Algorithm::CornerCrossing => Mode::Cross,
        };
    }

    // --- Strait --------------------------------------------------------------

    fn on_mouse_move_straight(&mut self, env: &mut impl CursorEnv, e: &mut MouseEventArg) {
        if self.check_for_stopped(env, e) {
            return;
        }
        let p_in = e.point;
        let old_zone = self.old_zone.unwrap();
        let bounds = self.layout.arena[old_zone].pixels_bounds();

        // C++ order: right, left, bottom, top — first matching border wins.
        //
        // Deliberate deviation from the C++ (`dist >= 0`): a crossing fires only
        // once the candidate leaves the zone (`dist > 0`). The C++ threshold made
        // the zone's own edge columns (left(), right()-1) trigger at distance 0 —
        // but those columns are exactly where crossings LAND, so a landed cursor
        // re-crossed backwards on the next event even for a purely tangential
        // (vertical) move. Invisible between same-DPI monitors (the y remap is
        // near identity), a violent ping-pong between mismatched ones.
        let right_dist = 1 + p_in.x() - bounds.right();
        if right_dist > 0 {
            self.strait_cross(env, e, old_zone, Side::Right, p_in.y(), right_dist);
            return;
        }
        let left_dist = bounds.left() - p_in.x();
        if left_dist > 0 {
            self.strait_cross(env, e, old_zone, Side::Left, p_in.y(), left_dist);
            return;
        }
        let bottom_dist = 1 + p_in.y() - bounds.bottom();
        if bottom_dist > 0 {
            self.strait_cross(env, e, old_zone, Side::Bottom, p_in.x(), bottom_dist);
            return;
        }
        let top_dist = bounds.top() - p_in.y();
        if top_dist > 0 {
            self.strait_cross(env, e, old_zone, Side::Top, p_in.x(), top_dist);
            return;
        }

        // No border crossed: pass the event through. Re-arm the resistance only
        // once the cursor actually leaves the edge columns: while a resisted push
        // pins the cursor against the border, the tangential (y) frames land here
        // as plain interior moves — with the `dist > 0` thresholds above there are
        // no 0-distance "attempts" keeping the link alive anymore, so resetting
        // unconditionally would restart a half-drained resistance every pixel.
        if self.current_resistance.is_some()
            && p_in.x() > bounds.left()
            && p_in.x() < bounds.right() - 1
            && p_in.y() > bounds.top()
            && p_in.y() < bounds.bottom() - 1
        {
            self.current_resistance = None;
        }
        self.old_point = p_in;
        e.handled = false;
    }

    /// One Strait border crossing: resolve the link, apply resistance, and either
    /// move the cursor into the target zone or stick it to the border.
    fn strait_cross(
        &mut self,
        env: &mut impl CursorEnv,
        e: &mut MouseEventArg,
        zone: ZoneId,
        side: Side,
        coord: i32,
        dist: i32,
    ) {
        // Extract everything from the arena before we mutate resistance state.
        let links = self.side_links(zone, side);
        let Some(index) = at_pixel_index(links, coord) else {
            // No links on this side (malformed layout): can't cross here.
            self.no_zone_matches(env, e);
            return;
        };
        let link = &links[index];
        let resistance_px = link.border_resistance_px;

        // Resolve the target BEFORE mapping the coordinate: the catch-all no-target link carries
        // i32::MIN/MAX sentinel bounds, so mapping through it is both meaningless and (pre-fix) an
        // overflow. A `None` target means the cursor hit an edge with no neighbour → confine.
        let Some(target) = link.target else {
            self.no_zone_matches(env, e);
            return;
        };
        let to_target = link.to_target_pixel(coord);

        let key = ResistanceKey { zone, side, index };
        if !self.try_pass_border_pixel(env, key, resistance_px, dist) {
            self.no_zone_matches(env, e);
            return;
        }

        let tb = self.layout.arena[target].pixels_bounds();
        let p_out = match side {
            Side::Right => Point::new(tb.left(), to_target),
            Side::Left => Point::new(tb.right() - 1, to_target),
            Side::Bottom => Point::new(to_target, tb.top()),
            Side::Top => Point::new(to_target, tb.bottom() - 1),
            Side::None => unreachable!(),
        };
        self.move_cursor(env, e, p_out, target);
    }

    // --- Cross ---------------------------------------------------------------

    fn on_mouse_move_cross(&mut self, env: &mut impl CursorEnv, e: &mut MouseEventArg) {
        if self.check_for_stopped(env, e) {
            return;
        }
        let old_zone = self.old_zone.unwrap();

        let bounds = self.layout.arena[old_zone].pixels_bounds();
        if bounds.contains(e.point) {
            // Same guard as Straight (see on_mouse_move_straight): while a
            // resisted push pins the cursor against the border, tangential and
            // zero-dx frames land here on the edge columns — resetting there
            // re-armed a half-drained resistance every frame and turned any
            // resisted border into a wall ("adhesion" feel in Cross mode).
            if self.current_resistance.is_some()
                && e.point.x() > bounds.left()
                && e.point.x() < bounds.right() - 1
                && e.point.y() > bounds.top()
                && e.point.y() < bounds.bottom() - 1
            {
                self.current_resistance = None;
            }
            self.old_point = e.point;
            e.handled = false;
            return;
        }

        let p_in_mm_old = self.layout.arena[old_zone].to_physical(self.old_point);
        let p_in_mm = self.layout.arena[old_zone].to_physical(e.point);
        let trip = Segment::new(p_in_mm_old, p_in_mm);
        let min_dist_squared = self.layout.max_travel_distance_squared;

        let (zone_out, p_out_in_mm) = self.find_target_zone(Some(old_zone), trip, min_dist_squared);
        if let Some(zone_out) = zone_out {
            if self.try_pass_border_cross(env, old_zone, trip) {
                self.move_in_mm(env, e, p_out_in_mm, zone_out);
            } else {
                self.no_zone_matches(env, e);
            }
            return;
        }

        if self.layout.loop_x {
            let width = self.layout.width();
            let shifted = if trip.b().x() < trip.a().x() {
                Some(trip + Point::new(width, 0.0))
            } else if trip.b().x() > trip.a().x() {
                Some(trip - Point::new(width, 0.0))
            } else {
                None
            };
            if let Some(shifted) = shifted {
                let (zone_out, p_out) = self.find_target_zone(None, shifted, min_dist_squared);
                if let Some(zone_out) = zone_out {
                    if self.try_pass_border_cross(env, old_zone, trip) {
                        self.move_in_mm(env, e, p_out, zone_out);
                    } else {
                        self.no_zone_matches(env, e);
                    }
                    return;
                }
            }
        }

        if self.layout.loop_y {
            let height = self.layout.height();
            let shifted = if trip.b().y() < trip.a().y() {
                Some(trip + Point::new(0.0, height))
            } else if trip.b().y() > trip.a().y() {
                Some(trip - Point::new(0.0, height))
            } else {
                None
            };
            if let Some(shifted) = shifted {
                let (zone_out, p_out) = self.find_target_zone(None, shifted, min_dist_squared);
                if let Some(zone_out) = zone_out {
                    if self.try_pass_border_cross(env, old_zone, trip) {
                        self.move_in_mm(env, e, p_out, zone_out);
                    } else {
                        self.no_zone_matches(env, e);
                    }
                    return;
                }
            }
        }

        self.no_zone_matches(env, e);
    }

    /// C++ `FindTargetZone`: the zone whose border the mouse direction crosses at
    /// the shortest travel, plus the exit point (mm).
    fn find_target_zone(
        &self,
        current: Option<ZoneId>,
        trip: Segment<f64>,
        min_dist_squared: f64,
    ) -> (Option<ZoneId>, Point<f64>) {
        let mut zone_out: Option<ZoneId> = None;
        let mut p_out_in_mm = trip.b();
        let mut min_dist_squared = min_dist_squared;
        let trip_line = trip.line();

        for &zid in &self.layout.zones {
            if current == Some(zid) {
                continue;
            }
            let zone = &self.layout.arena[zid];

            if zone.contains_mm(trip.b()) {
                zone_out = Some(zid);
                min_dist_squared = 0.0;
                continue;
            }

            for p in zone.physical_inside().intersect(&trip_line) {
                // Reject intersections that lie against the direction of travel.
                if p.x() < trip.a().x() {
                    if trip.b().x() > trip.a().x() {
                        continue;
                    }
                } else if trip.b().x() < trip.a().x() {
                    continue;
                }
                if p.y() < trip.a().y() {
                    if trip.b().y() > trip.a().y() {
                        continue;
                    }
                } else if trip.b().y() < trip.a().y() {
                    continue;
                }

                let dx = p.x() - trip.b().x();
                let dy = p.y() - trip.b().y();
                let dist = dx * dx + dy * dy;

                if dist > min_dist_squared {
                    continue;
                }
                min_dist_squared = dist;
                zone_out = Some(zid);
                p_out_in_mm = p;
            }
        }

        (zone_out, p_out_in_mm)
    }

    /// C++ `TryPassBorderCross`.
    fn try_pass_border_cross(
        &mut self,
        env: &mut impl CursorEnv,
        zone: ZoneId,
        trip: Segment<f64>,
    ) -> bool {
        let bounds = self.layout.arena[zone].physical_bounds();
        let mut p = Point::default();

        let borders = [
            (Side::Left, Segment::new(bounds.top_left(), bounds.bottom_left()), true),
            (Side::Right, Segment::new(bounds.top_right(), bounds.bottom_right()), true),
            (Side::Top, Segment::new(bounds.top_left(), bounds.top_right()), false),
            (Side::Bottom, Segment::new(bounds.bottom_left(), bounds.bottom_right()), false),
        ];

        for (side, border, use_y) in borders {
            if border.is_intersecting_segment(&trip, &mut p) {
                let distance = Segment::new(p, trip.b()).size();
                let pos = if use_y { p.y() } else { p.x() };
                let links = self.side_links(zone, side);
                let Some(index) = at_physical_index(links, pos) else {
                    return false;
                };
                let key = ResistanceKey { zone, side, index };
                let resistance = links[index].border_resistance;
                return self.try_pass_border(env, key, resistance, distance);
            }
        }
        false
    }

    // --- resistance (C++ TryPassBorder / TryPassBorderPixel) -----------------

    fn try_pass_border(
        &mut self,
        env: &impl CursorEnv,
        key: ResistanceKey,
        link_resistance: f64,
        distance: f64,
    ) -> bool {
        if env.ctrl_down() {
            return true;
        }
        if self.current_resistance != Some(key) {
            self.current_resistance = Some(key);
            self.border_resistance = link_resistance;
        }
        self.border_resistance -= distance;
        self.border_resistance <= 0.0
    }

    fn try_pass_border_pixel(
        &mut self,
        env: &impl CursorEnv,
        key: ResistanceKey,
        link_resistance_px: i32,
        distance: i32,
    ) -> bool {
        if env.ctrl_down() {
            return true;
        }
        if self.current_resistance != Some(key) {
            self.current_resistance = Some(key);
            self.border_resistance_px = link_resistance_px;
        }
        self.border_resistance_px -= distance;
        self.border_resistance_px <= 0
    }

    // --- moves ---------------------------------------------------------------

    fn move_in_mm(
        &mut self,
        env: &mut impl CursorEnv,
        e: &mut MouseEventArg,
        p_out_in_mm: Point<f64>,
        zone_out: ZoneId,
    ) {
        let p_out = self.layout.arena[zone_out].to_pixels(p_out_in_mm);
        self.move_cursor(env, e, p_out, zone_out);
    }

    /// C++ `Move`.
    fn move_cursor(
        &mut self,
        env: &mut impl CursorEnv,
        e: &mut MouseEventArg,
        p_out: Point<i32>,
        zone_out: ZoneId,
    ) {
        let old_zone = self.old_zone.unwrap();
        let travel = self.layout.travel_pixels(old_zone, zone_out);

        // Keep the zone actually entered (matters for duplicated displays #83/#222).
        self.old_zone = Some(zone_out);
        self.old_point = p_out;

        let r = self.layout.arena[zone_out].pixels_bounds();

        self.save_clip(env);
        let mut pos = e.point;
        for rect in &travel {
            if rect.contains(pos) {
                continue;
            }
            env.set_clip(*rect);
            pos = env.get_mouse_location();
            if rect.contains(p_out) {
                break;
            }
        }

        env.set_clip(r);
        env.set_mouse_location(p_out);
        e.handled = true;
    }

    /// C++ `NoZoneMatches` — clip the cursor back into the current zone.
    fn no_zone_matches(&mut self, env: &mut impl CursorEnv, e: &mut MouseEventArg) {
        self.save_clip(env);
        let bounds = self.layout.arena[self.old_zone.unwrap()].pixels_bounds();
        env.set_clip(bounds);
        e.handled = false;
    }

    fn side_links(&self, zone: ZoneId, side: Side) -> &[crate::zones::ZoneLink] {
        let z = &self.layout.arena[zone];
        match side {
            Side::Left => &z.left,
            Side::Top => &z.top,
            Side::Right => &z.right,
            Side::Bottom => &z.bottom,
            Side::None => &[],
        }
    }
}

impl Default for MouseEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeCursor {
        pos: Point<i32>,
        clip: Rect<i32>,
        ctrl: bool,
        hidden: bool,
        clipped_sub: bool,
        tick: u64,
    }

    impl FakeCursor {
        fn new() -> Self {
            FakeCursor {
                pos: Point::new(0, 0),
                // A realistic "whole virtual desktop" clip so save/restore behaves.
                clip: Rect::new(-10000, -10000, 30000, 20000),
                ctrl: false,
                hidden: false,
                clipped_sub: false,
                tick: 0,
            }
        }
    }

    impl CursorEnv for FakeCursor {
        fn get_mouse_location(&self) -> Point<i32> {
            self.pos
        }
        fn set_mouse_location(&mut self, location: Point<i32>) {
            self.pos = location;
        }
        fn get_clip(&self) -> Rect<i32> {
            self.clip
        }
        fn set_clip(&mut self, r: Rect<i32>) {
            self.clip = r;
        }
        fn ctrl_down(&self) -> bool {
            self.ctrl
        }
        fn cursor_hidden(&self) -> bool {
            self.hidden
        }
        fn clip_is_subrect_of_virtual_screen(&self) -> bool {
            self.clipped_sub
        }
        fn tick_count(&self) -> u64 {
            self.tick
        }
    }

    // "Left" (pixels -3840..0) is adjacent to "Right" (pixels 0..3840). Left's
    // RightLinks map its right edge onto Right (id 1) and Right's LeftLinks map
    // back onto Left (id 0), like a real layout. Strait algorithm.
    const FIXTURE: &str = concat!(
        r#"<ZonesLayout Priority="Normal" PriorityUnhooked="Below" Algorithm="Strait" MaxTravelDistance="200"><MainZones>"#,
        r#"<Zone Id="0" Name="Left"><PixelsBounds><Rect Left="-3840" Top="0" Width="3840" Height="2160"></Rect></PixelsBounds><PhysicalBounds><Rect Left="-567" Top="30.920075223319227" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<RightLinks><ZoneLink From="0" To="393" SourceFromPixel="-225" SourceToPixel="2642" TargetFromPixel="0" TargetToPixel="2160" BorderResistance="0" TargetId="1"></ZoneLink>"#,
        r#"<ZoneLink From="393" To="1.7976931348623157E+308" SourceFromPixel="2642" SourceToPixel="2147483647" TargetFromPixel="-2147483648" TargetToPixel="2147483647" BorderResistance="0" TargetId="-1"></ZoneLink></RightLinks></Zone>"#,
        r#"<Zone Id="1" Name="Right"><PixelsBounds><Rect Left="0" Top="0" Width="3840" Height="2160"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="698" Height="393"></Rect></PhysicalBounds>"#,
        r#"<LeftLinks><ZoneLink From="0" To="393" SourceFromPixel="0" SourceToPixel="2160" TargetFromPixel="-225" TargetToPixel="2642" BorderResistance="0" TargetId="0"></ZoneLink></LeftLinks></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );

    fn engine() -> MouseEngine {
        let mut e = MouseEngine::new();
        e.load(ZonesLayout::from_xml(FIXTURE).unwrap());
        e
    }

    fn feed(eng: &mut MouseEngine, env: &mut FakeCursor, x: i32, y: i32) -> MouseEventArg {
        let mut ev = MouseEventArg::new(Point::new(x, y));
        eng.on_mouse_move(env, &mut ev);
        ev
    }

    #[test]
    fn first_event_initializes_tracking_without_moving() {
        let mut eng = engine();
        let mut env = FakeCursor::new();
        let ev = feed(&mut eng, &mut env, -100, 1000);
        assert!(!ev.handled);
        assert_eq!(eng.old_zone, eng.layout.containing_pixel(Point::new(-100, 1000)));
    }

    #[test]
    fn interior_move_passes_through() {
        let mut eng = engine();
        let mut env = FakeCursor::new();
        feed(&mut eng, &mut env, -100, 1000); // init in Left
        let ev = feed(&mut eng, &mut env, -200, 1000); // still interior
        assert!(!ev.handled);
        assert_eq!(env.pos, Point::new(0, 0)); // cursor untouched
    }

    #[test]
    fn crossing_right_border_repositions_into_target_zone() {
        let mut eng = engine();
        let mut env = FakeCursor::new();
        feed(&mut eng, &mut env, -100, 1000); // init in Left
        // Move to Left's right edge (x=0): crosses into Right, y remapped by DPI.
        let ev = feed(&mut eng, &mut env, 0, 1000);
        assert!(ev.handled, "crossing must be handled");
        // to_target_pixel(1000) = (1000-(-225))*2160/(2642+225) = 922
        assert_eq!(env.pos, Point::new(0, 922));
        // Tracking followed the cursor into Right.
        let right = *eng
            .layout
            .zones
            .iter()
            .find(|&&z| eng.layout.arena[z].name == "Right")
            .unwrap();
        assert_eq!(eng.old_zone, Some(right));
    }

    #[test]
    fn ctrl_bypasses_border_resistance() {
        // Give the border some resistance and confirm Ctrl punches straight through.
        let xml = FIXTURE.replace(r#"BorderResistance="0" TargetId="1""#, r#"BorderResistance="500" TargetId="1""#);
        let mut eng = MouseEngine::new();
        eng.load(ZonesLayout::from_xml(&xml).unwrap());
        let mut env = FakeCursor::new();
        env.ctrl = true;
        feed(&mut eng, &mut env, -100, 1000);
        let ev = feed(&mut eng, &mut env, 0, 1000);
        assert!(ev.handled, "Ctrl should force the crossing despite resistance");
        assert_eq!(env.pos, Point::new(0, 922));
    }

    #[test]
    fn landing_column_is_stable_under_tangential_motion() {
        // Cross Left -> Right (lands on Right's first column, x=0), then move
        // purely vertically: the pre-fix `dist >= 0` threshold fired Right's
        // LeftLinks at distance 0 and ping-ponged the cursor straight back.
        let mut eng = engine();
        let mut env = FakeCursor::new();
        feed(&mut eng, &mut env, -100, 1000); // init in Left
        let ev = feed(&mut eng, &mut env, 0, 1000); // cross, land at (0, 922)
        assert!(ev.handled);
        assert_eq!(env.pos, Point::new(0, 922));

        let ev = feed(&mut eng, &mut env, 0, 921); // tangential move on the edge column
        assert!(!ev.handled, "vertical move on the landing column must not re-cross");
        assert_eq!(env.pos, Point::new(0, 922), "cursor must not be warped");

        let ev = feed(&mut eng, &mut env, -1, 921); // actually leaving: crosses back
        assert!(ev.handled, "moving past the edge must still cross");
        assert_eq!(env.pos.x(), -1, "landed on Left's last column");
    }

    #[test]
    fn resistance_survives_tangential_frames_while_pinned() {
        // Right's LeftLink gets a resistance of 2mm over 393mm/2160px -> 10px.
        // While pinned against the border, pushes (dist=1) interleave with
        // tangential y frames on the edge column; those frames must not re-arm
        // the resistance or the border becomes impassable diagonally.
        let xml = FIXTURE.replace(
            r#"SourceFromPixel="0" SourceToPixel="2160" TargetFromPixel="-225" TargetToPixel="2642" BorderResistance="0""#,
            r#"SourceFromPixel="0" SourceToPixel="2160" TargetFromPixel="-225" TargetToPixel="2642" BorderResistance="2""#,
        );
        let mut eng = MouseEngine::new();
        eng.load(ZonesLayout::from_xml(&xml).unwrap());
        let mut env = FakeCursor::new();
        feed(&mut eng, &mut env, 100, 1000); // init in Right

        let mut crossed_at = None;
        for i in 0..20 {
            let ev = feed(&mut eng, &mut env, -1, 1000 - i); // push (dist=1)
            if ev.handled {
                crossed_at = Some(i);
                break;
            }
            // Tangential frame on the pinned column: must not reset the drain.
            let ev = feed(&mut eng, &mut env, 0, 1000 - i - 1);
            assert!(!ev.handled);
        }
        assert_eq!(crossed_at, Some(9), "10px of resistance -> pass on the 10th push");
    }

    #[test]
    fn cross_mode_resistance_survives_tangential_frames_while_pinned() {
        // Cross-mode twin of resistance_survives_tangential_frames_while_pinned:
        // the interior fast-path used to reset the drain on EVERY contained
        // frame — the pinned edge column is contained, so tangential/zero-dx
        // frames re-armed the resistance and a resisted border became a wall.
        let xml = FIXTURE
            .replace(r#"Algorithm="Strait""#, r#"Algorithm="Cross""#)
            .replace(
                r#"SourceFromPixel="0" SourceToPixel="2160" TargetFromPixel="-225" TargetToPixel="2642" BorderResistance="0""#,
                r#"SourceFromPixel="0" SourceToPixel="2160" TargetFromPixel="-225" TargetToPixel="2642" BorderResistance="3""#,
            );
        let mut eng = MouseEngine::new();
        eng.load(ZonesLayout::from_xml(&xml).unwrap());
        let mut env = FakeCursor::new();
        feed(&mut eng, &mut env, 100, 1000); // init in Right

        // Right is 698mm/3840px: a 1px overshoot drains ~0.09mm per push, so
        // 3mm of resistance passes after a few dozen pushes — but only if the
        // interleaved tangential frames don't reset the drain.
        let mut crossed = None;
        for i in 0..100 {
            let ev = feed(&mut eng, &mut env, -1, 1000 - i); // push 1px past the border
            if ev.handled {
                crossed = Some(i);
                break;
            }
            let ev = feed(&mut eng, &mut env, 0, 1000 - i - 1); // tangential, contained
            assert!(!ev.handled);
        }
        assert!(crossed.is_some(), "the drain must survive tangential frames and eventually pass");
        assert!(crossed.unwrap() > 0, "resistance must actually resist the first push");
    }

    #[test]
    fn freelook_when_cursor_hidden_passes_through() {
        let mut eng = engine();
        let mut env = FakeCursor::new();
        feed(&mut eng, &mut env, -100, 1000); // init
        env.hidden = true;
        env.tick = 1000;
        // At the border (would normally cross), but freelook is active.
        let ev = feed(&mut eng, &mut env, 0, 1000);
        assert!(!ev.handled, "freelook must pass the event through");
        assert_eq!(env.pos, Point::new(0, 0), "cursor untouched in freelook");
    }

    #[test]
    fn reload_layout_does_not_use_stale_zone() {
        let mut eng = engine();
        let mut env = FakeCursor::new();
        feed(&mut eng, &mut env, -100, 1000); // old_zone points into the first arena
        // Reload: the old arena is dropped; old_zone must not be dereferenced.
        eng.load(ZonesLayout::from_xml(FIXTURE).unwrap());
        assert_eq!(eng.old_zone, None);
        // A fresh event re-initializes cleanly (no panic on a stale ZoneId).
        let ev = feed(&mut eng, &mut env, -100, 1000);
        assert!(!ev.handled);
        assert!(eng.old_zone.is_some());
    }

    #[test]
    fn hot_reload_under_motion_never_panics() {
        // The torture test: reload the layout repeatedly while feeding moves that
        // wander across the Left/Right border. If a stale ZoneId were ever
        // dereferenced, `arena[stale]` would panic — reaching the end proves the
        // generational arena makes the C++ use-after-free unrepresentable.
        let mut eng = engine();
        let mut env = FakeCursor::new();
        for i in 0..3000i32 {
            let x = -150 + (i.wrapping_mul(37) % 300);
            let y = 200 + (i % 1600);
            feed(&mut eng, &mut env, x, y);
            if i % 11 == 0 {
                eng.load(ZonesLayout::from_xml(FIXTURE).unwrap());
            }
        }
    }
}
