use crate::db::Db;

#[derive(Clone)]
pub struct AppState {
    pub mcp_port: u16,
    pub db: Db,
}
