/// A pair of scale factors, one per axis: `IDisplayRatio` in C#.
///
/// C# builds ratios as a graph of reactive objects (`DisplayRatioValue`,
/// `DisplayRatioRatio` for products, `DisplayInverseRatio` for reciprocals).
/// Here a ratio is a value, and the graph is the order of the calls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ratio {
    pub x: f64,
    pub y: f64,
}

impl Ratio {
    /// What an inverse of a ratio that does not exist yet reads as. C#'s
    /// `DisplayInverseRatio` over a null source never publishes, and its values
    /// stay at `default(double)`.
    pub const ZERO: Ratio = Ratio { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// `new DisplayRatioValue(r)`: the same factor on both axes.
    pub const fn uniform(r: f64) -> Self {
        Self::new(r, r)
    }

    /// `DisplayRatioRatio`: `a.X * b.X`, `a.Y * b.Y`.
    pub fn multiply(self, other: Ratio) -> Ratio {
        Ratio::new(self.x * other.x, self.y * other.y)
    }

    /// `DisplayInverseRatio`: `1 / x`, `1 / y`.
    pub fn inverse(self) -> Ratio {
        Ratio::new(1.0 / self.x, 1.0 / self.y)
    }

    /// `DisplayRatio.IsUnary`: both factors within `double.Epsilon` of 1, which
    /// is to say exactly 1.
    pub fn is_unary(self) -> bool {
        (self.x - 1.0).abs() < f64::from_bits(1) && (self.y - 1.0).abs() < f64::from_bits(1)
    }
}

/// `DisplayInverseRatio` over a ratio that may not exist: the inverse when it
/// does, [`Ratio::ZERO`] when it does not (see there).
pub fn inverse_of(ratio: Option<Ratio>) -> Ratio {
    ratio.map_or(Ratio::ZERO, Ratio::inverse)
}
