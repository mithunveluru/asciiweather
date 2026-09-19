//! asciiweather — real weather, redrawn as procedural ASCII art.
//!
//! The pipeline is deliberately one-directional:
//!
//! ```text
//! provider -> WeatherData -> SceneModel -> styled lines -> stdout
//! ```
//!
//! Each arrow is a hard boundary: provider JSON never reaches the renderer, and
//! the renderer never asks what the weather *means*.

pub mod cli;
pub mod config;
pub mod error;
pub mod render;
pub mod scene;
pub mod weather;
