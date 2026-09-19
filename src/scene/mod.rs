pub mod generator;
pub mod model;
pub mod primitives;
pub mod rng;

pub use generator::{RenderOptions, default_seed, generate};
pub use model::{SceneModel, Style};
