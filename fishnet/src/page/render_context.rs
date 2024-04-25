//! managing resources during a render
//!
//! the render context is used under the hood whenever you use macros that add things to the
//! page (e.g. [`c!`](crate::c!), [`style!`](crate::style!), [`script!`](crate::script!), ...).
//!
//! before a page renders its contents, it attaches itself to the render context via [`enter_page`]. during the
//! render when a resource is about to be added, it is first checked whether it already exists on
//! the render context. if yes, it is not added again and just reused. otherwise it is newly
//! constructed. after the render is finished, the page can use [`exit_page`] to get a list of all
//! the newly constructed things during the render and then process them further (e.g. add routes
//! from new components, minify added scripts, ...)
//!
//! you usually don't need to call anything from in here manually unless you want to have finer
//! control over resources (like dynamically adding resources to the page)

use axum::routing::Router;
use futures::future::BoxFuture;
use maud::{html, Markup};
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, OwnedMutexGuard};
use tracing::{error, instrument, trace, warn};

use crate::component::{BuildableComponent, BuiltComponent};
use crate::page::BuiltPage;
use crate::routes::ComponentRoute;
use crate::{css, js};

fn render_context() -> &'static Mutex<Option<RenderContext>> {
    static RENDER_CONTEXT: OnceLock<Mutex<Option<RenderContext>>> = OnceLock::new();
    RENDER_CONTEXT.get_or_init(|| Mutex::new(None))
}

/// acquire access to the [`GlobalStore`].
pub fn global_store() -> &'static GlobalStore {
    static GLOBAL_STORE: OnceLock<GlobalStore> = OnceLock::new();
    GLOBAL_STORE.get_or_init(|| GlobalStore::new())
}

#[derive(Debug)]
#[doc(hidden)]
pub struct ComponentStore(pub HashMap<String, BuiltComponent>);

impl ComponentStore {
    pub(crate) fn new() -> Self {
        Self(HashMap::new())
    }
}

/// a single global (page independent) resource
#[derive(Debug)]
pub struct GlobalStoreEntry {
    pub scripts: Vec<js::ScriptType>,
    pub style: Option<css::RenderedStyle>,
}

/// collection of resources that are page independent
#[derive(Debug)]
pub struct GlobalStore(Mutex<HashMap<String, Arc<GlobalStoreEntry>>>);
impl GlobalStore {
    pub(crate) fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    /// add a [`GlobalStoreEntry`] to the store
    ///
    /// does nothing if there already is a entry under the given id.
    /// otherwise the `globals` closure is executed to create a new entry
    ///
    /// this will also notify the currently active render context about the newly added resource
    /// (which usually means that it gets added to the rendered page)
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

    /// get a [`GlobalStoreEntry`] from its id.
    pub async fn get<'a>(&'a self, id: &str) -> Option<Arc<GlobalStoreEntry>> {
        self.0.lock().await.get(id).cloned()
    }

    #[doc(hidden)]
    pub async fn clear(&self) {
        self.0.lock().await.clear();
    }
}

pub(crate) struct RenderContext {
    base_route: String,

    components: OwnedMutexGuard<ComponentStore>,

    new_globals: HashSet<String>,

    static_state: bool,
    temporary_render_depth: usize,

    new_runners: Vec<BoxFuture<'static, ()>>,
    new_routers: Vec<(ComponentRoute, Router)>,
}
impl RenderContext {
    async fn new(base_route: &str, components: Arc<Mutex<ComponentStore>>) -> RenderContext {
        Self {
            base_route: base_route.to_string(),

            components: components.lock_owned().await,

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
/// * `routers` - A list of routers that should be accessible from the page at the given routes
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
pub async fn enter_page(page: &mut BuiltPage) {
    let mut context = render_context().lock().await;

    if context.is_some() {
        warn!("tried to render a page while another page is already being rendered");
    }

    context.replace(RenderContext::new(&page.api_path, page.components.clone()).await);
}

/// Exit a page render context.
///
/// This should be called after rendering all components. It will return the `RenderResult` containing all the scripts and runners that were collected during the rendering.
/// # Panics
/// Panics if no page is currently being rendered. (i.e. `enter_page` was not called before)
pub async fn exit_page() -> RenderResult {
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

    let is_temporary = context.temporary_render_depth > 0;

    let render;
    let existing_component = context.components.0.get(&context_id.to_string());
    if existing_component.is_some() {
        let content = existing_component.unwrap().content_cloned();

        drop(context_guard);

        // IMPORTANT: Since may lead to recursive calls, all the locks need to be dropped before calling
        if is_temporary {
            render = content.render_if_static().unwrap_or_default();
        } else {
            render = content.render().await;
        }

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

        drop(context_guard);

        // IMPORTANT: Since may lead to recursive calls, all the locks need to be dropped before calling
        trace!("building component");
        let new_component = lazy_component().build(&base_route).await;
        trace!("rendering component");
        if is_temporary {
            render = new_component
                .built_component
                .render_if_static()
                .unwrap_or_default();
        } else {
            render = new_component.built_component.render().await;
        }

        context_guard = render_context().lock().await;
        if context_guard.is_none() {
            error!(
                context_id,
                "page render exited while a component was still being rendered"
            );
            return html! { "rendering failed for context " (context_id) ": page render exited" };
        }

        context = context_guard.as_mut().unwrap();

        context.static_state &= !new_component.built_component.is_dynamic();

        if let Some(router) = new_component.router {
            context.new_routers.push(router)
        }
        if let Some(runner) = new_component.runner {
            context.new_runners.push(runner);
        }

        if !context.temporary_render_depth > 0 {
            context
                .components
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
/// Temporary render contexts can be exited using [`exit_temporary_render`], and can be multiple
/// levels deep. it is up to you to ensure that you always exit every render you enter.
///
/// You usually don't need to call this function yourself.
pub async fn enter_temporary_render() {
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
/// Returns true if the temporary render started via [`enter_temporary_render`] was static (i.e. no dynamic components were rendered).
/// If not within a (temporary) render context, this function will always return true.
///
/// You usually don't need to call this function yourself.
pub async fn exit_temporary_render() -> bool {
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

/// add [`css`](crate::css!) to the page
///
/// you have to call this from within a page render or it will not work.
///
/// there are two ways to use this macro:
/// ## from within a component
/// ```rust
/// use fishnet::component::prelude::*;
///
/// #[component]
/// async fn my_component() {
///     style!(css!{
///         color: red;
///     });
///
///     html! {
///         "hello world!"
///     }
/// }
///```
///
/// ## from outside a component
/// additionally to providing the style, you also have to name the top level class yourself as the
/// first argument.
///
/// this is helpful for adding styling to functions that just return [`Markup`](crate::Markup).
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
/// ```
#[macro_export]
macro_rules! style {
    ($css: expr) => {{
        compile_error!(
            "you have to provide a class name to the style macro or use it from within a component"
        );
    }};
    ($tl_class: literal, $css:expr) => {{
        $crate::page::render_context::global_store()
            .add($crate::const_nanoid!(10), || {
                $crate::page::render_context::GlobalStoreEntry {
                    scripts: Vec::new(),
                    style: Some($css.render($tl_class)),
                }
            })
            .await;
    }};
}

/// add js to the page
///
/// differently to the [`css!`] macro, this will work identically whether you are within a
/// component or not. you still have to be within a page render though or it will do nothing.
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
        $crate::page::render_context::global_store()
            .add($crate::const_nanoid!(10), || {
                $crate::page::render_context::GlobalStoreEntry {
                    scripts: vec![$crate::js::ScriptType::Inline($js)],
                    style: None,
                }
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
/// #[component]
/// async fn my_awesome_component() {
///     html!{
///         "Hello World"
///     }
/// }
///
/// Page::new("example").with_body(|| async {
///     c!(my_awesome_component())
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

        $crate::page::render_context::render_component($crate::const_nanoid!(10), component).await
    }};
}
