//! Real HTTP client for GPUI image assets.
//!
//! `gpui_platform::application()` installs a null HTTP client, so remote tile
//! URLs would never leave the process. This client runs reqwest on a Tokio
//! runtime and bridges its responses into GPUI's `AsyncBody`.

use std::sync::Arc;

use futures_util::future::BoxFuture;
use reqwest::Client;
use siegu_ui::gpui::http_client::{
    anyhow, http::HeaderValue, http::Request, http::Response, http::StatusCode, AsyncBody,
    HttpClient, Inner, Result, Url,
};
use tokio::{runtime::Runtime, sync::oneshot};

pub struct TileHttpClient {
    client: Client,
    runtime: Arc<Runtime>,
    user_agent: HeaderValue,
}

impl TileHttpClient {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            // OSM serves an "Access blocked" PNG to browser-like or bare
            // User-Agents. Identify Siegu so tile requests follow the policy.
            .user_agent("Siegu/0.1.0 (+https://github.com/denzyldick/siegu)")
            .build()
            .expect("tile HTTP client");
        Self {
            client,
            runtime,
            user_agent: HeaderValue::from_static(concat!(
                "Siegu/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/denzyldick/siegu)"
            )),
        }
    }
}

impl HttpClient for TileHttpClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        Some(&self.user_agent)
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(&self, req: Request<AsyncBody>) -> BoxFuture<'static, Result<Response<AsyncBody>>> {
        let method = req.method().clone();
        let Ok(url) = req.uri().to_string().parse::<Url>() else {
            return Box::pin(async { Err(anyhow!("invalid tile URL")) });
        };
        let mut builder = self.client.request(method, url);
        for (name, value) in req.headers() {
            builder = builder.header(name.as_str(), value.as_bytes());
        }
        if let Inner::Bytes(cursor) = req.into_body().0 {
            builder = builder.body(cursor.into_inner().to_vec());
        }

        let (tx, rx) = oneshot::channel();
        self.runtime.spawn(async move {
            let result = builder.send().await;
            let _ = tx.send(result);
        });

        Box::pin(async move {
            let response = rx
                .await
                .map_err(|error| anyhow!("tile request cancelled: {error}"))??;
            let status = StatusCode::from_u16(response.status().as_u16())?;
            let mut out = Response::builder().status(status);
            for (name, value) in response.headers() {
                out = out.header(name.as_str(), value.as_bytes());
            }
            let bytes = response.bytes().await?;
            Ok(out.body(AsyncBody::from(bytes.to_vec()))?)
        })
    }
}
