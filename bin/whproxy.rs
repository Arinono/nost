use std::{net::SocketAddr, time::Duration};

use axum::{
    error_handling::HandleErrorLayer,
    http::{header, HeaderValue, Method, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use tokio::signal;
use tower::{BoxError, ServiceBuilder};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const PORT: u16 = 3000;

#[derive(Debug, Clone)]
struct AppState {}

#[derive(Debug)]
pub enum Error {
    NotFound(String),
    AppError(anyhow::Error),
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nost=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app_state = AppState {};

    let cors = CorsLayer::new()
        .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
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

    // build our application with a route
    let app = Router::new()
        .route("/eventsub", post(eventsub))
        .route("/health", get(health))
        .route("/*catchall", get(not_found))
        .layer(cors)
        .layer(error_handler)
        .with_state(app_state);

    let addr: SocketAddr = format!("[::]:{}", PORT).parse().unwrap();
    tracing::debug!("Listening on:\n{addr}");
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn eventsub() -> impl IntoResponse {
    (StatusCode::OK, Html("EventSub".to_string()))
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
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    println!("Shutting down gracefully...");
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
