//! Manual check: ask KWin where the cursor is (evdev backend start-position probe).
fn main() {
    match littlebigmouse_hook::hook::linux::evdev::kwin_cursor_pos() {
        Some(p) => println!("cursor at ({},{})", p.x(), p.y()),
        None => println!("probe failed (no KWin?)"),
    }
}
