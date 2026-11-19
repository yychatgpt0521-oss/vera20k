//! Cell addressing: retail 512-wide indexing and map-owned extended layouts.
//!
//! Native helpers preserve retail aliases; loaded-map consumers select their
//! layout through CellGrid without changing a process-wide mode.
//! `sim::cell_rect` re-exports these for its runtime consumers. The square
//! rings native cell searches walk ([`square_ring_cells`]) live here too.
//!
//! ## Dependency rules
//! - Part of map/; depends on nothing above std.

use std::ops::RangeInclusive;

/// Retained CellClass allocation identity after a fixed-stride lookup. A real
/// index addresses the loaded terrain storage; Dummy always names the same
/// process object even when a later failed lookup changes its coordinate.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum NativeCellIdentity {
    Real(usize),
    Dummy,
}

/// Fixed cell-array stride — the engine indexes cells `y*0x200 + x` regardless of
/// the loaded map's playfield width. The valid linear range is `[0, MAX_CELL_INDEX]`.
/// This is NOT the loaded-map width index (that is `PathGrid`'s `y*width+x` cache).
pub const CELL_ROW_STRIDE: i64 = 512;
/// Capacity supported by this engine. A map chooses its addressing layout below.
pub const MAX_CELL_SIDE: u16 = 1024;
pub const MAX_CELL_INDEX: i64 = CELL_ROW_STRIDE * CELL_ROW_STRIDE - 1;

/// Addressing belongs to a map, never to a process-wide or thread-local switch.
/// Retail-sized maps retain packed 512-stride aliases; larger maps use 1024.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CellGrid {
    #[default]
    Retail,
    Extended,
}

impl CellGrid {
    pub const fn for_map_size(width: i32, height: i32) -> Self {
        if width as i64 + height as i64 > CELL_ROW_STRIDE {
            Self::Extended
        } else {
            Self::Retail
        }
    }

    pub const fn for_extent(width: u16, height: u16) -> Self {
        if width > CELL_ROW_STRIDE as u16 || height > CELL_ROW_STRIDE as u16 {
            Self::Extended
        } else {
            Self::Retail
        }
    }

    pub const fn shift(self) -> u32 {
        match self {
            Self::Retail => 9,
            Self::Extended => 10,
        }
    }

    pub const fn side(self) -> u16 {
        1u16 << self.shift()
    }

    pub const fn is_extended(self) -> bool {
        matches!(self, Self::Extended)
    }

    pub fn linear_index(self, x: i32, y: i32) -> Option<i64> {
        let (x, y) = packed_cell_coord(x, y);
        let side = i64::from(self.side());
        let index = (i64::from(y) << self.shift()) + i64::from(x);
        (0..side * side).contains(&index).then_some(index)
    }

    pub fn coord_from_index(self, index: i64) -> Option<(u16, u16)> {
        let shift = self.shift();
        let side = 1i64 << shift;
        (0..side * side)
            .contains(&index)
            .then_some(((index & (side - 1)) as u16, (index >> shift) as u16))
    }

    pub fn canonical_coord(self, x: i32, y: i32) -> Option<(u16, u16)> {
        self.linear_index(x, y)
            .and_then(|index| self.coord_from_index(index))
    }
}

/// Cell coordinates cross the native seam as packed 16-bit words. Normalize at
/// that seam so the Rust-facing `i32` parameters cannot retain non-native high
/// bits.
pub(crate) const fn packed_cell_coord(x: i32, y: i32) -> (i32, i32) {
    (x as i16 as i32, y as i16 as i32)
}

/// Linear cell index using the fixed 512-wide stride (NOT the loaded-map width).
///
/// Returns `None` only when the index falls outside `[0, MAX_CELL_INDEX]`; the
/// dummy fallback (`get_cellclass_fallback`) turns that `None` into a non-null
/// reference, mirroring the engine's never-null `Get_CellClass`.
pub fn cell_linear_index(x: i32, y: i32) -> Option<i64> {
    CellGrid::Retail.linear_index(x, y)
}

/// Canonical real slot selected by the packed fixed-stride Map lookup.
pub(crate) fn canonical_cell_coord(x: i32, y: i32) -> Option<(u16, u16)> {
    CellGrid::Retail.canonical_coord(x, y)
}

/// The cells of square ring `r` around `center` (`max(|dx|, |dy|) == r`), in
/// the fixed order gamemd's ring searches walk them:
///
/// 1. for `d = -r ..= r`: the north cell `(x + d, y - r)`, then the south cell
///    `(x + d, y + r)` — both apex rows together, west to east.
/// 2. for `e = 1 - r ..= r - 1`: the west cell `(x - r, y + e)`, then the east
///    cell `(x + r, y + e)` — the side columns' interior rows, north to south.
///
/// This is NOT a Manhattan diamond and NOT a continuous clockwise walk. At
/// `r == 0` the first loop runs once and north and south coincide, so the
/// centre comes TWICE (the second loop is empty): native's actual candidate
/// stream, which decides ties and candidate counts, so it is not deduplicated.
/// `TechnoClass::Greatest_Threat @ 0x006F8DF0` walks its rings this way, and so
/// does the nearby-cell search ([`crate::sim::find_nearby_cell`]).
pub(crate) fn square_ring_cells(center: (i32, i32), r: i32) -> impl Iterator<Item = (i32, i32)> {
    let (x, y) = center;
    let rows = (-r..=r).flat_map(move |d| [(x + d, y - r), (x + d, y + r)]);
    let columns = (1 - r..r).flat_map(move |e| [(x - r, y + e), (x + r, y + e)]);
    rows.chain(columns)
}

/// The cells of [`square_ring_cells`] inside the box `xs` × `ys`, in the same
/// order. VERA-only: it steps over only the stretches of the ring that cross
/// the box, so a ring far wider than the map costs the map's cells.
pub(crate) fn square_ring_cells_within(
    center: (i32, i32),
    r: i32,
    xs: &RangeInclusive<i32>,
    ys: &RangeInclusive<i32>,
) -> impl Iterator<Item = (i32, i32)> + use<> {
    let (x, y) = center;
    let (north, south, west, east) = (y - r, y + r, x - r, x + r);
    let [north_in, south_in] = [north, south].map(|row| ys.contains(&row));
    let [west_in, east_in] = [west, east].map(|column| xs.contains(&column));
    // The steps whose cell can fall in the box; none when neither side can.
    let steps = |within: &RangeInclusive<i32>, from: i32, low: i32, high: i32, any: bool| {
        let low = low.max(within.start().saturating_sub(from));
        let high = high.min(within.end().saturating_sub(from));
        if any { low..=high } else { 1..=0 }
    };
    let rows = steps(xs, x, -r, r, north_in || south_in).flat_map(move |d| {
        let (north, south) = ((x + d, north), (x + d, south));
        [north_in.then_some(north), south_in.then_some(south)]
            .into_iter()
            .flatten()
    });
    let columns = steps(ys, y, 1 - r, r - 1, west_in || east_in).flat_map(move |e| {
        let (west, east) = ((west, y + e), (east, y + e));
        [west_in.then_some(west), east_in.then_some(east)]
            .into_iter()
            .flatten()
    });
    rows.chain(columns)
}

#[cfg(all(test, tests_map))]
mod tests {
    use super::*;

    #[test]
    fn a_ring_within_a_box_keeps_the_rings_order() {
        let (xs, ys) = (3..=9, 2..=12);
        let filtered = |center, r| -> Vec<_> {
            square_ring_cells(center, r)
                .filter(|(x, y)| xs.contains(x) && ys.contains(y))
                .collect()
        };
        let within =
            |center, r| -> Vec<_> { square_ring_cells_within(center, r, &xs, &ys).collect() };
        // Both apex rows cut at both ends; rows cut at one end and one side
        // column; the centre twice at r 0; a ring wholly outside.
        for (center, r) in [((6, 7), 5), ((8, 7), 4), ((4, 6), 0), ((40, 40), 3)] {
            assert_eq!(within(center, r), filtered(center, r), "{center:?} r {r}");
        }
        assert_eq!(within((6, 7), 5).len(), 14);
        assert_eq!(within((4, 6), 0), [(4, 6), (4, 6)]);
        assert!(within((40, 40), 3).is_empty());
    }

    #[test]
    fn large_map_addressing_is_map_owned_and_preserves_retail_aliases() {
        let retail = CellGrid::for_map_size(128, 128);
        let large = CellGrid::for_map_size(500, 500);
        assert_eq!(retail.canonical_coord(512, 0), Some((0, 1)));
        assert_eq!(large.canonical_coord(512, 0), Some((512, 0)));
        assert_eq!(retail.canonical_coord(512, 0), Some((0, 1)));
        assert_eq!(CellGrid::for_extent(256, 256), retail);
        assert_eq!(CellGrid::for_extent(1000, 1000), large);
        assert_eq!(cell_linear_index(0xffff, 1), Some(511));
    }
}
