use std::net::SocketAddr;
use std::ops::Deref;

use axum::extract::Extension;
use axum::{Router, routing};
use r2d2::{Pool, CustomizeConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), eyre::Report> {
    tracing_subscriber::fmt::init();
    info!("Sqlite version: {}", rusqlite::version());
    let pool = Pool::builder()
        .connection_customizer(Box::new(ConnectionCustomizer))
        .build(SqliteConnectionManager::file("database.db"))?;
    init_database(pool.get()?.deref())?;
    let app = Router::new()
        .route("/transactions/list", routing::get(transactions::list))
        .route("/transactions/new", routing::post(transactions::create))
        .layer(Extension(pool))
        .layer(TraceLayer::new_for_http());
    let addr = SocketAddr::from(([127, 0, 0, 1], 4000));
    info!("Listening on {}", addr);
    axum::Server::bind(&addr).serve(app.into_make_service()).await?;
    Ok(())
}

#[derive(Debug)]
struct ConnectionCustomizer;

impl CustomizeConnection<Connection, rusqlite::Error> for ConnectionCustomizer {
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        #[cfg(debug_assertions)] {
            let mut res = 0;
            conn.query_row("PRAGMA foreign_keys", [], |r| { res = r.get(0)?; Ok(()) })?;
            assert_eq!(res, 1, "PRAGMA foreign_keys not supported");
        }
        Ok(())
    }
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

type DbPool = Extension<Pool<SqliteConnectionManager>>;

mod transactions {
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
}
