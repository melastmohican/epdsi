//! Graphics and paged rendering framework module.

pub mod buffer;
pub mod paged;

pub use buffer::{PageBuffer, PageBufferPair, TriColor};
pub use paged::{render_paged, render_paged_tri_color};
