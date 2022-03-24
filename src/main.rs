use std::net::SocketAddr;
use std::ops::Deref;
use std::path::PathBuf;

use axum::extract::Extension;
use axum::{Router, routing};
use r2d2::{Pool, CustomizeConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::config::Config;

#[tokio::main]
async fn main() -> Result<(), eyre::Report> {
    tracing_subscriber::fmt::init();
    let config_path = "kleingeld.toml";
    let config = Config::get(config_path.as_ref())?;
    info!("Sqlite version: {}", rusqlite::version());
    let connection_manager = if config.db_file == PathBuf::from("memory") {
        SqliteConnectionManager::memory()
    } else {
        SqliteConnectionManager::file(config.db_file)
    };
    let pool = Pool::builder()
        .connection_customizer(Box::new(ConnectionCustomizer))
        .build(connection_manager)?;
    init_database(pool.get()?.deref())?;
    let app = Router::new()
        .route("/transactions/list", routing::get(transactions::list))
        .route("/transactions/new", routing::post(transactions::create))
        .layer(Extension(pool))
        .layer(TraceLayer::new_for_http());
    let addr = SocketAddr::from((config.address, config.port));
    info!("Listening on {}", addr);
    axum::Server::bind(&addr).serve(app.into_make_service()).await?;
    Ok(())
}

mod config;

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

mod transactions;
