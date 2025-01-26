use crate::env::Environment;

use tracing_error::ErrorLayer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{
    layer::{Layer, SubscriberExt},
    util::SubscriberInitExt,
};

pub fn install_tools(env: &Environment) -> eyre::Result<()> {
    install_tracing(env);
    install_eyre()?;
    Ok(())
}

fn install_eyre() -> eyre::Result<()> {
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default().into_hooks();

    eyre_hook.install()?;

    std::panic::set_hook(Box::new(move |pi| {
        tracing::error!("{}", panic_hook.panic_report(pi));
    }));
    Ok(())
}

fn install_tracing(env: &Environment) {
    let format_layer = {
        if env.dev_mode {
            tracing_subscriber::fmt::layer().pretty().boxed()
        } else {
            tracing_subscriber::fmt::layer().json().boxed()
        }
    };

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("debug"))
        .map(|f| {
            f.add_directive("hyper=error".parse().expect("could not make directive"))
                .add_directive("twitch_api=info".parse().expect("could not make directive"))
                .add_directive("retainer=info".parse().expect("could not make directive"))
                .add_directive("tower_http=info".parse().expect("could not make directive"))
        })
        .expect("could not make filter layer");

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(format_layer)
        .with(ErrorLayer::default())
        .init();
}
