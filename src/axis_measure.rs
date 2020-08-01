use druid::{Cursor, EventCtx, Point, Rect, Selector, Size, Data};
use float_ord::FloatOrd;
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::{Debug, Formatter};
use crate::config::{DEFAULT_COL_HEADER_HEIGHT, DEFAULT_ROW_HEADER_WIDTH};
use TableAxis::*;
use crate::Remap;
use crate::data::{RemapDetails, SortSpec};
use std::ops::{Add, Sub, RangeInclusive};
use std::iter::Map;

#[derive(Debug, Clone, Copy)]
pub enum TableAxis {
    Rows,
    Columns,
}

impl TableAxis {
    pub fn cross_axis(&self) -> TableAxis {
        match self {
            Rows => Columns,
            Columns => Rows,
        }
    }

    pub fn main_pixel_from_point(&self, point: &Point) -> f64 {
        match self {
            Rows => point.y,
            Columns => point.x,
        }
    }

    pub fn pixels_from_rect(&self, rect: &Rect) -> (f64, f64) {
        match self {
            Rows => (rect.y0, rect.y1),
            Columns => (rect.x0, rect.x1),
        }
    }

    pub fn default_header_cross(&self) -> f64 {
        match self {
            Rows => DEFAULT_ROW_HEADER_WIDTH,
            Columns => DEFAULT_COL_HEADER_HEIGHT,
        }
    }

    pub fn coords(&self, main: f64, cross: f64) -> (f64, f64) {
        match self {
            Rows => (cross, main),
            Columns => (main, cross),
        }
    }

    pub fn size(&self, main: f64, cross: f64) -> Size {
        self.coords(main, cross).into()
    }

    pub fn cell_origin(&self, main: f64, cross: f64) -> Point {
        self.coords(main, cross).into()
    }

    pub fn resize_cursor(&self) -> &'static Cursor {
        match self {
            Rows => &Cursor::ResizeUpDown,
            Columns => &Cursor::ResizeLeftRight,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AxisMeasureAdjustment {
    LengthChanged(TableAxis, VisIdx, f64),
    RemapChanged(TableAxis, Remap)
}

pub const ADJUST_AXIS_MEASURE: Selector<AxisMeasureAdjustment> =
    Selector::new("druid-builtin.table.adjust-measure");

pub type AxisMeasureAdjustmentHandler = dyn Fn(&mut EventCtx, &AxisMeasureAdjustment);

#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Data)]
pub struct VisIdx(pub(crate) usize);
#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Data)]
pub struct LogIdx(pub(crate) usize);

impl VisIdx{
    // Todo work out how to support custom range
    pub fn range_inc_iter(from_inc: VisIdx, to_inc: VisIdx) -> Map<RangeInclusive<usize>, fn(usize) -> VisIdx> {
        ((from_inc.0)..=(to_inc.0)).into_iter().map(VisIdx)
    }
}

impl Add<usize> for VisIdx{
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        VisIdx(self.0 + rhs)
    }
}

impl Sub<usize> for VisIdx{
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        VisIdx(self.0 - rhs)
    }
}

pub trait AxisMeasure: Clone {
    fn border(&self) -> f64;
    fn set_axis_properties(&mut self, border: f64, len: usize, remap: &Remap);
    fn total_pixel_length(&self) -> f64;
    fn vis_from_pixel(&self, pixel: f64) -> Option<VisIdx>;
    fn vis_range_from_pixels(&self, p0: f64, p1: f64) -> (VisIdx, VisIdx);
    fn first_pixel_from_vis(&self, idx: VisIdx) -> Option<f64>;
    fn pixels_length_for_vis(&self, idx: VisIdx) -> Option<f64>;
    fn set_far_pixel_for_vis(&mut self, idx: VisIdx, pixel: f64) -> f64;
    fn set_pixel_length_for_vis(&mut self, idx: VisIdx, length: f64) -> f64;
    fn can_resize(&self, idx: VisIdx) -> bool;

    fn pixel_near_border(&self, pixel: f64) -> Option<VisIdx> {
        let idx = self.vis_from_pixel(pixel)?;
        let idx_border_middle = self.first_pixel_from_vis(idx).unwrap_or(0.) - self.border() / 2.;
        let next_border_middle = self
            .first_pixel_from_vis(idx + 1)
            .unwrap_or_else(|| self.total_pixel_length())
            - self.border() / 2.;
        if f64::abs(pixel - idx_border_middle) < MOUSE_MOVE_EPSILON {
            Some(idx)
        } else if f64::abs(pixel - next_border_middle) < MOUSE_MOVE_EPSILON {
            Some(idx + 1)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FixedAxisMeasure {
    pixels_per_unit: f64,
    border: f64,
    len: usize,
}

impl FixedAxisMeasure {
    pub fn new(pixels_per_unit: f64) -> Self {
        FixedAxisMeasure {
            pixels_per_unit,
            border: 0.,
            len: 0,
        }
    }

    fn full_pixels_per_unit(&self) -> f64 {
        self.pixels_per_unit + self.border
    }
}

const MOUSE_MOVE_EPSILON: f64 = 3.;

impl AxisMeasure for FixedAxisMeasure {
    fn border(&self) -> f64 {
        self.border
    }

    fn set_axis_properties(&mut self, border: f64, len: usize, _remap: &Remap) {
        self.border = border;
        self.len = len;
        // We don't care about remap as every item is the same... I think.
        // TODO: Maybe we should care about the length?
    }

    fn total_pixel_length(&self) -> f64 {
        self.full_pixels_per_unit() * (self.len as f64)
    }

    fn vis_from_pixel(&self, pixel: f64) -> Option<VisIdx> {
        let index = (pixel / self.full_pixels_per_unit()).floor() as usize;
        if index < self.len {
            Some(VisIdx(index))
        } else {
            None
        }
    }

    fn vis_range_from_pixels(&self, p0: f64, p1: f64) -> (VisIdx, VisIdx) {
        let start = self.vis_from_pixel(p0);
        let end = self.vis_from_pixel(p1);

        let start = start.unwrap_or(VisIdx(0));
        let end = end.unwrap_or(VisIdx(self.len - 1));
        (start, end)
    }

    fn first_pixel_from_vis(&self, idx: VisIdx) -> Option<f64> {
        if idx.0 < self.len {
            Some((idx.0 as f64) * self.full_pixels_per_unit())
        } else {
            None
        }
    }

    fn pixels_length_for_vis(&self, idx: VisIdx) -> Option<f64> {
        if idx.0 < self.len {
            Some(self.pixels_per_unit)
        } else {
            None
        }
    }

    fn set_far_pixel_for_vis(&mut self, _idx: VisIdx, _pixel: f64) -> f64 {
        self.pixels_per_unit
    }

    fn set_pixel_length_for_vis(&mut self, _idx: VisIdx, _length: f64) -> f64 {
        self.pixels_per_unit
    }

    fn can_resize(&self, _idx: VisIdx) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct StoredAxisMeasure {
    remap: Remap,
    log_pix_lengths: Vec<f64>,
    vis_pix_lengths: Vec<f64>,
    first_pixels: BTreeMap<VisIdx, f64>, // TODO newtypes
    pixels_to_vis: BTreeMap<FloatOrd<f64>, VisIdx>,
    default_pixels: f64,
    border: f64,
    total_pixel_length: f64,
}

struct DebugFn<'a, F: Fn(&mut Formatter) -> fmt::Result>(&'a F);

impl<'a, F: Fn(&mut Formatter) -> fmt::Result> Debug for DebugFn<'a, F> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let func = self.0;
        (func)(f)
    }
}

macro_rules! debug_fn {
    ($content: expr) => {
        &DebugFn(&$content)
    };
}

impl Debug for StoredAxisMeasure {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        let fp = &self.first_pixels;
        let pti = &self.pixels_to_vis;
        fmt.debug_struct("StoredAxisMeasure")
            .field("log_pix_lengths", &self.log_pix_lengths)
            .field("vis_pix_lengths", &self.vis_pix_lengths)
            .field("default_pixels", &self.default_pixels)
            .field("border", &self.border)
            .field("total_pixel_length", &self.total_pixel_length)
            .field(
                "first_pixels",
                debug_fn!(|f| f.debug_map().entries(fp.iter()).finish()),
            )
            .field(
                "pixels_to_index",
                debug_fn!(|f| f
                    .debug_map()
                    .entries(pti.iter().map(|(k, v)| (k.0, v)))
                    .finish()),
            )
            .finish()
    }
}

impl StoredAxisMeasure {
    pub fn new(default_pixels: f64) -> Self {
        StoredAxisMeasure {
            remap: Remap::Pristine,
            log_pix_lengths: Default::default(),
            vis_pix_lengths: Default::default(),
            first_pixels: Default::default(),
            pixels_to_vis: Default::default(),
            default_pixels,
            border: 0.,
            total_pixel_length: 0.,
        }
    }

    fn build_maps(&mut self) {
        let mut cur = 0.;
        self.vis_pix_lengths.clear();
        match &self.remap {
            Remap::Selected(RemapDetails::Full(vis_to_log))=>{
                for log_idx in vis_to_log{
                    self.vis_pix_lengths.push( self.log_pix_lengths[log_idx.0] );
                }
            }
            _=>self.vis_pix_lengths.extend_from_slice( &self.log_pix_lengths )
        }


        self.first_pixels.clear();
        self.pixels_to_vis.clear();
        for (idx, pixels) in self.vis_pix_lengths.iter().enumerate() {
            self.first_pixels.insert(VisIdx(idx), cur);
            self.pixels_to_vis.insert(FloatOrd(cur), VisIdx(idx));
            cur += pixels + self.border;
        }
        self.total_pixel_length = cur;
    }
}

impl AxisMeasure for StoredAxisMeasure {
    fn border(&self) -> f64 {
        self.border
    }

    fn set_axis_properties(&mut self, border: f64, len: usize, remap: &Remap) {
        self.border = border;
        self.remap = remap.clone(); // Todo: pass by ref where needed? Or make the measure own it


        let old_len = self.log_pix_lengths.len();
        if old_len > len {
            self.log_pix_lengths.truncate(len)
        }else if old_len < len{
            let extra = vec![self.default_pixels; len - old_len];
            self.log_pix_lengths.extend_from_slice( &extra[..] );
            assert_eq!(self.log_pix_lengths.len(), len);
        }

        // TODO: handle renumbering / remapping. Erk.
        self.build_maps()
    }

    fn total_pixel_length(&self) -> f64 {
        self.total_pixel_length
    }

    fn vis_from_pixel(&self, pixel: f64) -> Option<VisIdx> {
        self.pixels_to_vis
            .range(..=FloatOrd(pixel))
            .next_back()
            .map(|(_, v)| *v)
    }

    fn vis_range_from_pixels(&self, p0: f64, p1: f64) -> (VisIdx, VisIdx) {
        (
            self.vis_from_pixel(p0).unwrap_or(VisIdx(0)),
            self.vis_from_pixel(p1)
                .unwrap_or(VisIdx(self.vis_pix_lengths.len() - 1)),
        )
    }

    fn first_pixel_from_vis(&self, idx: VisIdx) -> Option<f64> {
        self.first_pixels.get(&idx).copied()
    }

    fn pixels_length_for_vis(&self, idx: VisIdx) -> Option<f64> {
        self.vis_pix_lengths.get(idx.0).copied()
    }

    fn set_far_pixel_for_vis(&mut self, idx: VisIdx, pixel: f64) -> f64 {
        let length = f64::max(0., pixel - *self.first_pixels.get(&idx).unwrap_or(&0.));
        self.set_pixel_length_for_vis(idx, length)
    }

    fn set_pixel_length_for_vis(&mut self, vis_idx: VisIdx, length: f64) -> f64 {
        // Todo Option
        if let Some(log_idx) = self.remap.get_log_idx(vis_idx) {
            if let Some(place) = self.log_pix_lengths.get_mut(log_idx.0) {
                *place = length;
                self.build_maps(); // TODO : modify efficiently instead of rebuilding
                return length
            }
        }
        0.
    }

    fn can_resize(&self, _idx: VisIdx) -> bool {
        true
    }
}

#[cfg(test)]
mod test {
    use crate::{AxisMeasure, FixedAxisMeasure, StoredAxisMeasure, Remap};
    use float_ord::FloatOrd;
    use std::collections::HashSet;
    use std::fmt::Debug;
    use crate::axis_measure::VisIdx;

    #[test]
    fn fixed_axis() {
        let mut ax = FixedAxisMeasure::new(99.0);

        test_equal_sized(&mut ax);
        assert_eq!(ax.set_far_pixel_for_vis(VisIdx(12), 34.), 99.);
    }

    fn test_equal_sized<AX: AxisMeasure + Debug>(ax: &mut AX) {
        ax.set_axis_properties(1.0, 4, &Remap::Pristine);
        println!("Axis:{:#?}", ax);
        assert_eq!(ax.total_pixel_length(), 400.);
        assert_eq!(ax.vis_from_pixel(350.0), Some(VisIdx(3)));
        assert_eq!(ax.first_pixel_from_vis(VisIdx(0)), Some(0.));
        assert_eq!(ax.vis_from_pixel(0.0), Some(VisIdx(0)));
        assert_eq!(ax.vis_from_pixel(100.0), Some(VisIdx(1)));
        assert_eq!(ax.vis_from_pixel(1.0), Some(VisIdx(0)));
        assert_eq!(ax.first_pixel_from_vis(VisIdx(1)), Some(100.0));

        assert_eq!(
            (199..=201)
                .into_iter()
                .map(|n| ax.vis_from_pixel(n as f64).unwrap())
                .collect::<Vec<VisIdx>>(),
            vec![VisIdx(1), VisIdx(2), VisIdx(2)]
        );

        assert_eq!(ax.vis_range_from_pixels(105.0, 295.0), (VisIdx(1), VisIdx(2)));
        assert_eq!(ax.vis_range_from_pixels(100.0, 300.0), (VisIdx(1), VisIdx(3)));
        let lengths = (1usize..=3)
            .into_iter()
            .map(|i| FloatOrd(ax.pixels_length_for_vis(VisIdx(i)).unwrap()))
            .collect::<HashSet<FloatOrd<f64>>>();

        assert_eq!(lengths.len(), 1);
        assert_eq!(lengths.iter().next().unwrap().0, 99.0)
    }

    #[test]
    fn stored_axis() {
        let mut ax = StoredAxisMeasure::new(99.);
        test_equal_sized(&mut ax);

        assert_eq!(ax.set_pixel_length_for_vis(VisIdx(2), 49.), 49.);
        assert_eq!(ax.set_far_pixel_for_vis(VisIdx(1), 109.), 9.);
        assert_eq!(ax.total_pixel_length(), 260.0)
    }
}
