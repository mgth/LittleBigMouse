//! Asking the world where things are: where the cursor may go, the desktop the ABS
//! range has to match, the cursor position to take over from, and the fallback start
//! point.
//!
//! Two of these read the engine's layout under its lock, which is why they come
//! in a blocking flavour (arm time, before the grabs — nothing is captured yet)
//! and a `try_lock` one (pump time — the routing thread must never block; on
//! contention the caller keeps its cached bounds for one cycle).

use std::time::Duration;

use crate::geometry::{Point, Rect};
use crate::shared::Shared;

/// Where the cursor may go — the union of the layout's main zones in play, in the
/// compositor's logical pixel space (kscreen coordinates), so the crossing geometry
/// and the positions emitted agree — and the union of this machine's own zones among
/// them, which is all a position emitted here may ever be in (docs/multi-pc.md: the
/// other machines' desktops sit outside this one's).
///
/// Arm-time variant: routing has not started, a blocking lock is fine here. The pump
/// reads [`extents`] under the `try_lock` it already holds.
pub(crate) fn layout_bounds_blocking(shared: &Shared) -> (Rect<i32>, Rect<i32>) {
    let engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
    extents(&engine.layout)
}

/// The desktop the absolute device must span: what the agent said when it handed
/// over the layout, because only the agent enumerates the outputs. Falling back to
/// the layout's own extent when it said nothing — an agent older than the field, or
/// a hook driven by something else — which is what this always used to do.
///
/// The two differ exactly when the layout does not cover the desktop: a monitor
/// excluded from the layout is still drawn by the compositor. Declaring the smaller
/// rectangle does not keep the cursor off that monitor; it stretches every position
/// by the ratio between the two, and the cursor lands somewhere else entirely.
/// Arm-time variant: routing has not started, a blocking lock is fine here.
pub(super) fn device_bounds_blocking(shared: &Shared, layout: Rect<i32>) -> Rect<i32> {
    shared
        .desktop
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .unwrap_or(layout)
}

/// Pump-side variant — routing-thread rule: never block. On contention (a `Load`
/// recording a new desktop under the lock) returns None and the caller keeps what it
/// has for one cycle, exactly as it does for the layout's own extent.
pub(super) fn try_device_bounds(shared: &Shared, layout: Rect<i32>) -> Option<Rect<i32>> {
    let desktop = match shared.desktop.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return None,
    };
    Some(desktop.unwrap_or(layout))
}

/// `(every zone in play, this machine's zones)`, each as one rectangle. One pass, no
/// allocation: the pump calls it every cycle.
pub(super) fn extents(layout: &crate::zones::ZonesLayout) -> (Rect<i32>, Rect<i32>) {
    // (left, top, right, bottom), or None while nothing was seen.
    type Union = Option<(i32, i32, i32, i32)>;
    fn grow(union: &mut Union, z: Rect<i32>) {
        let (l, t, r, b) = union.get_or_insert((z.left(), z.top(), z.right(), z.bottom()));
        *l = (*l).min(z.left());
        *t = (*t).min(z.top());
        *r = (*r).max(z.right());
        *b = (*b).max(z.bottom());
    }
    fn rect(union: Union) -> Rect<i32> {
        match union {
            Some((l, t, r, b)) => Rect::new(l, t, r - l, b - t),
            None => Rect::new(0, 0, 1920, 1080),
        }
    }
    let (mut all, mut local): (Union, Union) = (None, None);
    for &id in &layout.main_zones {
        let zone = &layout.arena[id];
        grow(&mut all, zone.pixels_bounds());
        if zone.host.is_none() {
            grow(&mut local, zone.pixels_bounds());
        }
    }
    (rect(all), rect(local))
}

/// Ask KWin for the real cursor position (logical coordinates, the same space
/// as the zones) through its scripting API — the only channel an ordinary
/// process has under Wayland, where no global pointer query exists. A one-shot
/// script reports `workspace.cursorPos` back over DBus (as a string: KWin
/// marshals JS numbers as doubles, which would not match an integer signature)
/// and is unloaded again. Returns None on any failure (no session bus, not
/// KWin, timeout): the caller falls back to a neutral position.
pub fn kwin_cursor_pos() -> Option<Point<i32>> {
    use tokio::sync::mpsc;

    struct Probe {
        tx: mpsc::Sender<(i32, i32)>,
    }

    #[zbus::interface(name = "org.littlebigmouse.CursorProbe")]
    impl Probe {
        fn report(&self, pos: String) {
            if let Some((x, y)) = pos.split_once(',') {
                if let (Ok(x), Ok(y)) = (x.trim().parse(), y.trim().parse()) {
                    let _ = self.tx.try_send((x, y));
                }
            }
        }
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    rt.block_on(async {
        let (tx, mut rx) = mpsc::channel(1);
        let service = format!("org.littlebigmouse.CursorProbe{}", std::process::id());
        let conn = zbus::connection::Builder::session()
            .ok()?
            .name(service.as_str())
            .ok()?
            .serve_at("/", Probe { tx })
            .ok()?
            .build()
            .await
            .ok()?;

        let plugin = format!("lbm-cursor-probe-{}", std::process::id());
        let script_path = std::env::temp_dir().join(format!("{plugin}.js"));
        std::fs::write(
            &script_path,
            format!(
                // "Report": the zbus interface macro exposes rust methods under
                // their PascalCase DBus names; callDBus swallows NoSuchMethod.
                "callDBus(\"{service}\", \"/\", \"org.littlebigmouse.CursorProbe\", \"Report\", \
                 workspace.cursorPos.x + \",\" + workspace.cursorPos.y);\n"
            ),
        )
        .ok()?;

        let scripting = zbus::Proxy::new(
            &conn,
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting",
        )
        .await
        .ok()?;
        // A probe left over by a crashed run would make loadScript return -1.
        let _ = scripting
            .call_method("unloadScript", &(plugin.as_str(),))
            .await;

        let id: i32 = scripting
            .call(
                "loadScript",
                &(script_path.to_string_lossy().as_ref(), plugin.as_str()),
            )
            .await
            .unwrap_or(-1);

        let result = if id < 0 {
            None
        } else {
            match zbus::Proxy::new(
                &conn,
                "org.kde.KWin",
                format!("/Scripting/Script{id}"),
                "org.kde.kwin.Script",
            )
            .await
            {
                Ok(script) if script.call::<_, _, ()>("run", &()).await.is_ok() => {
                    tokio::time::timeout(Duration::from_millis(700), rx.recv())
                        .await
                        .ok()
                        .flatten()
                        .map(|(x, y)| Point::new(x, y))
                }
                _ => None,
            }
        };

        let _ = scripting
            .call_method("unloadScript", &(plugin.as_str(),))
            .await;
        let _ = std::fs::remove_file(&script_path);
        result
    })
}

/// Centre of the first main zone of this machine, a guaranteed on-screen start point.
pub(super) fn first_zone_center(shared: &Shared) -> Option<Point<i32>> {
    let engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
    let layout = &engine.layout;
    let id = *layout
        .main_zones
        .iter()
        .find(|&&id| layout.arena[id].host.is_none())?;
    let b = engine.layout.arena[id].pixels_bounds();
    Some(Point::new(
        b.left() + b.width() / 2,
        b.top() + b.height() / 2,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The desktop is the agent's to name. Its own extent stands in only while nobody
    /// has said — an agent older than the field, or a hook driven by something else.
    #[test]
    fn the_agents_desktop_wins_over_the_layouts_extent() {
        let shared = Shared::new();
        let layout = Rect::new(0, 0, 1920, 1080);

        assert_eq!(
            device_bounds_blocking(&shared, layout),
            layout,
            "nobody has said: the layout's extent stands in"
        );

        // A second screen the layout excludes: the compositor still draws it, so the
        // desktop is wider than the layout. Declaring the narrower one would stretch
        // every position by the ratio between the two.
        let desktop = Rect::new(0, 0, 3840, 1080);
        *shared.desktop.lock().unwrap() = Some(desktop);
        assert_eq!(device_bounds_blocking(&shared, layout), desktop);
        assert_eq!(try_device_bounds(&shared, layout), Some(desktop));
    }

    /// An origin left of zero is what a screen placed to the left of the primary gives.
    /// Losing it would move every position by that screen's width.
    #[test]
    fn a_desktop_that_starts_left_of_zero_keeps_its_origin() {
        let shared = Shared::new();
        let desktop = Rect::new(-1920, -120, 5760, 1200);
        *shared.desktop.lock().unwrap() = Some(desktop);

        assert_eq!(
            device_bounds_blocking(&shared, Rect::new(0, 0, 1920, 1080)),
            desktop
        );
    }
}
