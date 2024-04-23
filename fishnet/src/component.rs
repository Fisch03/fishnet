//! reusable components for building web pages (aka the main thing)

pub mod prelude;

mod build;
pub use build::{BuildableComponent, BuiltComponent, ComponentBuildResult};

mod render;
use render::ContentRenderer;

pub mod fake_macros;

use crate::css::StyleFragment;
use crate::js::ScriptType;
use crate::routes::ComponentRoute;

use axum::{
    body::Body, http::Request, response::IntoResponse, routing::method_routing::MethodRouter,
    Router,
};
use core::convert::Infallible;
use futures::future::BoxFuture;
use maud::Markup;
use std::{fmt::Debug, marker::PhantomData, ops::Deref};
use tower_service::Service;

#[derive(Debug, Clone)]
pub struct ComponentState<ST>
where
    ST: Clone + Send + Sync,
{
    api_route: ComponentRoute,
    state: ST,
}
impl<ST> ComponentState<ST>
where
    ST: Clone + Send + Sync,
{
    pub fn endpoint(&self) -> &str {
        self.api_route.as_str()
    }
}
impl<ST> Deref for ComponentState<ST>
where
    ST: Clone + Send + Sync,
{
    type Target = ST;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

pub type ComponentRunner<ST> =
    Box<dyn FnOnce(ComponentState<ST>) -> BoxFuture<'static, ()> + Send + Sync + 'static>;

#[doc(hidden)]
pub struct NoRenderer;
#[doc(hidden)]
pub struct HasRenderer;
#[doc(hidden)]
pub struct NoState;
#[doc(hidden)]
pub struct FixedNoState;
#[doc(hidden)]
pub struct HasState;
pub struct Component<R, S, ST>
where
    ST: Clone + Send + Sync,
{
    name: String,
    id: String,
    is_dynamic: bool,

    renderer: Option<ContentRenderer<ST>>,

    state: ST,
    router: Option<Router>,

    runner: Option<ComponentRunner<ST>>,
    scripts: Vec<ScriptType>,
    style: Option<StyleFragment<'static>>,

    _renderer_state: PhantomData<R>,
    _state_state: PhantomData<S>,
}

// ---- constructor ----
impl Component<NoRenderer, NoState, ()> {
    pub fn new(name: &str, id: &str) -> Component<NoRenderer, NoState, ()> {
        Self {
            name: name.to_string(),
            id: id.to_string(),

            is_dynamic: false,

            state: (),
            router: None,

            renderer: None,
            runner: None,

            scripts: Vec::new(),
            style: None,

            _renderer_state: PhantomData,
            _state_state: PhantomData,
        }
    }
}

// ---- on all components ----
impl<R, S, ST> Component<R, S, ST>
where
    ST: Clone + Send + Sync,
{
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_script(mut self, script: ScriptType) -> Self {
        self.scripts.push(script);
        self
    }

    pub fn style(mut self, style: StyleFragment<'static>) -> Self {
        self.style = Some(style);
        self
    }

    pub fn route(mut self, path: &str, route: MethodRouter) -> Component<R, S, ST> {
        let router = self.router.unwrap_or_else(|| Router::new());
        self.router = Some(router.route(path, route));
        self
    }

    pub fn nest_service<T>(mut self, path: &str, service: T) -> Self
    where
        T: Service<Request<Body>, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        let router = self.router.unwrap_or_else(|| Router::new());
        self.router = Some(router.nest_service(path, service));
        self
    }
}

// ---- adding a renderer ----
impl<S, ST> Component<NoRenderer, S, ST>
where
    ST: Clone + Send + Sync,
    S: Send + Sync + 'static,
{
    pub fn render<C>(self, renderer: C) -> impl BuildableComponent
    where
        ST: Clone + Send + Sync + 'static,
        C: Fn(ComponentState<ST>) -> BoxFuture<'static, Markup> + Send + Sync + 'static,
    {
        Component::<HasRenderer, S, ST> {
            name: self.name,
            id: self.id,

            is_dynamic: false,

            state: self.state,
            router: self.router,

            renderer: Some(Box::new(renderer)),
            runner: self.runner,

            scripts: self.scripts,
            style: self.style,

            _renderer_state: PhantomData,
            _state_state: PhantomData,
        }
    }

    pub fn render_dynamic<C>(self, renderer: C) -> impl BuildableComponent
    where
        ST: Clone + Send + Sync + 'static,
        C: Fn(ComponentState<ST>) -> BoxFuture<'static, Markup> + Send + Sync + 'static,
    {
        Component::<HasRenderer, S, ST> {
            name: self.name,
            id: self.id,
            is_dynamic: true,
            state: self.state,
            router: self.router,
            renderer: Some(Box::new(renderer)),
            runner: self.runner,
            scripts: self.scripts,
            style: self.style,
            _renderer_state: PhantomData,
            _state_state: PhantomData,
        }
    }
}

// ---- adding a runner ----
impl<R> Component<R, FixedNoState, ()> {
    pub fn with_runner<F>(mut self, runner: ComponentRunner<()>) -> Self {
        self.runner = Some(Box::new(runner));
        self
    }
}
impl<R, ST> Component<R, HasState, ST>
where
    ST: Clone + Send + Sync + 'static,
{
    pub fn with_runner<F>(mut self, runner: F) -> Self
    where
        F: FnOnce(ComponentState<ST>) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        self.runner = Some(Box::new(runner));
        self
    }
}

// ---- setting state ----
impl<R> Component<R, NoState, ()> {
    /// Add state to the component.
    /// Since the state will be passed around a lot, it should be cheap to clone. This usually
    /// means wrapping it in an Arc or similar.
    pub fn with_state<ST>(self, state: ST) -> Component<NoRenderer, HasState, ST>
    where
        ST: Clone + Send + Sync + 'static,
    {
        Component::<NoRenderer, HasState, ST> {
            name: self.name,
            id: self.id,

            is_dynamic: self.is_dynamic,

            state,
            router: self.router,

            renderer: None, // this is fine because there is no renderer on the component yet
            runner: None,   // runners can also only be added after with_state

            scripts: self.scripts,
            style: self.style,

            _renderer_state: PhantomData,
            _state_state: PhantomData,
        }
    }
}
