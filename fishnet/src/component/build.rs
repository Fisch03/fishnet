use async_trait::async_trait;
use axum::{Extension, Router};
use futures::future::{BoxFuture, FutureExt};
use maud::{html, Markup};
use std::sync::Arc;
use tracing::{debug, instrument, trace};

use super::{
    render::{ContentType, StatefulContentRenderer},
    Component, ComponentRoute, ComponentState, HasRenderer,
};
use crate::css;
use crate::page::render_context;

#[derive(Debug, Clone)]
pub struct BuiltComponent {
    #[allow(dead_code)]
    name: Arc<str>,
    id: Arc<str>,

    class_name: Arc<str>,

    content: Arc<ContentType>,
}

pub struct ComponentBuildResult {
    pub built_component: BuiltComponent,

    pub runner: Option<BoxFuture<'static, ()>>,
    pub router: Option<(ComponentRoute, Router)>,
}

impl BuiltComponent {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub async fn render(&self) -> Markup {
        html! {
            div class=(self.class_name) { (self.content.render().await) }
        }
    }

    pub fn render_if_static(&self) -> Option<Markup> {
        self.content.render_if_static().map(|content| {
            html! {
                div class=(self.class_name) { (content) }
            }
        })
    }

    pub fn is_dynamic(&self) -> bool {
        match self.content.as_ref() {
            ContentType::Static(_) => false,
            _ => true,
        }
    }
}

#[async_trait]
pub trait BuildableComponent {
    fn name(&self) -> &str;
    fn id(&self) -> &str;

    async fn build(self: Self, base_route: &str) -> ComponentBuildResult;
}

#[async_trait]
impl<ST, S> BuildableComponent for Component<HasRenderer, S, ST>
where
    ST: Clone + Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }
    fn id(&self) -> &str {
        &self.id
    }

    #[instrument(name = "build_component", skip_all, fields(name = %self.name))]
    async fn build(self, base_route: &str) -> ComponentBuildResult {
        trace!("building state");
        let api_route = ComponentRoute::new(base_route, &self.name, &self.id);
        let state = ComponentState {
            api_route: api_route.clone(),
            state: self.state,
        };

        let router = self.router.map(|r| r.layer(Extension(state.clone())));

        let renderer = self.renderer.unwrap();

        let runner = self.runner.map(|runner| {
            let runner = (runner)(state.clone());
            runner.boxed()
        });

        let content;

        if !self.is_dynamic {
            trace!("pre-rendering static component");
            render_context::enter_temporary_render().await;
            let render = renderer(state.clone()).await;
            if !render_context::exit_temporary_render().await {
                debug!("detected dynamic child, making self dynamic");
                content = ContentType::Dynamic(StatefulContentRenderer::new(renderer, state));
            } else {
                content = ContentType::Static(Arc::new(render));
            }
        } else {
            content = ContentType::Dynamic(StatefulContentRenderer::new(renderer, state));
        }

        trace!("rendering component style");

        let class_name = css::pascal_to_kebab(&self.name);
        let style = self.style.map(|style| style.render(&class_name));

        render_context::global_store()
            .add(&self.id, || render_context::GlobalStoreEntry {
                scripts: self.scripts,
                style,
            })
            .await;

        debug!("built component");
        ComponentBuildResult {
            built_component: BuiltComponent {
                name: self.name,
                id: self.id,
                class_name: class_name.into(),
                content: Arc::new(content),
            },
            runner,
            router: router.map(|r| (api_route, r)),
        }
    }
}

/*
#[cfg(test)]
mod tests {
    use crate::component::prelude::*;
    use crate::component::render::ContentType;

    #[tokio::test]
    async fn test_build_minimal() {
        let result = component!(TestComponent)
            .render(|_| {
                async {
                    html! { "test" }
                }
                .boxed()
            })
            .build("/")
            .await;

        assert_eq!(result.built_component.name(), "TestComponent");
        assert!(matches!(
            result.built_component.content.as_ref(),
            ContentType::Static(_)
        ));

        assert!(result.runner.is_none());
        assert!(result.router.is_none());
    }

    #[tokio::test]
    async fn test_build_minimal_dynamic() {
        let result = component!(TestComponent)
            .render_dynamic(|_| {
                async {
                    html! { "test" }
                }
                .boxed()
            })
            .build("/")
            .await;

        assert_eq!(result.built_component.name(), "TestComponent");
        assert!(matches!(
            result.built_component.content.as_ref(),
            ContentType::Dynamic(_)
        ));

        assert!(result.runner.is_none());
        assert!(result.router.is_none());
    }

    #[tokio::test]
    async fn test_build() {
        use axum::routing::get;

        let result = component!(TestComponent)
            .add_script(ScriptType::External("test.js".to_string()))
            .route("/route", get(|| async { "test" }))
            .render(|_| {
                async {
                    html! { "test" }
                }
                .boxed()
            })
            .build("/")
            .await;

        assert_eq!(result.built_component.name(), "TestComponent");
        assert!(matches!(
            result.built_component.content.as_ref(),
            ContentType::Static(_)
        ));
        assert!(result.router.is_some());

        /* it would be nice to verify the global store here
        assert_eq!(result.scripts.len(), 1);
        assert_eq!(
            result.scripts[0],
            ScriptType::External("test.js".to_string())
        );
        */
    }

    /*
    #[test]
    fn test_nested_dynamic() {
        todo!();
    }
    */
}
*/
