//! ARGB override-redirect windows with server-verified geometry and empty input shapes.
use crate::{
    raster,
    scene::{Band, Pixels},
    Backend,
};
use std::time::{Duration, Instant};
use x11rb::{
    connection::{Connection, RequestConnection},
    protocol::{
        randr::{ConnectionExt as _, NotifyMask},
        render::{ConnectionExt as _, PictType},
        shape::{self, ConnectionExt as _, SK},
        xproto::{ConnectionExt as _, *},
        Event,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

fn err(e: impl std::fmt::Display) -> String {
    format!("X11 overlay: {e}")
}
struct Surface {
    window: u32,
    pixmap: u32,
    gc: u32,
    band: Band,
}
pub(super) struct Host {
    conn: RustConnection,
    root: u32,
    visual: u32,
    colormap: u32,
    selection: u32,
    owner: u32,
    surfaces: Vec<Surface>,
    checked: Instant,
}
impl Host {
    pub fn connect() -> Result<Self, String> {
        let (conn, index) = x11rb::connect(None).map_err(err)?;
        let shape = conn
            .shape_query_version()
            .map_err(err)?
            .reply()
            .map_err(err)?;
        if (shape.major_version, shape.minor_version) < (1, 1) {
            return Err(err("Shape 1.1 is required for click-through."));
        }
        let randr = conn
            .randr_query_version(1, 3)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        if (randr.major_version, randr.minor_version) < (1, 3) {
            return Err(err("RandR 1.3 is required to verify output transforms."));
        }
        let screen = &conn.setup().roots[index];
        let root = screen.root;
        let formats = conn
            .render_query_pict_formats()
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let visual = screen
            .allowed_depths
            .iter()
            .filter(|d| d.depth == 32)
            .flat_map(|d| &d.visuals)
            .find(|v| {
                v.class == VisualClass::TRUE_COLOR
                    && formats.screens.get(index).is_some_and(|s| {
                        s.depths.iter().flat_map(|d| &d.visuals).any(|p| {
                            p.visual == v.visual_id
                                && formats.formats.iter().any(|f| {
                                    let d = f.direct;
                                    f.id == p.format
                                        && f.depth == 32
                                        && f.type_ == PictType::DIRECT
                                        && d.alpha_mask == 255
                                        && d.alpha_shift == 24
                                        && d.red_mask == 255
                                        && d.red_shift == 16
                                        && d.green_mask == 255
                                        && d.green_shift == 8
                                        && d.blue_mask == 255
                                        && d.blue_shift == 0
                                })
                        })
                    })
            })
            .map(|v| v.visual_id)
            .ok_or_else(|| err("No premultiplied ARGB visual is available."))?;
        if !conn
            .setup()
            .pixmap_formats
            .iter()
            .any(|f| f.depth == 32 && f.bits_per_pixel == 32)
        {
            return Err(err("Unsupported ARGB pixel format."));
        }
        let selection = conn
            .intern_atom(false, format!("_NET_WM_CM_S{index}").as_bytes())
            .map_err(err)?
            .reply()
            .map_err(err)?
            .atom;
        let owner = conn
            .get_selection_owner(selection)
            .map_err(err)?
            .reply()
            .map_err(err)?
            .owner;
        if owner == 0 {
            return Err(err(
                "A compositing manager is required for transparent overlays.",
            ));
        }
        let colormap = conn.generate_id().map_err(err)?;
        conn.create_colormap(ColormapAlloc::NONE, colormap, root, visual)
            .map_err(err)?
            .check()
            .map_err(err)?;
        conn.randr_select_input(
            root,
            NotifyMask::SCREEN_CHANGE | NotifyMask::OUTPUT_CHANGE | NotifyMask::CRTC_CHANGE,
        )
        .map_err(err)?
        .check()
        .map_err(err)?;
        Ok(Self {
            conn,
            root,
            visual,
            colormap,
            selection,
            owner,
            surfaces: Vec::new(),
            checked: Instant::now(),
        })
    }
    fn verify_outputs(&self, bands: &[Band]) -> Result<(), String> {
        let resources = self
            .conn
            .randr_get_screen_resources_current(self.root)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let mut outputs = Vec::new();
        for output in resources.outputs {
            let info = self
                .conn
                .randr_get_output_info(output, resources.config_timestamp)
                .map_err(err)?
                .reply()
                .map_err(err)?;
            if info.crtc == 0 {
                continue;
            }
            let crtc = self
                .conn
                .randr_get_crtc_info(info.crtc, resources.config_timestamp)
                .map_err(err)?
                .reply()
                .map_err(err)?;
            let transform = self
                .conn
                .randr_get_crtc_transform(info.crtc)
                .map_err(err)?
                .reply()
                .map_err(err)?
                .current_transform;
            let identity = transform.matrix11 == 65536
                && transform.matrix22 == 65536
                && transform.matrix33 == 65536
                && [
                    transform.matrix12,
                    transform.matrix13,
                    transform.matrix21,
                    transform.matrix23,
                    transform.matrix31,
                    transform.matrix32,
                ]
                .iter()
                .all(|v| *v == 0);
            outputs.push((
                String::from_utf8_lossy(&info.name).into_owned(),
                Pixels {
                    x: crtc.x.into(),
                    y: crtc.y.into(),
                    width: crtc.width.into(),
                    height: crtc.height.into(),
                },
                identity,
            ));
        }
        for band in bands {
            if !outputs.iter().any(|(name, bounds, identity)| {
                name == &band.connector && bounds == &band.screen && *identity
            }) {
                return Err(err(format!("Output {} changed or uses transformed coordinates. Reload the displays; scaled RandR transforms are not supported.",band.connector)));
            }
            let r = band.rect;
            if i16::try_from(r.x).is_err()
                || i16::try_from(r.y).is_err()
                || r.width > i16::MAX.into()
                || r.height > i16::MAX.into()
            {
                return Err(err("Overlay coordinates exceed the X11 protocol range."));
            }
        }
        Ok(())
    }
    fn create(&mut self, band: &Band) -> Result<(), String> {
        let c = &self.conn;
        let window = c.generate_id().map_err(err)?;
        let pixmap = c.generate_id().map_err(err)?;
        let gc = c.generate_id().map_err(err)?;
        let r = band.rect;
        // Register ownership before fallible requests; connection teardown also releases partial allocations.
        self.surfaces.push(Surface {
            window,
            pixmap,
            gc,
            band: band.clone(),
        });
        c.create_pixmap(32, pixmap, self.root, r.width as u16, r.height as u16)
            .map_err(err)?
            .check()
            .map_err(err)?;
        c.create_gc(gc, pixmap, &CreateGCAux::new())
            .map_err(err)?
            .check()
            .map_err(err)?;
        c.create_window(
            32,
            window,
            self.root,
            r.x as i16,
            r.y as i16,
            r.width as u16,
            r.height as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            self.visual,
            &CreateWindowAux::new()
                .override_redirect(1)
                .background_pixmap(pixmap)
                .border_pixel(0)
                .colormap(self.colormap),
        )
        .map_err(err)?
        .check()
        .map_err(err)?;
        c.shape_rectangles(
            shape::SO::SET,
            SK::INPUT,
            ClipOrdering::UNSORTED,
            window,
            0,
            0,
            &[],
        )
        .map_err(err)?
        .check()
        .map_err(err)?;
        c.change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            b"LittleBigMouse screen overlay",
        )
        .map_err(err)?
        .check()
        .map_err(err)?;
        // ICCCM InputHint=False. We never request focus or select keyboard/pointer events.
        c.change_property32(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_HINTS,
            AtomEnum::WM_HINTS,
            &[1, 0, 0, 0, 0, 0, 0, 0, 0],
        )
        .map_err(err)?
        .check()
        .map_err(err)?;
        let geometry = c.get_geometry(window).map_err(err)?.reply().map_err(err)?;
        let attributes = c
            .get_window_attributes(window)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let input = c
            .shape_get_rectangles(window, SK::INPUT)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        if (
            i32::from(geometry.x),
            i32::from(geometry.y),
            i32::from(geometry.width),
            i32::from(geometry.height),
        ) != (r.x, r.y, r.width, r.height)
            || geometry.depth != 32
            || !attributes.override_redirect
            || !input.rectangles.is_empty()
        {
            return Err(err(
                "The server refused overlay placement or click-through.",
            ));
        }
        Ok(())
    }
    fn paint(&self, index: usize, band: &Band) -> Result<(), String> {
        let s = &self.surfaces[index];
        let mut image = raster::render(band)?;
        if self.conn.setup().image_byte_order == ImageOrder::MSB_FIRST {
            for pixel in image.bgra.as_chunks_mut::<4>().0 {
                pixel.reverse();
            }
        }
        let stride = image.width * 4;
        let rows =
            (self.conn.maximum_request_bytes().saturating_sub(32) / stride).min(u16::MAX as usize);
        if rows == 0 {
            return Err(err("The X11 request limit cannot fit one overlay row."));
        }
        for (n, chunk) in image.bgra.chunks(rows * stride).enumerate() {
            self.conn
                .put_image(
                    ImageFormat::Z_PIXMAP,
                    s.pixmap,
                    s.gc,
                    image.width as u16,
                    (chunk.len() / stride) as u16,
                    0,
                    (n * rows) as i16,
                    0,
                    32,
                    chunk,
                )
                .map_err(err)?
                .check()
                .map_err(err)?;
        }
        self.conn
            .clear_area(false, s.window, 0, 0, 0, 0)
            .map_err(err)?
            .check()
            .map_err(err)?;
        self.conn
            .configure_window(
                s.window,
                &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
            )
            .map_err(err)?
            .check()
            .map_err(err)?;
        self.conn
            .map_window(s.window)
            .map_err(err)?
            .check()
            .map_err(err)?;
        Ok(())
    }
}
impl Backend for Host {
    fn present(&mut self, bands: &[Band]) -> Result<(), String> {
        super::validate(bands)?;
        self.verify_outputs(bands)?;
        let rebuild = self.surfaces.len() != bands.len()
            || self
                .surfaces
                .iter()
                .zip(bands)
                .any(|(s, b)| s.band.rect != b.rect || s.band.connector != b.connector);
        if rebuild {
            self.clear();
            for band in bands {
                self.create(band)?;
            }
        }
        for (index, band) in bands.iter().enumerate() {
            if rebuild || self.surfaces[index].band != *band {
                self.paint(index, band)?;
                self.surfaces[index].band = band.clone();
            }
        }
        self.conn.flush().map_err(err)
    }
    fn clear(&mut self) {
        for s in self.surfaces.drain(..) {
            let _ = self.conn.destroy_window(s.window);
            let _ = self.conn.free_gc(s.gc);
            let _ = self.conn.free_pixmap(s.pixmap);
        }
        let _ = self.conn.flush();
    }
    fn poll(&mut self) -> Result<(), String> {
        while let Some(event) = self.conn.poll_for_event().map_err(err)? {
            if matches!(
                event,
                Event::RandrNotify(_) | Event::RandrScreenChangeNotify(_)
            ) {
                return Err(err(
                    "Displays changed. Reload the screens before showing overlays again.",
                ));
            }
            if let Event::Error(e) = event {
                return Err(err(format!("{e:?}")));
            }
        }
        if self.checked.elapsed() >= Duration::from_millis(250) {
            self.checked = Instant::now();
            if self
                .conn
                .get_selection_owner(self.selection)
                .map_err(err)?
                .reply()
                .map_err(err)?
                .owner
                != self.owner
            {
                return Err(err("The compositing manager changed."));
            }
            // Override-redirect windows bypass WM policies, including its "above" hint.
            // Repair stacking when another window is raised, without activating anything.
            if !self.surfaces.is_empty() {
                let children = self
                    .conn
                    .query_tree(self.root)
                    .map_err(err)?
                    .reply()
                    .map_err(err)?
                    .children;
                if children
                    .iter()
                    .rev()
                    .take(self.surfaces.len())
                    .any(|id| !self.surfaces.iter().any(|s| s.window == *id))
                {
                    for surface in &self.surfaces {
                        self.conn
                            .configure_window(
                                surface.window,
                                &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
                            )
                            .map_err(err)?
                            .check()
                            .map_err(err)?;
                    }
                    self.conn.flush().map_err(err)?;
                }
            }
        }
        Ok(())
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.clear();
        let _ = self.conn.free_colormap(self.colormap);
        let _ = self.conn.flush();
    }
}
