//! Graphics and paged rendering framework module.

pub mod buffer;
pub mod paged;

pub use buffer::{
    Gray4Color, Gray4Polarity, GrayBufferPair, PageBuffer, PageBufferPair, PlanePolarity, TriColor,
};
pub use paged::{render_paged, render_paged_gray4, render_paged_tri_color};
