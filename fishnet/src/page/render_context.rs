use axum::routing::Router;
use futures::future::BoxFuture;
use maud::{html, Markup};
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use tracing::{error, instrument, trace, warn};

use crate::component::{BuildableComponent, BuiltComponent};
use crate::page::BuiltPage;
use crate::routes::ComponentRoute;

fn render_context() -> &'static Mutex<Option<RenderContext>> {
    static RENDER_CONTEXT: OnceLock<Mutex<Option<RenderContext>>> = OnceLock::new();
    RENDER_CONTEXT.get_or_init(|| Mutex::new(None))
}

#[doc(hidden)]
pub fn global_store() -> &'static GlobalStore {
    static GLOBAL_STORE: OnceLock<GlobalStore> = OnceLock::new();
    GLOBAL_STORE.get_or_init(|| GlobalStore::new())
}

#[derive(Debug)]
pub(crate) struct ComponentStore(HashMap<String, BuiltComponent>);

impl ComponentStore {
    pub(crate) fn new() -> Self {
        Self(HashMap::new())
    }
}

#[derive(Debug)]
#[doc(hidden)]
pub struct GlobalStoreEntry {
    pub scripts: Vec<crate::js::ScriptType>,
    pub style: Option<crate::css::RenderedStyle>,
}

#[derive(Debug)]
pub struct GlobalStore(Mutex<HashMap<String, Arc<GlobalStoreEntry>>>);
impl GlobalStore {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    pub async fn add<F>(&self, id: &str, globals: F)
    where
        F: FnOnce() -> GlobalStoreEntry,
    {
        let mut store = self.0.lock().await;
        match store.entry(id.to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(Arc::new(globals()));
                let mut context = render_context().lock().await;
                if context.is_some() {
                    context.as_mut().unwrap().notify_global(&id)
                }
            }
            Entry::Occupied(_) => {}
        }
    }

    pub async fn get<'a>(&'a self, id: &str) -> Option<Arc<GlobalStoreEntry>> {
        self.0.lock().await.get(id).cloned()
    }
}

struct RenderContext {
    base_route: String,

    components: Arc<Mutex<ComponentStore>>,
    new_globals: HashSet<String>,

    static_state: bool,
    temporary_render_depth: usize,

    new_runners: Vec<BoxFuture<'static, ()>>,
    new_routers: Vec<(ComponentRoute, Router)>,
}
impl RenderContext {
    fn new(base_route: &str, components: Arc<Mutex<ComponentStore>>) -> RenderContext {
        Self {
            base_route: base_route.to_string(),

            components,

            static_state: false,
            temporary_render_depth: 0,

            new_runners: Vec::new(),
            new_routers: Vec::new(),
            new_globals: HashSet::new(),
        }
    }

    fn finish(self) -> RenderResult {
        RenderResult {
            runners: self.new_runners,
            routers: self.new_routers,
            new_components: self.new_globals,
        }
    }

    fn notify_global(&mut self, id: &str) {
        self.new_globals.insert(id.to_string());
    }
}

/// The result of a page render.
///
/// Contains all the scripts, runners and routers that were collected during the rendering.
/// * `scripts` - A list of scripts that should be included in the page.
/// * `runners` - A list of runners that should be executed.
/// * `routers` - A list of routers that should be accessible from the page. This is usually achieved using a [APIRouter](`crate::routes::APIRouter`).
pub struct RenderResult {
    pub runners: Vec<BoxFuture<'static, ()>>,
    pub routers: Vec<(ComponentRoute, Router)>,
    pub new_components: HashSet<String>,
}

/// Enter a page render context.
///
/// This should be called before rendering any components.
/// After the rendering is complete, `exit_page` should be called to acquire the results.
/// Calling `enter_page` while another page is being rendered results in the loss of the previous page's render results!
///
/// You usually don't need to call this function yourself.
pub(crate) async fn enter_page(page: &mut BuiltPage) {
    let mut context = render_context().lock().await;

    if context.is_some() {
        warn!("tried to render a page while another page is already being rendered");
    }

    context.replace(RenderContext::new(&page.api_path, page.components.clone()));
}

/// Exit a page render context.
///
/// This should be called after rendering all components. It will return the `RenderResult` containing all the scripts and runners that were collected during the rendering.
///
/// # Panics
/// Panics if no page is currently being rendered. (i.e. `enter_page` was not called before)
pub(crate) async fn exit_page() -> RenderResult {
    let mut context = render_context().lock().await;

    context
        .take()
        .expect("tried to exit a page while no page is being rendered")
        .finish()
}

/// Render a component into the current page render context.
///
/// This function should only be called while a page is being rendered.
/// It is highly recommended to use the [`c!`](crate::c!) macro instead of calling this function directly, since it will handle the context id generation automatically.
/// * `context_id` - A unique identifier for the render. This should be kept consistent for the same component across renders.
/// * `lazy_component` - A closure that returns the component to render. It will only be called if the component is not already rendered for the current page.
#[instrument(name = "c", level = "debug", skip_all)]
pub async fn render_component<F, C>(context_id: &str, lazy_component: F) -> Markup
where
    F: FnOnce() -> C,
    C: BuildableComponent,
{
    let mut context_guard = render_context().lock().await;
    if context_guard.is_none() {
        error!(
            context_id,
            "tried to add a component while no page is being rendered"
        );
        #[cfg(debug_assertions)]
        {
            return html! { "rendering failed for context " (context_id) ": no page is being rendered" };
        }
        #[cfg(not(debug_assertions))]
        {
            return html! {};
        }
    }
    let mut context = context_guard.as_mut().unwrap();
    let mut components_guard = context.components.lock().await;

    let render;
    let existing_component = components_guard.0.get(&context_id.to_string());
    if existing_component.is_some() {
        let component = existing_component.unwrap().clone(); //TODO: avoid this clone somehow?

        drop(components_guard);
        drop(context_guard);

        // IMPORTANT: Since may lead to recursive calls, all the locks need to be dropped before calling
        render = component.render().await;

        context_guard = render_context().lock().await;
        if context_guard.is_none() {
            error!(
                context_id,
                "page render exited while a component was still being rendered"
            );
            return html! { "rendering failed for context " (context_id) ": page render exited" };
        }
    } else {
        let base_route = context.base_route.clone();

        drop(components_guard);
        drop(context_guard);

        // IMPORTANT: Since may lead to recursive calls, all the locks need to be dropped before calling
        trace!("building component");
        let new_component = lazy_component().build(&base_route).await;
        trace!("rendering component");
        render = new_component.built_component.render().await;

        context_guard = render_context().lock().await;
        if context_guard.is_none() {
            error!(
                context_id,
                "page render exited while a component was still being rendered"
            );
            return html! { "rendering failed for context " (context_id) ": page render exited" };
        }

        context = context_guard.as_mut().unwrap();
        components_guard = context.components.lock().await;

        context.static_state &= !new_component.built_component.is_dynamic();

        if let Some(router) = new_component.router {
            context.new_routers.push(router)
        }
        if let Some(runner) = new_component.runner {
            context.new_runners.push(runner);
        }

        if !context.temporary_render_depth > 0 {
            components_guard
                .0
                .insert(context_id.to_string(), new_component.built_component);
        }
    }

    render
}

/// Enter a temporary render context.
///
/// While in a temporary render context, rendered components will not be saved to the page's component store.
/// Furthermore, it will be recorded, whether any dynamic components were rendered during the temporary render.
/// This can be used to "test-render" a static component to see if it contains any dynamic children.
///
/// Temporary render contexts can be exited using `exit_temporary_render`, and can be nested.
///
/// You usually don't need to call this function yourself.
pub(crate) async fn enter_temporary_render() {
    let mut context = render_context().lock().await;
    if let Some(context) = context.as_mut() {
        trace!("entering temporary render");
        if context.temporary_render_depth == 0 {
            context.static_state = true;
        }
        context.temporary_render_depth += 1;
    }
}

/// Exit a temporary render context.
///
/// Returns true if the temporary render was static (i.e. no dynamic components were rendered).
/// If not within a (temporary) render context, this function will always return true.
///
/// You usually don't need to call this function yourself.
pub(crate) async fn exit_temporary_render() -> bool {
    let mut context = render_context().lock().await;
    if let Some(context) = context.as_mut() {
        if context.temporary_render_depth == 0 {
            warn!("tried to exit temporary render while not in temporary render");
            return true;
        }

        trace!(
            "exiting temporary render, was static: {}",
            context.static_state
        );
        context.temporary_render_depth -= 1;
        context.static_state
    } else {
        true
    }
}

/// add [`css`](crate::css!) to the page from outside a component.
///
/// this is helpful for adding styling to functions that just return [`Markup`](crate::Markup).
/// you have to call this from within a page render or the css will not be added to the page.
///
/// ### usage
/// ```rust
/// use fishnet::{style, html, Markup, css};
///
/// async fn render_something() -> Markup {
///     style!("my_class", css!{
///         color: red;
///     });
///     
///     html!{
///         div class="my_class" {
///             "hello world!"
///         }
///     }
/// }
#[macro_export]
macro_rules! style {
    ($tl_class: literal, $css:expr) => {{
        $crate::global_store()
            .add($crate::const_nanoid!(10), || $crate::GlobalStoreEntry {
                scripts: Vec::new(),
                style: Some($css.render($tl_class)),
            })
            .await;
    }};
}

/// add js to the page from outside a component.
///
/// this is helpful for adding styling to functions that just return [`Markup`](crate::Markup).
/// you have to call this from within a page render or the js will not be added to the page.
///
/// ### usage
/// ```rust
/// use fishnet::{script, html, Markup};
/// async fn render_something() -> Markup {
///     script!(r#"
///         console.log("hello from javascript!");
///     "#);
///     
///     html!{
///         div class="my_class" {
///             "hello from html!"
///         }
///     }
/// }
#[macro_export]
macro_rules! script {
    ($js:literal) => {{
        $crate::global_store()
            .add($crate::const_nanoid!(10), || $crate::GlobalStoreEntry {
                scripts: vec![$crate::js::ScriptType::Inline($js)],
                style: None,
            })
            .await;
    }};
}

/// add components to the page.
///
/// This is done by wrapping the component in a `c!` macro. The component will then be
/// automatically built and rendered when needed.
/// ```rust
/// use fishnet::{
///     Page,
///     component::prelude::*
/// };
///
/// Page::new("example").with_body(|| async {
///     c!(component!(MyAwesomeComponent).render(|_| async {
///             html!{ "Hello World!" }
///      }.boxed()))
/// }.boxed());
/// ```
///
/// # calling from outside a page render
/// you have to call this from within a page render or it will not work.
/// if you are in debug mode it will render out an error message containing the associated context id.
/// in release mode it will just render to nothing.
#[macro_export]
macro_rules! c {
    ($component:expr) => {{
        let component = || $component;

        $crate::render_component($crate::const_nanoid!(10), component).await
    }};
}
