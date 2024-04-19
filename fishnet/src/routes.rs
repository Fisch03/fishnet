use axum::{
    extract::{Path, Request},
    http::StatusCode,
    response::IntoResponse,
    routing::any,
    Extension, Router,
};
use tower_service::Service;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ComponentRoute {
    full: Arc<String>,
    component: Arc<String>,
}
impl ComponentRoute {
    pub fn new(base_route: &str, name: &str, id: &str) -> Self {
        let component = format!("{}_{}", name, id);
        let full = format!("{}/{}", base_route, component);

        Self {
            full: Arc::new(full),
            component: Arc::new(component),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.full
    }

    pub fn to_string(&self) -> String {
        self.full.to_string()
    }

    pub fn component_only_string(&self) -> String {
        self.component.to_string()
    }
}

#[derive(Debug)]
struct APIRouterInner {
    base_route: String,
    routes: HashMap<String, Router>,
}
#[derive(Debug, Clone)]
pub struct APIRouter(Arc<Mutex<APIRouterInner>>);

impl APIRouter {
    pub fn new(base_route: &str) -> Self {
        Self(Arc::new(Mutex::new(APIRouterInner {
            base_route: base_route.to_string(),
            routes: HashMap::new(),
        })))
    }

    pub async fn add_component(
        &mut self,
        component_route: ComponentRoute,
        component_router: Router<()>,
    ) {
        let mut inner = self.0.lock().await;

        inner
            .routes
            .insert(component_route.component_only_string(), component_router);
    }

    async fn get(
        Extension(router): Extension<APIRouter>,
        Path(mut component_route): Path<String>,
        mut req: Request,
    ) -> impl IntoResponse {
        let mut inner = router.0.lock().await;
        if let Some((c, _)) = component_route.split_once('/') {
            component_route = c.to_string();
        }
        let full_route = format!("{}/{}", inner.base_route, component_route);

        if let Some(router) = inner.routes.get_mut(&component_route) {
            // Strip the component route from the request path.
            // TODO: im not that happy with this code
            let uri = format!("{}/", req.uri());
            *req.uri_mut() = uri
                .replace(&full_route, "")
                .parse()
                .expect("failed to parse uri");

            let res = router.call(req).await;
            res.unwrap_or_else(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            })
        } else {
            (StatusCode::NOT_FOUND, "API route does not exist").into_response()
        }
    }

    pub async fn make_router(&self) -> Router {
        let inner = self.0.lock().await;

        Router::new()
            .route(
                &format!("{}/*component_route", inner.base_route),
                any(Self::get),
            )
            .layer(Extension(self.clone()))
    }
}
