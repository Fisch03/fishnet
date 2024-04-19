//! Storing and serving multiple [`Page`]s as a website.

use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tracing::{info, instrument};

use crate::page::{Page, RouterPageExt};

/// A simple website builder. A Website consists of multiple [`Page`]s and can additionally serve static files.
pub struct Website {
    router: Router,

    serve_dir: Option<String>,
    compression: bool,
}

impl Website {
    /// Create a new website.
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            serve_dir: None,
            compression: false,
        }
    }

    /// Add a page to the website.
    ///
    /// This will first initiate a build of the page and afterwards attach the page to the Website at the given path.
    pub async fn add_page(mut self, path: &str, page: Page) -> Self {
        self.router = self.router.clone().attach_page(path, page).await;

        self
    }

    /// Enable or disable compression for the website.
    pub fn compression(mut self, enable: bool) -> Self {
        self.compression = enable;
        self
    }

    /// Serve a directory as static files.
    pub fn serve_dir(mut self, path: &str) -> Self {
        self.serve_dir = Some(path.to_string());
        self
    }

    /// Start serving the website on the given port.
    ///
    /// The returned future will never resolve unless an error occurs.
    #[instrument(name = "Website::serve" skip_all, level = "debug")]
    pub async fn serve(mut self, port: u16) {
        if let Some(path) = self.serve_dir {
            let serve_dir = ServeDir::new(path).append_index_html_on_directories(true);
            self.router = self.router.fallback_service(serve_dir);
        }

        if self.compression {
            let compression = CompressionLayer::new()
                .gzip(true)
                .zstd(true)
                .br(true)
                .deflate(true);

            self.router = self.router.layer(compression);
        }

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
            .await
            .unwrap();

        info!("ready! serving page on port {}", port);
        axum::serve(listener, self.router).await.unwrap();
    }
}
