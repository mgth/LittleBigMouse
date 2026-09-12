//! `RotateChainTests.cs`. A freshly built Rotate(1) chain must expose the
//! transposed size right away: the factory places monitors right after building
//! it, so a deferred first value would let the placement run on the untransposed
//! size. The source must be the INTRINSIC (EDID, landscape) size — the chain owns
//! the transposition (#507). In Rust the chain is a sequence of calls on a value,
//! so "right away" is the return value.

use lbm_layout::geo::Point;
use lbm_layout::model::{MmSize, Ratio};

fn landscape() -> MmSize {
    let mut size = MmSize::default();
    size.set_width(697.0);
    size.set_height(392.0);
    size
}

/// C#: `RotateChainTests.Rotate1_ExposesTransposedSize_Immediately`.
#[test]
fn rotate1_exposes_transposed_size_immediately() {
    let rotated = landscape().as_display_size().rotate(1);

    assert_eq!(rotated.width, 392.0);
    assert_eq!(rotated.height, 697.0);
}

/// C#: `RotateChainTests.FullDepthChain_Rotate1_ExposesTransposedSize_Immediately`.
#[test]
fn full_depth_chain_rotate1_exposes_transposed_size_immediately() {
    // `Locate()` without a point: `new Point()`.
    let chain = landscape()
        .as_display_size()
        .rotate(1)
        .scale(Ratio::new(1.0, 1.0))
        .located(Point::default());

    assert_eq!(chain.width, 392.0);
    assert_eq!(chain.height, 697.0);
}
