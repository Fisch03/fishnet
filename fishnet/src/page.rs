//! A visitable page on the [`Website`](crate::website::Website).

use async_trait::async_trait;
use axum::{http::header, response::IntoResponse, routing::get, Extension, Router};
use futures::future::{BoxFuture, FutureExt};
use maud::{html, Markup, DOCTYPE};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, debug_span, instrument, Instrument};

use crate::css::Stylesheet;
use crate::js::{self, ScriptType};
use crate::routes::APIRouter;

pub mod render_context;
use render_context::ComponentStore;

pub struct BuiltPage {
    #[allow(dead_code)]
    name: String,

    head: Markup,
    body_renderer: Box<dyn Fn() -> BoxFuture<'static, Markup> + Send + Sync>,

    pub used_globals: HashSet<String>,
    pub components: Arc<Mutex<ComponentStore>>,

    pub api_path: String,
    api_router: APIRouter,

    script_path: String,
    bundled_script: String,

    style_path: String,
    stylesheet: Stylesheet,

    // tasks that need to be awaited before serving content
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl BuiltPage {
    #[instrument(name = "Page::build", skip_all, fields(name = %page.name))]
    async fn new(page: Page, path: &str) -> Router {
        let base_path = path.trim_end_matches('/');
        let script_path = format!("{}/script.js", base_path);
        let style_path = format!("{}/style.css", base_path);

        let api_path = format!("{}/api", base_path);

        let mut bundled_script = String::new();
        for script in &page.extra_scripts {
            let script: js::ScriptString = script.into();

            #[cfg(feature = "minify-js")]
            let script = &js::minify_script(script).await;

            bundled_script.push_str(script.as_str());
        }

        let built_page = Self {
            name: page.name,

            head: page.head,
            body_renderer: page.body_renderer,

            used_globals: HashSet::new(),
            components: Arc::new(Mutex::new(ComponentStore::new())),

            api_path,
            api_router: APIRouter::new(&format!("{}/api", base_path)),

            script_path,
            bundled_script,

            style_path,
            stylesheet: Stylesheet::new(),

            tasks: Vec::new(),
        };

        let api_router = built_page.api_router.make_router().await;
        let page_extension = Extension(Arc::new(Mutex::new(built_page)));

        // pre-render the page to save request time. this is obviously not guaranteed to prerender all the components, but it should get most of them.
        debug!("performing page pre-render");
        let _ = Self::render(page_extension.clone()).await;

        debug!("building router");
        Router::new()
            .route("/", get(BuiltPage::render))
            .route("/script.js", get(BuiltPage::script))
            .route("/style.css", get(BuiltPage::style))
            .merge(api_router)
            .layer(page_extension)
    }

    async fn render(page: Extension<Arc<Mutex<Self>>>) -> Markup {
        let start = std::time::Instant::now();

        let mut page_guard = page.lock().await;

        render_context::enter_page(&mut page_guard).await;
        let render = (page_guard.body_renderer)().await;
        let mut result = render_context::exit_page().await;

        //dbg!(&page.components.lock().unwrap());

        drop(page_guard);

        let mut tasks = Vec::new();
        let span = debug_span!("Page::task");
        for id in result.new_components.drain() {
            let page = page.clone();
            tasks.push(tokio::spawn(
                async move {
                    if page.lock().await.used_globals.contains(&id) {
                        return;
                    }
                    if let Some(component_globals) = render_context::global_store().get(id).await {
                        if let Some(style) = &component_globals.style {
                            page.lock().await.stylesheet.add(style);
                        }

                        for script in &component_globals.scripts {
                            let script: js::ScriptString = script.into();

                            #[cfg(feature = "minify-js")]
                            let script = &js::minify_script(script).await;

                            page.lock().await.bundled_script.push_str(script.as_str());
                        }
                    }
                }
                .instrument(span.clone()),
            ))
        }

        let mut page = page.lock().await;
        page.tasks.append(&mut tasks);

        for runner in result.runners {
            tokio::spawn(runner);
        }

        for (route, router) in result.routers.drain(..) {
            page.api_router.add_component(route, router).await;
        }

        let full_render = html! {
                (DOCTYPE)
                html lang="en" {
                    head {
                        (page.head)
                        link rel="stylesheet" href=(page.style_path) {}
                    }
                    (render)
                    script src=(page.script_path) {}
                }
        };

        debug!("page render took {:?}", start.elapsed());

        full_render
    }

    async fn wait_for_tasks(self: &mut BuiltPage) {
        let len = self.tasks.len();
        if len == 0 {
            return;
        }

        debug!("waiting for {:?} tasks to finish", len);
        futures::future::join_all(self.tasks.drain(..)).await;
    }

    // Endpoint for serving the bundled script.
    async fn script(page: Extension<Arc<Mutex<Self>>>) -> impl IntoResponse {
        let mut page = page.lock().await;
        page.wait_for_tasks().await;

        (
            [(header::CONTENT_TYPE, "application/javascript")],
            page.bundled_script.clone(),
        )
    }

    // Endpoint for serving the stylesheet.
    async fn style(page: Extension<Arc<Mutex<Self>>>) -> impl IntoResponse {
        let mut page = page.lock().await;
        page.wait_for_tasks().await;

        (
            [(header::CONTENT_TYPE, "text/css")],
            page.stylesheet.render(),
        )
    }
}

/// A page represents a visitable route on the website.
///
/// It manages rendering of the content, preparing [scripts](ScriptType) and running components.
pub struct Page {
    name: String,

    head: Markup,
    body_renderer: Box<dyn Fn() -> BoxFuture<'static, Markup> + Send + Sync>,

    extra_scripts: HashSet<ScriptType>,
}

impl Page {
    /// Create a new page.
    ///
    /// The name is only used for logging purposes.
    pub fn new(name: &str) -> Self {
        let mut extra_scripts = HashSet::new();
        extra_scripts.insert(ScriptType::Inline(include_str!("../htmx/dist/htmx.js")));

        Self {
            name: name.into(),

            head: html! {},
            body_renderer: Box::new(|| {
                async {
                    html! {}
                }
                .boxed()
            }),

            extra_scripts,
        }
    }

    pub fn with_head(mut self, head: Markup) -> Self {
        self.head = head;
        self
    }

    /// Add content to the page.
    ///
    /// This function takes in a closure that returns a rendered page.
    pub fn with_body<C>(mut self, content_renderer: C) -> Self
    where
        C: Fn() -> BoxFuture<'static, Markup> + Send + Sync + 'static,
    {
        self.body_renderer = Box::new(content_renderer);
        self
    }
}

/// Allows attaching a page to a router.
#[async_trait]
pub trait RouterPageExt {
    /// Attach the given page to the router. This involves building the page and adding multiple routes for the api, scripts and content.
    async fn attach_page(self, path: &str, page: Page) -> Self;
}

#[async_trait]
impl RouterPageExt for Router {
    async fn attach_page(self, path: &str, page: Page) -> Self {
        BuiltPage::new(page, path).await
    }
}
