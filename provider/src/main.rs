use axum::extract::DefaultBodyLimit;
use bytes::Bytes;
use clap::{command, Parser, Subcommand};
use config::Config;
use openaiapi::server;
use std::{net::Ipv4Addr, time::Duration};
use tokio::net::TcpListener;
use tower_http::{
    limit::RequestBodyLimitLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::info;

use provider::{ProviderBaseService, ProviderConfig, ProviderOaiService};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Run(RunCli),
}

#[derive(Debug, Parser)]
pub struct RunCli {
    #[clap(long, default_value = "127.0.0.1")]
    host: Ipv4Addr,

    #[clap(long, default_value = "8080")]
    port: u16,

    #[clap(long)]
    config: Option<String>,
}

pub async fn start_server(addr: &str, args: RunCli) {
    tracing_subscriber::fmt().init();
    let provider_model_config = match args.config {
        Some(config_filename) => {
            match Config::builder()
                .add_source(config::File::with_name(&config_filename))
                .build()
            {
                Ok(config) => match config.try_deserialize::<ProviderConfig>() {
                    Ok(config) => config,
                    Err(e) => {
                        panic!("Error parsing config: {}", e);
                    }
                },
                Err(e) => {
                    panic!("Error reading config filename {}: {}", config_filename, e);
                }
            }
        }
        None => ProviderConfig::default(),
    };

    info!("Starting Provider API");
    let provider_base = ProviderBaseService::new();
    let provider_base_service = ProviderBaseService::router(provider_base);
    let provider_oai = ProviderOaiService::new(provider_model_config.clone());
    let provider_oai_service = server::new(provider_oai);
    let app = axum::Router::new()
        .layer(DefaultBodyLimit::disable())
        .layer(
            TraceLayer::new_for_http()
                .on_body_chunk(|chunk: &Bytes, latency: Duration, _: &tracing::Span| {
                    tracing::trace!(size_bytes = chunk.len(), latency = ?latency, "sending body chunk")
                })
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_response(DefaultOnResponse::new().include_headers(true).latency_unit(LatencyUnit::Micros)),
        )
        // All requests that prefix /oai will go here
        .layer(RequestBodyLimitLayer::new(500 * 1000 * 1000)) // 500MB
        .nest("/", provider_oai_service)
        .layer(DefaultBodyLimit::disable())
        .nest("/", provider_base_service);

    let listener = TcpListener::bind(addr).await.unwrap();
    info!("Listening on: {}", addr);
    axum::serve(listener, app).await.unwrap();
}

#[tokio::main(flavor = "multi_thread", worker_threads = 32)]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run(cli_args) => {
            let addr = format!("{}:{}", cli_args.host, cli_args.port);
            start_server(&addr, cli_args).await;
        }
    }
}
