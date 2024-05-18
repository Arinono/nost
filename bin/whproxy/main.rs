mod discord;
mod env;
mod eventsub;
mod tools;
mod twitch;
mod twitch_oauth;

use env::Environment;
use eyre::Context;
use tools::install_tools;

use std::{net::SocketAddr, process::exit, sync::Arc, time::Duration};

use axum::{
    error_handling::HandleErrorLayer,
    http::{header, HeaderValue, Method, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Extension, Router,
};
use tokio::{signal, task::JoinHandle};
use tower::{BoxError, ServiceBuilder};
use tower_http::{catch_panic::CatchPanicLayer, cors::CorsLayer, trace::TraceLayer};
use twitch_api::helix::HelixClient;

const PORT: u16 = 3000;

#[derive(Clone)]
pub struct AppState {
    pub env: Arc<Environment>,
    pub token: Arc<tokio::sync::RwLock<twitch_oauth2::AppAccessToken>>,
    pub client: HelixClient<'static, reqwest::Client>,
    pub retainer: Arc<retainer::Cache<String, String>>,
}

#[derive(Debug)]
pub enum Error {
    NotFound(String),
    AppError(anyhow::Error),
}

#[tokio::main]
async fn main() -> Result<(), eyre::Report> {
    install_tools().expect("Failed to install tools");

    let env = Environment::new();

    tracing::debug!("App starting with:\n{:#?}", env);

    let client: HelixClient<reqwest::Client> = HelixClient::default();
    let token = twitch_oauth2::AppAccessToken::get_app_access_token(
        &client,
        twitch_oauth2::ClientId::new(env.twitch_client_id.clone()),
        twitch_oauth2::ClientSecret::new(env.twitch_client_secret.secret_str().to_owned()),
        vec![],
    )
    .await?;
    tracing::debug!("Token: {:#?}", token);

    let token = Arc::new(tokio::sync::RwLock::new(token));

    let retainer = Arc::new(retainer::Cache::<String, String>::new());
    let ret = retainer.clone();
    let retainer_cleanup = tokio::spawn(async move {
        ret.monitor(10, 0.50, tokio::time::Duration::from_secs(86400 / 2))
            .await;
        Ok::<(), eyre::Report>(())
    });

    let app_state = AppState {
        env: Arc::new(env.clone()),
        token: token.clone(),
        client: client.clone(),
        retainer: retainer.clone(),
    };

    let cors = CorsLayer::new()
        // .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::POST, Method::OPTIONS])
        .allow_headers(vec![header::CONTENT_TYPE, header::ACCEPT]);

    let error_handler = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|error: BoxError| async move {
            if error.is::<tower::timeout::error::Elapsed>() {
                Ok(StatusCode::REQUEST_TIMEOUT)
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unhandled internal error: {error}"),
                ))
            }
        }))
        .timeout(Duration::from_secs(10))
        .layer(TraceLayer::new_for_http())
        .into_inner();

    let tower_extensions = ServiceBuilder::new()
        .layer(Extension(client.clone()))
        .layer(Extension(Arc::new(env.clone())))
        .layer(
            TraceLayer::new_for_http().on_failure(|error, _latency, _span: &tracing::Span| {
                tracing::error!(error=%error);
            }),
        )
        .layer(CatchPanicLayer::new());

    // build our application with a route
    let app = Router::new()
        .route("/twitch/eventsub", post(eventsub::eventsub))
        .route("/health", get(health))
        .route("/twitch/oauth/authorize", get(twitch_oauth::authorize))
        .route("/twitch/oauth/callback", get(twitch_oauth::callback))
        .route("/*catchall", get(not_found))
        .layer(cors)
        .layer(error_handler)
        .layer(tower_extensions)
        .with_state(app_state.clone());

    let event_cache = Arc::new(retainer::Cache::<axum::http::HeaderValue, ()>::new());
    let ecret = event_cache.clone();
    let ec_monitor = tokio::spawn(async move {
        ecret
            .monitor(10, 0.50, tokio::time::Duration::from_secs(86400 / 2))
            .await;
        Ok::<(), eyre::Report>(())
    });

    let addr: SocketAddr = format!("[::]:{}", PORT).parse().unwrap();
    tracing::info!("Listening on: {addr}");
    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| eyre::eyre!("Server error: {:#}", e))
    });

    tokio::try_join!(
        flatten(ec_monitor),
        flatten(server),
        flatten(tokio::spawn(twitch::eventsub_register(
            app_state,
            client.clone(),
            token.clone()
        ))),
        flatten(retainer_cleanup)
    )?;

    Ok(())
}

async fn flatten<T>(handle: JoinHandle<Result<T, eyre::Report>>) -> Result<T, eyre::Report> {
    match handle.await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(err),
        Err(e) => Err(e).wrap_err_with(|| "handling failed"),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C signal"),
        _ = terminate => tracing::info!("Received SIGTERM signal"),
    }
    tracing::info!("Shutting down forcefully...");
    exit(0);
}

pub async fn health() -> &'static str {
    "Healthy!"
}

async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Html("Not Found".to_string()))
}

impl<E> From<E> for Error
where
    E: Into<anyhow::Error>,
{
    fn from(e: E) -> Self {
        Self::AppError(e.into())
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::NotFound(_) => {
                (StatusCode::NOT_FOUND, Html("Not found".to_string())).into_response()
            }
            Self::AppError(e) => {
                tracing::error!("Application error: {:#}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())).into_response()
            }
        }
    }
}
