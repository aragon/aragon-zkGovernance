use actix_cors::Cors;
use actix_web::{
    delete, error, get,
    http::header,
    middleware::Logger,
    post,
    web::{self, Json, ServiceConfig},
    Result,
};
use std::process::Command;

use serde::{Deserialize, Serialize};
use shuttle_actix_web::ShuttleActixWeb;
use sqlx::{FromRow, PgPool};

#[get("/{id}")]
async fn retrieve_vote(path: web::Path<i32>, state: web::Data<AppState>) -> Result<Json<Vote>> {
    let vote = sqlx::query_as("SELECT * FROM votes WHERE id = $1")
        .bind(*path)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;

    Ok(Json(vote))
}

#[get("")]
async fn retrieve_all_votes(state: web::Data<AppState>) -> Result<Json<Vec<Vote>>> {
    let votes = sqlx::query_as("SELECT * FROM votes ORDER BY id DESC")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;

    Ok(Json(votes))
}

#[post("")]
async fn add_vote(vote: web::Json<VoteNew>, state: web::Data<AppState>) -> Result<Json<Vote>> {
    let vote = sqlx::query_as("INSERT INTO votes(account, option, proposal_id, dao) VALUES ($1, $2, $3, $4) RETURNING id, account, option, proposal_id, dao")
        .bind(&vote.account)
        .bind(&vote.option)
        .bind(&vote.proposal_id)
        .bind(&vote.dao)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;

    Ok(Json(vote))
}

#[delete("/{id}")]
async fn delete_vote(path: web::Path<i32>, state: web::Data<AppState>) -> Result<Json<()>> {
    sqlx::query("DELETE FROM votes WHERE id = $1")
        .bind(*path)
        .execute(&state.pool)
        .await
        .unwrap();

    Ok(Json(()))
}

#[post("/vote")]
async fn proof_vote(Json(payload): Json<VotingParams>) -> Result<Json<()>> {
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
        // .current_dir("../target/release/")
        .current_dir("./target/release/")
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
        .arg(format!("--testing={}", 0))
        .output()
        .expect("Failed to execute command");
    println!("Execution done");

    let _success = output.status.success();
    let message_out = String::from_utf8_lossy(&output.stdout).to_string();
    let message_stderr = String::from_utf8_lossy(&output.stderr).to_string();
    println!("{}", message_out);
    println!("{}", message_stderr);
    Ok(Json(()))
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_shared_db::Postgres] pool: PgPool,
) -> ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let state = web::Data::new(AppState { pool });

    let config = move |cfg: &mut ServiceConfig| {
        cfg.service(
            web::scope("/votes")
                .wrap(Cors::permissive())
                .wrap(Logger::default())
                .service(retrieve_all_votes)
                .service(retrieve_vote)
                .service(add_vote)
                .service(delete_vote)
                .service(proof_vote)
                .app_data(state),
        );
    };

    Ok(config.into())
}

#[derive(Deserialize)]
struct VoteNew {
    account: String,
    option: i16,
    proposal_id: i64,
    dao: String,
}

#[derive(Serialize, Deserialize, FromRow)]
struct Vote {
    id: i32,
    account: String,
    option: i16,
    proposal_id: i64,
    dao: String,
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
