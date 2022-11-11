#![warn(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub mod app;
pub mod components;
mod desync;
pub mod error;
mod widget;

pub use widget::*;
