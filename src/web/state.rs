use std::sync::Arc;

use parking_lot::{Mutex, RwLock};

use crate::config::SafehouseConfig;
use crate::db::Database;
use crate::dirs::SafehouseDirs;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub config: Arc<RwLock<SafehouseConfig>>,
    pub dirs: Arc<SafehouseDirs>,
    pub http: reqwest::Client,
    pub docker: Arc<bollard::Docker>,
}
