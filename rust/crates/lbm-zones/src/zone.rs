//! Port of `Engine/Zone`.

use super::zone_link::ZoneLink;
use super::ZoneId;
use crate::geometry::{Point, Rect};

pub struct Zone {
    pub id: i32,
    pub device_id: String,
    pub name: String,
    pixels_bounds: Rect<i32>,
    physical_bounds: Rect<f64>,
    physical_inside: Rect<f64>,
    pub dpi: f64,
    /// Self, or the `ZoneId` of an earlier zone with identical pixel bounds
    /// (a clone). C++ `Zone::Main`.
    pub main: ZoneId,
    pub left: Vec<ZoneLink>,
    pub top: Vec<ZoneLink>,
    pub right: Vec<ZoneLink>,
    pub bottom: Vec<ZoneLink>,
}

/// C++ `Zone::ComputeDpi` / the DPI computed in the constructor.
pub fn compute_dpi(pixels: Rect<i32>, physical: Rect<f64>) -> f64 {
    let dpi_x = pixels.width() as f64 / (physical.width() / 25.4);
    let dpi_y = pixels.height() as f64 / (physical.height() / 25.4);
    (dpi_x * dpi_x + dpi_y * dpi_y).sqrt() / 2.0_f64.sqrt()
}

impl Zone {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i32,
        device_id: String,
        name: String,
        pixels_bounds: Rect<i32>,
        physical_bounds: Rect<f64>,
        main: ZoneId,
        left: Vec<ZoneLink>,
        top: Vec<ZoneLink>,
        right: Vec<ZoneLink>,
        bottom: Vec<ZoneLink>,
    ) -> Self {
        let dpi = compute_dpi(pixels_bounds, physical_bounds);

        let pixel_width = physical_bounds.width() / pixels_bounds.width() as f64;
        let pixel_height = physical_bounds.height() / pixels_bounds.height() as f64;
        let physical_inside = Rect::new(
            physical_bounds.left() + pixel_width / 2.0,
            physical_bounds.top() + pixel_height / 2.0,
            physical_bounds.width() - pixel_width,
            physical_bounds.height() - pixel_height,
        );

        Zone {
            id,
            device_id,
            name,
            pixels_bounds,
            physical_bounds,
            physical_inside,
            dpi,
            main,
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn pixels_bounds(&self) -> Rect<i32> {
        self.pixels_bounds
    }
    pub fn physical_bounds(&self) -> Rect<f64> {
        self.physical_bounds
    }
    pub fn physical_inside(&self) -> Rect<f64> {
        self.physical_inside
    }

    /// C++ `Zone::ToPhysical`.
    pub fn to_physical(&self, px: Point<i32>) -> Point<f64> {
        let x = self.physical_bounds.left()
            + (0.5 + (px.x() - self.pixels_bounds.left()) as f64) * self.physical_bounds.width()
                / self.pixels_bounds.width() as f64;
        let y = self.physical_bounds.top()
            + (0.5 + (px.y() - self.pixels_bounds.top()) as f64) * self.physical_bounds.height()
                / self.pixels_bounds.height() as f64;
        Point::new(x, y)
    }

    /// C++ `Zone::ToPixels`.
    pub fn to_pixels(&self, mm: Point<f64>) -> Point<i32> {
        let x = self.pixels_bounds.left()
            + ((mm.x() - self.physical_bounds.left()) * self.pixels_bounds.width() as f64
                / self.physical_bounds.width()) as i32;
        let y = self.pixels_bounds.top()
            + ((mm.y() - self.physical_bounds.top()) * self.pixels_bounds.height() as f64
                / self.physical_bounds.height()) as i32;
        Point::new(x, y)
    }

    /// C++ `Zone::CenterPixel` — reproduced verbatim, including the `left +
    /// right/2` operator-precedence quirk (not a true centre).
    pub fn center_pixel(&self) -> Point<i32> {
        let x = self.pixels_bounds.left() + self.pixels_bounds.right() / 2;
        let y = self.pixels_bounds.top() + self.pixels_bounds.bottom() / 2;
        Point::new(x, y)
    }

    /// C++ `Zone::Contains(Point<long>)`.
    pub fn contains_pixel(&self, pixel: Point<i32>) -> bool {
        pixel.x() >= self.pixels_bounds.left()
            && pixel.y() >= self.pixels_bounds.top()
            && pixel.x() < self.pixels_bounds.right()
            && pixel.y() < self.pixels_bounds.bottom()
    }

    /// C++ `Zone::Contains(Point<double>)`.
    pub fn contains_mm(&self, mm: Point<f64>) -> bool {
        self.physical_bounds.contains(mm)
    }

    /// C++ `Zone::InsidePixelsBounds`.
    pub fn inside_pixels_bounds(&self, px: Point<i32>) -> Point<i32> {
        let mut x = px.x();
        let mut y = px.y();
        if x < self.pixels_bounds.left() {
            x = self.pixels_bounds.left();
        } else if x > self.pixels_bounds.right() - 1 {
            x = self.pixels_bounds.right() - 1;
        }
        if y < self.pixels_bounds.top() {
            y = self.pixels_bounds.top();
        } else if y > self.pixels_bounds.bottom() - 1 {
            y = self.pixels_bounds.bottom() - 1;
        }
        Point::new(x, y)
    }

    /// C++ `Zone::InsidePhysicalBounds`.
    pub fn inside_physical_bounds(&self, mm: Point<f64>) -> Point<f64> {
        let mut x = mm.x();
        let mut y = mm.y();
        if x < self.physical_bounds.left() {
            x = self.physical_bounds.left();
        } else if x > self.physical_bounds.right() {
            x = self.physical_bounds.right();
        }
        if y < self.physical_bounds.top() {
            y = self.physical_bounds.top();
        } else if y > self.physical_bounds.bottom() {
            y = self.physical_bounds.bottom();
        }
        Point::new(x, y)
    }

    /// C++ `Zone::HorizontalReachable`.
    pub fn horizontal_reachable(&self, mm: Point<f64>) -> bool {
        let y = mm.y();
        self.physical_bounds.top() <= y && self.physical_bounds.bottom() >= y
    }

    /// C++ `Zone::VerticalReachable`.
    pub fn vertical_reachable(&self, mm: Point<f64>) -> bool {
        let x = mm.x();
        self.physical_bounds.left() <= x && self.physical_bounds.right() >= x
    }
}
