/// Information about an installed application.
#[derive(Debug, Clone)]
pub struct AppInfo {
    pub name: String,
    pub bundle_id: String,
    pub extensions: Vec<String>,
}

/// The current default application for a file extension.
#[derive(Debug, Clone)]
pub struct DefaultApp {
    pub name: String,
    pub bundle_id: String,
}

