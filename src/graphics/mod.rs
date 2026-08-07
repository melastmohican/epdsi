//! Graphics and paged rendering framework module.

pub mod buffer;
pub mod paged;

pub use buffer::PageBuffer;
pub use paged::render_paged;
