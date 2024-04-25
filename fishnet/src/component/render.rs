use super::ComponentState;
use async_trait::async_trait;
use futures::future::BoxFuture;
use maud::Markup;
use std::sync::Arc;

pub type ContentRenderer<ST> =
    Box<dyn Fn(ComponentState<ST>) -> BoxFuture<'static, Markup> + Send + Sync>;

pub struct StatefulContentRenderer<ST>
where
    ST: Clone + Send + Sync,
{
    renderer: ContentRenderer<ST>,
    state: ComponentState<ST>,
}
impl<ST> StatefulContentRenderer<ST>
where
    ST: Clone + Send + Sync,
{
    pub fn new(renderer: ContentRenderer<ST>, state: ComponentState<ST>) -> Arc<Self> {
        Arc::new(Self { renderer, state })
    }
}

#[async_trait]
pub trait StatefulRenderer: Send + Sync {
    async fn render(&self) -> Markup;
}

#[async_trait]
impl<ST> StatefulRenderer for StatefulContentRenderer<ST>
where
    ST: Clone + Send + Sync,
{
    async fn render(&self) -> Markup {
        (self.renderer)(self.state.clone()).await
    }
}

pub enum ContentType {
    Dynamic(Arc<dyn StatefulRenderer>),
    Static(Arc<Markup>),
}
impl ContentType {
    pub async fn render(&self) -> Markup {
        match self {
            ContentType::Dynamic(renderer) => renderer.render().await,
            ContentType::Static(content) => content.as_ref().clone(),
        }
    }

    #[inline]
    pub fn render_if_static(&self) -> Option<Markup> {
        match self {
            ContentType::Static(content) => Some(content.as_ref().clone()),
            _ => None,
        }
    }
}
impl std::fmt::Debug for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentType::Dynamic(_) => write!(f, "Dynamic"),
            ContentType::Static(_) => write!(f, "Static"),
        }
    }
}
