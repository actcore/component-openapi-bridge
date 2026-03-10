use std::collections::HashMap;
use std::sync::Mutex;

use crate::spec::OpenApiSpec;
use crate::tools::ResolvedTool;

/// Cached parsed spec and its resolved tools.
pub struct CachedSpec {
    pub spec: OpenApiSpec,
    pub tools: Vec<ResolvedTool>,
}

static CACHE: std::sync::OnceLock<Mutex<HashMap<String, CachedSpec>>> = std::sync::OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, CachedSpec>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Get cached tools for a URL, or return None if not cached.
pub fn get_cached(url: &str) -> Option<Vec<ResolvedTool>> {
    let lock = cache().lock().unwrap();
    lock.get(url).map(|c| c.tools.clone())
}

/// Get a cached tool by URL and tool name.
pub fn get_cached_tool(url: &str, tool_name: &str) -> Option<ResolvedTool> {
    let lock = cache().lock().unwrap();
    lock.get(url)
        .and_then(|c| c.tools.iter().find(|t| t.name == tool_name).cloned())
}

/// Get the base URL from a cached spec.
pub fn get_base_url(url: &str) -> Option<String> {
    let lock = cache().lock().unwrap();
    lock.get(url).map(|c| c.spec.base_url().to_string())
}

/// Cache a parsed spec and its resolved tools.
pub fn put_cached(url: String, spec: OpenApiSpec, tools: Vec<ResolvedTool>) {
    let mut lock = cache().lock().unwrap();
    lock.insert(url, CachedSpec { spec, tools });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_miss_returns_none() {
        assert!(get_cached("https://nonexistent.example.com/spec.json").is_none());
    }
}
