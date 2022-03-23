use std::net::SocketAddr;
use std::ops::Deref;

use axum::extract::Extension;
use axum::{Router, routing, Json};
use axum_macros::debug_handler;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Row;
use serde::Serialize;

#[tokio::main]
async fn main() -> Result<(), eyre::Report> {
    let pool = Pool::new(SqliteConnectionManager::file("database.db"))?;
    init_database(pool.get()?.deref())?;
    let app = Router::new()
        .route("/transactions", routing::get(list_transactions))
        .layer(Extension(pool));
    let addr = SocketAddr::from(([127, 0, 0, 1], 4000));
    axum::Server::bind(&addr).serve(app.into_make_service()).await?;
    Ok(())
}

#[debug_handler]
async fn list_transactions(pool: Extension<Pool<SqliteConnectionManager>>) -> Result<Json<Vec<Transaction>>, String> {
    let txns = tokio::task::spawn_blocking(move || -> Result<Vec<Transaction>, eyre::Report>{
        let conn = pool.get()?;
        let txns: Vec<Transaction> = conn
            .prepare_cached("SELECT id, payee, description FROM transactions")?
            .query_map([], |row| row.try_into())?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;
        Ok(txns)
    }).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    Ok(Json(txns))
}

#[derive(Serialize)]
struct Transaction {
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

fn init_database(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute("
        CREATE TABLE IF NOT EXISTS transactions (
            id INTEGER PRIMARY KEY,
            payee TEXT,
            description TEXT
        );
        CREATE TABLE IF NOT EXISTS postings (
            id INTEGER PRIMARY KEY,
            date TEXT NOT NULL,
            amount INTEGER NOT NULL,
            account INTEGER NOT NULL,
            transaction INTEGER NOT NULL,
            FOREIGN KEY (account) REFERENCES accounts(id),
            FOREIGN KEY (transaction) REFERENCES transactions(id)
        );
        CREATE TABLE IF NOT EXISTS accounts (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            kind INTEGER NOT NULL
        );
    ", [])?;
    Ok(())
}
