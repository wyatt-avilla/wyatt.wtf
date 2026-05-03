#![cfg_attr(test, warn(clippy::pedantic))]
#![cfg_attr(test, allow(clippy::missing_errors_doc))]

use axum::Router;
use clap::Parser;
use leptos::logging::log;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list};
use wyattwtf::{
    app::{App, shell},
    server::{AppState, Cli, ServerConfig, api_router},
};

#[tokio::main]
async fn main() {
    let server_config =
        ServerConfig::try_from(Cli::parse()).expect("failed to load server configuration");
    let toml_conf = get_configuration(None).unwrap();
    let addr = toml_conf.leptos_options.site_addr;
    let leptos_options = toml_conf.leptos_options;
    let routes = generate_route_list(App);
    let app_state = AppState::new(server_config).expect("failed to build application state");

    let leptos_app = Router::new()
        .leptos_routes_with_context(
            &leptos_options,
            routes,
            {
                let app_state = app_state.clone();
                move || provide_context(app_state.clone())
            },
            {
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);
    let app = api_router(app_state).merge(leptos_app);

    log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
