mod api;
mod database;
mod discord;
mod env;
mod middleware;
mod models;
mod tools;
mod twitch;

use database::Database;
use env::Environment;
use eyre::Context;
use middleware::{auth_middleware, rate_limit_middleware, RateLimiter};
use tools::install_tools;
use twitch_oauth2::Scope;

use std::{net::SocketAddr, process::exit, sync::Arc, time::Duration};

use axum::{
    error_handling::HandleErrorLayer,
    http::{header, HeaderValue, Method, StatusCode},
    middleware as axum_middleware,
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
    pub database: Arc<Database>,
}

#[derive(Debug)]
pub enum Error {
    NotFound(String),
    AppError(anyhow::Error),
}

#[tokio::main]
async fn main() -> Result<(), eyre::Report> {
    let env = Environment::new();
    install_tools(&env).expect("Failed to install tools");

    tracing::info!("App starting with:\n{:#?}", env);

    let client: HelixClient<reqwest::Client> = HelixClient::default();
    let token = twitch_oauth2::AppAccessToken::get_app_access_token(
        &client,
        twitch_oauth2::ClientId::new(env.twitch_client_id.clone()),
        twitch_oauth2::ClientSecret::new(env.twitch_client_secret.secret_str().to_owned()),
        vec![
            Scope::ModeratorReadFollowers,
            Scope::ChannelReadSubscriptions,
            Scope::BitsRead,
        ],
    )
    .await?;
    tracing::debug!("Token: {:?}", token);

    let token = Arc::new(tokio::sync::RwLock::new(token));

    let retainer = Arc::new(retainer::Cache::<String, String>::new());
    let ret = retainer.clone();
    let retainer_cleanup = tokio::spawn(async move {
        ret.monitor(10, 0.50, tokio::time::Duration::from_secs(86400 / 2))
            .await;
        Ok::<(), eyre::Report>(())
    });

    let db = Database::new(&env).await.unwrap();

    let app_state = AppState {
        env: Arc::new(env.clone()),
        token: token.clone(),
        client: client.clone(),
        retainer: retainer.clone(),
        database: Arc::new(db),
    };

    // Create rate limiter with 60 requests per minute
    let rate_limiter = Arc::new(RateLimiter::new(60, 60));

    // Start a background task to clean up the rate limiter
    let rate_limiter_clone = rate_limiter.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            rate_limiter_clone.cleanup();
        }
    });

    let cors = CorsLayer::new()
        // .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::POST, Method::OPTIONS, Method::GET])
        .allow_headers(vec![
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::AUTHORIZATION,
        ]);

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

    // Create an API router with authentication and rate limiting
    let api_routes = Router::new()
        .route("/user/latest_follow", get(api::latest_follow))
        .route("/user/latest_subscriber", get(api::latest_subscriber))
        .route("/user/latest_subgift", get(api::latest_subgift))
        .route("/user/latest_bits", get(api::latest_bits))
        .layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware,
        ))
        .layer(axum_middleware::from_fn_with_state(
            rate_limiter,
            rate_limit_middleware,
        ))
        .with_state(app_state.clone());

    // build our application with a route
    let app = Router::new()
        // install app
        .route("/twitch/oauth/authorize", get(twitch::oauth::authorize))
        .route("/twitch/oauth/callback", get(twitch::oauth::callback))
        // eventsub
        .route("/twitch/eventsub", post(twitch::eventsub::eventsub))
        // api
        .nest("/api", api_routes)
        //misc
        .route("/health", get(health))
        .route("/*catchall", get(not_found))
        .layer(cors)
        .layer(error_handler)
        .layer(tower_extensions)
        .with_state(app_state.clone());

    let event_cache = Arc::new(retainer::Cache::<axum::http::HeaderValue, ()>::new());
    let secret = event_cache.clone();
    let ec_monitor = tokio::spawn(async move {
        secret
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
        flatten(retainer_cleanup),
    )?;

    Ok(())
}

// async fn followers_monitor(
//     user_id: String,
//     token: twitch_oauth2::AppAccessToken,
//     client: HelixClient<'static, reqwest::Client>,
// ) {
//     tracing::info!("Monitoring followers for user: {}", user_id);
//     let request = get_channel_followers::GetChannelFollowersRequest::broadcaster_id(&user_id);
//     let followers: Vec<get_channel_followers::Follower> =
//         client.req_get(request, &token).await.unwrap().data;
//
//     tracing::debug!("Followers: {:#?}", followers);
//     tokio::time::sleep(Duration::from_secs(86400)).await;
// }

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
