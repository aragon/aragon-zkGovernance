use axum::{
    http::{HeaderValue, StatusCode},
    routing::post,
    Json, Router,
};
use dotenvy::dotenv;
use http::Method;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/", post(root)).layer(
        CorsLayer::new()
            .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
            .allow_headers([http::header::CONTENT_TYPE])
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS]),
    );

    println!("Listening on: http://0.0.0.0:3001");
    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// basic handler that responds with a static string
async fn root(Json(payload): Json<VotingParams>) -> (StatusCode, Json<VotingResponse>) {
    let voter_signature = &payload.voter_signature[2..];
    println!("Request received");
    println!("Blocknumber: {}", payload.block_number);
    println!("Voter Signature: {}", voter_signature);
    println!("Voter: {}", payload.voter);
    println!("DAO Address: {}", payload.dao_address);
    println!("Proposal Id: {}", payload.proposal_id);
    println!("Direction: {}", payload.direction);
    println!("Balance: {}", payload.balance);
    println!("Config Contract: {}", payload.config_contract);
    println!("Token Address: {}", payload.token_address);
    println!(
        "Additional Delegation Data: {:?}",
        payload.additional_delegation_data
    );

    let output = Command::new("./publisher")
        .current_dir("../target/release/")
        .env("BONSAI_API_KEY", std::env::var("BONSAI_API_KEY").unwrap())
        .env("BONSAI_API_URL", std::env::var("BONSAI_API_URL").unwrap())
        .env("RPC_URL", std::env::var("RPC_URL").unwrap())
        .env(
            "ETH_WALLET_PRIVATE_KEY",
            std::env::var("ETH_WALLET_PRIVATE_KEY").unwrap(),
        )
        .arg(format!(
            "--chain-id={}",
            std::env::var("CHAIN_ID").unwrap_or_else(|_| "11155111".to_string())
        ))
        .arg(format!("--rpc-url={}", std::env::var("RPC_URL").unwrap()))
        .arg(format!("--block-number={}", payload.block_number))
        .arg(format!("--voter-signature={}", voter_signature))
        .arg(format!("--voter={}", payload.voter))
        .arg(format!("--dao-address={}", payload.dao_address))
        .arg(format!("--proposal-id={}", payload.proposal_id))
        .arg(format!("--direction={}", payload.direction))
        .arg(format!("--balance={}", payload.balance))
        .arg(format!("--config-contract={}", payload.config_contract))
        .arg(format!("--token={}", payload.token_address))
        .arg(format!(
            "--additional-delegation-data={}",
            payload.additional_delegation_data
        ))
        .output()
        .expect("Failed to execute command");

    println!("Execution done");

    let success = output.status.success();
    let message_out = String::from_utf8_lossy(&output.stdout).to_string();
    let message_stderr = String::from_utf8_lossy(&output.stderr).to_string();
    println!("{}", message_out);
    println!("{}", message_stderr);

    let response = VotingResponse {
        success,
        message: message_out,
    };

    (
        if success {
            StatusCode::OK
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        },
        Json(response),
    )
}

#[derive(Deserialize)]
struct VotingParams {
    block_number: String,
    voter_signature: String,
    voter: String,
    dao_address: String,
    proposal_id: String,
    direction: u8,
    balance: String,
    config_contract: String,
    token_address: String,
    additional_delegation_data: String,
}

#[derive(Serialize)]
struct VotingResponse {
    success: bool,
    message: String,
}
