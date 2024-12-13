use actix_web::{
    delete, error, get,
    middleware::Logger,
    post,
    web::{self, Json, ServiceConfig},
    Result,
};
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
                .wrap(Logger::default())
                .service(retrieve_all_votes)
                .service(retrieve_vote)
                .service(add_vote)
                .service(delete_vote)
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
