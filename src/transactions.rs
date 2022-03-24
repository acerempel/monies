use axum::Json;
use axum_macros::debug_handler;
use rusqlite::Row;
use serde::Deserialize;
use serde::Serialize;
use tracing::Instrument;
use tracing::debug_span;

use crate::DbPool;

#[debug_handler]
#[tracing::instrument]
pub(crate) async fn list(pool: DbPool) -> Result<Json<Vec<Transaction>>, String> {
    let txns = tokio::task::spawn_blocking(move || -> Result<Vec<Transaction>, eyre::Report>{
        let conn = pool.get()?;
        let txns: Vec<Transaction> = conn
            .prepare_cached("SELECT id, payee, description FROM transactions")?
            .query_map([], |row| row.try_into())?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;
        Ok(txns)
    }).instrument(debug_span!("db fetch transactions")).await
    .map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    Ok(Json(txns))
}

#[derive(Serialize)]
pub(crate) struct Transaction {
    id: i64,
    payee: String,
    description: String,
}

impl<'a> TryFrom<&'a Row<'_>> for Transaction {
    fn try_from(row: &'a Row) -> Result<Self, Self::Error> {
        Ok(Transaction { id: row.get(0)?, payee: row.get(1)?, description: row.get(2)? })
    }

    type Error = rusqlite::Error;
}

#[derive(Deserialize, Debug)]
pub(crate) struct CreateRequest {
    payee: String,
    description: String,
}

#[derive(Serialize)]
pub(crate) struct CreateResponse {
    id: i64,
}

#[tracing::instrument]
pub(crate) async fn create(pool: DbPool, Json(trans): Json<CreateRequest>) -> Result<Json<CreateResponse>, String> {
    let id = tokio::task::spawn_blocking(move || -> Result<i64, eyre::Report> {
        let conn = pool.get()?;
        conn.prepare_cached("INSERT INTO transactions (payee, description) VALUES (?, ?)")?
            .execute([trans.payee, trans.description])?;
        Ok(conn.last_insert_rowid())
    }).instrument(debug_span!("db transaction create"))
    .await
        .map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    Ok(Json(CreateResponse { id }))
}
