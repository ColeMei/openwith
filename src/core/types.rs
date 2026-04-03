/// Information about an installed application.
#[derive(Debug, Clone)]
pub struct AppInfo {
    pub name: String,
    pub bundle_id: String,
    pub extensions: Vec<String>,
}
