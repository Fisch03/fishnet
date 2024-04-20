use super::ComponentState;
use async_trait::async_trait;
use futures::future::BoxFuture;
use maud::Markup;

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
    pub fn new(renderer: ContentRenderer<ST>, state: ComponentState<ST>) -> Box<Self> {
        Box::new(Self { renderer, state })
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
    Dynamic(Box<dyn StatefulRenderer>),
    Static(Markup),
}
impl ContentType {
    pub async fn render(&self) -> Markup {
        match self {
            ContentType::Dynamic(renderer) => renderer.render().await,
            ContentType::Static(content) => content.clone(),
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
