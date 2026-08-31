//! Asking the world where things are: the desktop the ABS range has to match,
//! the cursor position to take over from, and the fallback start point.
//!
//! Two of these read the engine's layout under its lock, which is why they come
//! in a blocking flavour (arm time, before the grabs — nothing is captured yet)
//! and a `try_lock` one (pump time — the routing thread must never block; on
//! contention the caller keeps its cached bounds for one cycle).

use std::time::Duration;

use crate::geometry::{Point, Rect};
use crate::shared::Shared;

/// The union of the layout's main zones — the compositor's logical pixel space
/// (kscreen coordinates), so the ABS mapping and the crossing geometry agree.
/// Arm-time variant: routing has not started, a blocking lock is fine here.
pub(super) fn desktop_bounds_blocking(shared: &Shared) -> Rect<i32> {
    let engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
    bounds_of(&engine)
}

/// Pump-side variant — routing-thread rule: never block. On contention (an IPC
/// Load swapping the layout under the lock) returns None and the caller keeps
/// its cached bounds for one cycle.
pub(super) fn try_desktop_bounds(shared: &Shared) -> Option<Rect<i32>> {
    let engine = match shared.engine.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return None,
    };
    Some(bounds_of(&engine))
}

fn bounds_of(engine: &crate::engine::MouseEngine) -> Rect<i32> {
    let mut it = engine
        .layout
        .main_zones
        .iter()
        .map(|&id| engine.layout.arena[id].pixels_bounds());
    let Some(first) = it.next() else {
        return Rect::new(0, 0, 1920, 1080);
    };
    let (mut l, mut t, mut r, mut b) = (first.left(), first.top(), first.right(), first.bottom());
    for z in it {
        l = l.min(z.left());
        t = t.min(z.top());
        r = r.max(z.right());
        b = b.max(z.bottom());
    }
    Rect::new(l, t, r - l, b - t)
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

/// Centre of the first main zone, a guaranteed on-screen start point.
pub(super) fn first_zone_center(shared: &Shared) -> Option<Point<i32>> {
    let engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
    let id = *engine.layout.main_zones.first()?;
    let b = engine.layout.arena[id].pixels_bounds();
    Some(Point::new(
        b.left() + b.width() / 2,
        b.top() + b.height() / 2,
    ))
}
