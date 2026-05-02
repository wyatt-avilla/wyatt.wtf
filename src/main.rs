#![warn(clippy::pedantic)]

use axum::Router;
use clap::Parser;
use leptos::logging::log;
use leptos::prelude::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use wyattwtf::{
    app::{shell, App},
    server::{api_router, AppState, Cli, ServerConfig},
};

#[tokio::main]
async fn main() {
    let server_config =
        ServerConfig::from_cli(Cli::parse()).expect("failed to load server configuration");
    let toml_conf = get_configuration(None).unwrap();
    let addr = toml_conf.leptos_options.site_addr;
    let leptos_options = toml_conf.leptos_options;
    let routes = generate_route_list(App);

    let leptos_app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);
    let app = api_router(AppState::new(server_config)).merge(leptos_app);

    log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
