use std::sync::Mutex;

use super::types::AppInfo;
use super::{launchservices, scanner};

/// A file extension and its current default handler.
#[derive(Clone)]
pub struct Association {
    pub ext: String,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

/// Query the current default for every extension declared by the scanned
/// apps, in parallel. `on_progress` is called once per completed extension.
pub fn query_all(apps: &[AppInfo], on_progress: &(dyn Fn() + Sync)) -> Vec<Association> {
    let extensions = scanner::all_extensions(apps);
    let rows: Mutex<Vec<Association>> = Mutex::new(Vec::new());

    std::thread::scope(|s| {
        for chunk in extensions.chunks(20) {
            let rows = &rows;
            let chunk = chunk.to_vec();
            s.spawn(move || {
                for ext in chunk {
                    let bundle_id = launchservices::query_default_bundle_id(&ext).ok().flatten();
                    let app_name = bundle_id
                        .as_ref()
                        .map(|bid| scanner::resolve_name(apps, bid));
                    rows.lock().unwrap().push(Association {
                        ext,
                        app_name,
                        bundle_id,
                    });
                    on_progress();
                }
            });
        }
    });

    let mut rows = rows.into_inner().unwrap();
    rows.sort_by(|a, b| a.ext.cmp(&b.ext));
    rows
}

/// A URL scheme and its current default handler.
#[derive(Clone)]
pub struct SchemeAssociation {
    pub scheme: String,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

/// Schemes worth surfacing: those declared by more than one installed app,
/// plus the always-contested web/mail schemes. Vanity schemes declared by
/// exactly one app aren't a real choice — that app is the only handler.
pub fn contested_schemes(apps: &[AppInfo]) -> Vec<String> {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for app in apps {
        for scheme in &app.url_schemes {
            *counts.entry(scheme.to_lowercase()).or_default() += 1;
        }
    }

    let mut candidates: std::collections::BTreeSet<String> = counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .map(|(scheme, _)| scheme)
        .collect();
    for common in ["http", "https", "mailto"] {
        candidates.insert(common.to_string());
    }

    candidates.into_iter().collect()
}

/// Query the current default handler for every contested scheme.
pub fn query_all_schemes(apps: &[AppInfo]) -> Vec<SchemeAssociation> {
    contested_schemes(apps)
        .into_iter()
        .map(|scheme| {
            let bundle_id = launchservices::query_default_scheme_handler(&scheme)
                .ok()
                .flatten();
            let app_name = bundle_id
                .as_ref()
                .map(|bid| scanner::resolve_name(apps, bid));
            SchemeAssociation {
                scheme,
                app_name,
                bundle_id,
            }
        })
        .collect()
}
