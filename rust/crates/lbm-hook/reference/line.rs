use crate::point::Point;

#[derive(Debug)]
pub struct Line<T> {
    // Y = slope * X + origin
    slope: f64,
    // y pour x==0
    origin: T,
}

impl<T> Line<T>
where
    T: Copy + std::ops::Div<Output = T> + std::ops::Mul<Output = T> +
    std::ops::Sub<Output = T> + std::ops::Add<Output = T> +
    PartialEq + From<f64>
{
    pub fn new(slope: f64, y_for_x0: T) -> Self {
        Line {
            slope,
            origin: y_for_x0,
        }
    }

    pub fn new_vertical(x: T) -> Self {
        Line {
            slope: f64::MAX,
            origin: x,
        }
    }

    pub fn is_vertical(&self) -> bool {
        !(self.slope < f64::MAX)
    }

    pub fn slope(&self) -> f64 {
        self.slope
    }

    pub fn x_at_y0(&self) -> T {
        if self.is_vertical() {
            self.origin
        } else {
            let slope_t: T = T::from(self.slope);
            self.origin / slope_t
        }
    }

    pub fn y_at_x0(&self) -> T {
        if self.is_vertical() {
            T::from(0.0)
        } else {
            self.origin
        }
    }

    pub fn x(&self, y: T) -> T {
        if self.slope == 0.0 {
            T::from(0.0) // Erreur
        } else if self.is_vertical() {
            self.origin
        } else {
            let slope_t: T = T::from(self.slope);
            self.origin - y / slope_t
        }
    }

    pub fn y(&self, x: T) -> T {
        if self.slope == 0.0 {
            self.origin
        } else if self.is_vertical() {
            T::from(0.0) // Erreur
        } else {
            let slope_t: T = T::from(self.slope);
            slope_t * x + self.origin
        }
    }

    pub fn origin(&self) -> Point<T> {
        Point::new(T::from(0.0), self.y_at_x0())
    }

    pub fn is_intersecting(&self, l: &Line<T>) -> Option<Point<T>> {
        // Les lignes sont parallèles
        if self.slope == l.slope {
            // Les lignes sont identiques, retourner le point d'origine
            if self.origin == l.origin {
                Some(self.origin())
            } else {
                // Pas d'intersection
                None
            }
        } else {
            let x: T;
            let y: T;

            if self.is_vertical() {
                x = self.origin;
                y = l.y(x);
            } else if l.is_vertical() {
                x = l.origin;
                y = self.y(x);
            } else {
                let slope_diff: T = T::from(l.slope - self.slope);
                x = (self.origin - l.origin) / slope_diff;
                y = self.y(x);
            }

            Some(Point::new(x, y))
        }
    }
}

