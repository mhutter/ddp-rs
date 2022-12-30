use env_logger::Env;
use log::{error, info};
use serde_json::json;

#[tokio::main]
async fn main() {
    // enable log output. WARNING: `trace` will log all your messages in plain text, including any
    // secrets
    env_logger::init_from_env(Env::default().default_filter_or("ddp=trace"));

    let conn = ddp::connect("wss://open.rocket.chat/websocket")
        .await
        .unwrap_or_else(|err| {
            error!("Failed to establish connection: {err}");
            std::process::exit(1);
        });

    let res = conn
        .call(
            "login",
            Some(json!([{ "resume": "your-personal-access-token" }])),
        )
        .await
        .unwrap_or_else(|err| {
            error!("Failed to log in: {err}");
            std::process::exit(1);
        });

    info!("Login response: {res:?}");
}
