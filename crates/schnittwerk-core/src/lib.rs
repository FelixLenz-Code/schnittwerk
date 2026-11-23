//! Schnittwerk core: image tracing, vector import and (later) cutting logic.
//! Contains no GUI or USB code so it can be tested on its own.

pub mod centerline;
pub mod contour;
pub mod cut;
pub mod field;
pub mod geom;
pub mod gpgl;
pub mod offset;
pub mod preprocess;
pub mod sticker;
pub mod svg_import;
pub mod text;
pub mod trace;

pub use svg_import::{import_svg, ImportedVector};
pub use trace::{trace, TraceLayer, TraceMode, TraceParams, TraceResult};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Bild konnte nicht gelesen werden: {0}")]
    Image(#[from] image::ImageError),
    #[error("SVG konnte nicht gelesen werden: {0}")]
    Svg(#[from] usvg::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
