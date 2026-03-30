//! The foundational building blocks of the UI layout engine.
//!
//! This module contains the specialized implementations of the `Widget` trait,
//! ranging from basic visual elements like text and images to complex
//! structural managers like flex containers.
//!
//! ### Architecture
//! Each widget in this module follows a consistent lifecycle:
//! 1. **Data Association:** Using a [`WidgetId`], a widget retrieves its
//!    specific data and configuration from a [`CompileCtx`].
//! 2. **Compilation:** The widget validates itself against spatial
//!    constraints and prepares its internal render state (e.g., building
//!    a Skia `Paragraph`).
//! 3. **Drawing:** The widget renders its visual representation onto a
//!    provided Skia surface.
//!
//! ### Core Widget Types
//! The module is organized into three primary categories:
//!
//! * **Visual Elements:**
//!   - [`Text`]: High-performance typography with support for mixed-style entities.
//!   - [`Image`]: Scalable pixel data with aspect-ratio awareness and mipmapping.
//!
//! * **Layout Containers:**
//!   - [`Container`]: A fixed-size wrapper for a single child, providing
//!     borders and backgrounds.
//!   - [`FlexContainer`]: A dynamic, axis-based manager for multiple children
//!     supporting complex alignments (Rows/Columns).
//!
//! * **Structural Helpers:**
//!   - [`Unknown`]: A Zero-Sized Type (ZST) used as a "no-op" placeholder
//!     to simplify macro-driven logic and handle failed compilations gracefully.
//!
//! ### Uniformity via Macros
//! By grouping these types here, the engine can utilize declarative macros
//! to perform uniform operations across all variants of the `Widget` enum
//! without needing to account for the internal complexity of each specific type.

pub mod container;
pub mod flex_container;
pub mod image;
pub mod text;
pub(crate) mod unknown;

pub(crate) use unknown::Unknown;

pub use {
    container::{
        Container, ContainerBuilder, ContainerBuilderError, ContainerConfiguration,
        ContainerGBuilder,
    },
    flex_container::{
        FlexContainer, FlexContainerBuilder, FlexContainerBuilderError, FlexContainerGBuilder,
    },
    image::{Image, ImageBuilder, ImageBuilderError, ImageConfiguration, ImageGBuilder},
    text::{
        Font, FontGBuilder, FontStyle, Text, TextAlignment, TextBuilder, TextBuilderError,
        TextConfiguration, TextGBuilder,
    },
};
