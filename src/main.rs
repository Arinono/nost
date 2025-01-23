mod airtable;
mod api;
mod discord;
mod env;
mod models;
mod tools;
mod twitch;

use env::Environment;
use eyre::Context;
use tools::install_tools;
use twitch_oauth2::Scope;

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
const CACHE_CLEAN: u64 = 60;

#[derive(Clone)]
pub struct AppState {
    pub env: Arc<Environment>,
    pub token: Arc<tokio::sync::RwLock<twitch_oauth2::AppAccessToken>>,
    pub client: HelixClient<'static, reqwest::Client>,
    pub retainer: Arc<retainer::Cache<String, String>>,
    pub db: Arc<libsql::Database>,
    pub user_cache: Arc<retainer::Cache<String, models::UserRecord>>,
    pub subgift_cache: Arc<retainer::Cache<String, models::SubgiftRecord>>,
    pub bits_cache: Arc<retainer::Cache<String, models::BitsRecord>>,
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

    let db = match env.dev_mode {
        true => {
            libsql::Builder::new_local(env.turso_db_url.clone())
                .build()
                .await?
        }
        false => {
            libsql::Builder::new_remote_replica(
                env.turso_local_db_path.clone(),
                env.turso_db_url.clone(),
                env.turso_auth_token.secret_str().to_string(),
            )
            .build()
            .await?
        }
    };

    let retainer = Arc::new(retainer::Cache::<String, String>::new());
    let ret = retainer.clone();
    let retainer_cleanup = tokio::spawn(async move {
        ret.monitor(10, 0.50, tokio::time::Duration::from_secs(86400 / 2))
            .await;
        Ok::<(), eyre::Report>(())
    });

    let user_cache = Arc::new(retainer::Cache::<String, models::UserRecord>::new());
    let user_cache_ret = user_cache.clone();

    let subgift_cache = Arc::new(retainer::Cache::<String, models::SubgiftRecord>::new());
    let subgift_cache_ret = subgift_cache.clone();

    let bits_cache = Arc::new(retainer::Cache::<String, models::BitsRecord>::new());
    let bits_cache_ret = bits_cache.clone();

    let airtable_cleanup = tokio::spawn(async move {
        user_cache_ret
            .monitor(10, 0.50, tokio::time::Duration::from_secs(CACHE_CLEAN))
            .await;
        subgift_cache_ret
            .monitor(10, 0.50, tokio::time::Duration::from_secs(CACHE_CLEAN))
            .await;
        bits_cache_ret
            .monitor(10, 0.50, tokio::time::Duration::from_secs(CACHE_CLEAN))
            .await;
        Ok::<(), eyre::Report>(())
    });

    let app_state = AppState {
        env: Arc::new(env.clone()),
        token: token.clone(),
        client: client.clone(),
        retainer: retainer.clone(),
        db: Arc::new(db),
        user_cache: user_cache.clone(),
        subgift_cache: subgift_cache.clone(),
        bits_cache: bits_cache.clone(),
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
        // install app
        .route("/twitch/oauth/authorize", get(twitch::oauth::authorize))
        .route("/twitch/oauth/callback", get(twitch::oauth::callback))
        // eventsub
        .route("/twitch/eventsub", post(twitch::eventsub::eventsub))
        // api
        .route("/api/user/latest_follow", get(api::latest_follow))
        .route("/api/user/latest_subscriber", get(api::latest_subscriber))
        .route("/api/user/latest_subgift", get(api::latest_subgift))
        .route("/api/user/latest_bits", get(api::latest_bits))
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

    // let twitch_broadcaster_id = env.twitch_broadcaster_id.clone();
    // let client_clone = client.clone();
    // let follower_cleanup = tokio::spawn(async move {
    //     followers_monitor(twitch_user_id, twitch_token, client_clone).await;
    //
    //     Ok::<(), eyre::Report>(())
    // });

    tokio::try_join!(
        flatten(ec_monitor),
        flatten(server),
        flatten(tokio::spawn(twitch::eventsub_register(
            app_state,
            client.clone(),
            token.clone()
        ))),
        flatten(retainer_cleanup),
        flatten(airtable_cleanup),
        // flatten(follower_cleanup)
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
