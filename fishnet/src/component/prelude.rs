//! commonly used imports for building components.

pub use crate::c;

// components itself
pub use super::{BuildableComponent, ComponentState};
pub use crate::component::component;

// html, js, css
pub use crate::js::ScriptType;
pub use fishnet_macros::css;
pub use maud::{html, Markup, Render};

// boxing runner futures
pub use futures::future::{BoxFuture, FutureExt};

// sharing state
pub use std::sync::Arc;
pub use tokio::sync::Mutex;
// extracting state
pub use axum::Extension;

// endpoints
pub use axum::routing;
