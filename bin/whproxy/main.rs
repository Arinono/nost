mod env;
// mod eventsub;
mod tools;

use env::Environment;
use tools::install_tools;

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
use twitch_api::helix::{self, HelixClient};

const PORT: u16 = 3000;

#[derive(Clone)]
pub struct AppState {
    pub env: Environment,
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

    tracing::debug!(
        "Channel: {:?}",
        client.get_channel_from_login("nyrionset", &token).await?
    );

    let app_state = AppState { env };

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
        // .route("/eventsub", post(eventsub::eventsub))
        .route("/health", get(health))
        .route("/*catchall", get(not_found))
        .layer(cors)
        .layer(error_handler)
        .with_state(app_state);

    let addr: SocketAddr = format!("[::]:{}", PORT).parse().unwrap();
    tracing::info!("Listening on: {addr}");
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    Ok(())
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
    tracing::info!("Shutting down gracefully...");
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
