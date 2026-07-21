//! A small time-based cache in front of the marketplace APIs.
//!
//! Browsing is far more repetitive than it looks: opening the Plugins tab,
//! switching away and back, or installing an addon all re-run the same
//! searches and the same "what's the newest version?" lookups. Modrinth
//! rate-limits per IP (300 requests/minute, and a token does not raise it),
//! so the cheapest request is the one we never send.
//!
//! Entries are held in memory only — a restart starts cold, which is the
//! right trade for data that goes stale on its own schedule anyway.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::addons::sources::{AddonSearchResult, AddonVersion};

/// How long a search result list is reused. Marketplace listings shift
/// slowly, and this only has to outlive a bout of tab-switching.
const SEARCH_TTL: Duration = Duration::from_secs(5 * 60);

/// How long a project's newest version is reused. Update checks fire once
/// per installed addon, so this is the entry that saves the most requests.
const VERSION_TTL: Duration = Duration::from_secs(15 * 60);

/// Upper bound on entries per map, so a long session spent typing searches
/// can't grow the cache without limit. Expired entries are dropped first;
/// this only bites when everything is still live.
const MAX_ENTRIES: usize = 200;

struct Cached<T> {
    value: T,
    stored_at: Instant,
}

impl<T> Cached<T> {
    fn is_fresh(&self, ttl: Duration) -> bool {
        let age = self.stored_at.elapsed();
        age < ttl
    }
}

/// Cached marketplace responses, shared app-wide via `AppState`.
#[derive(Default)]
pub struct MarketplaceCache {
    searches: Mutex<HashMap<String, Cached<Vec<AddonSearchResult>>>>,
    versions: Mutex<HashMap<String, Cached<AddonVersion>>>,
}

impl MarketplaceCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn search(&self, key: &str) -> Option<Vec<AddonSearchResult>> {
        let searches = self.searches.lock().await;
        fresh_value(&searches, key, SEARCH_TTL)
    }

    /// Stores a search result list, except an empty one. An empty response is
    /// usually the marketplace having a bad day rather than a real answer, and
    /// caching it turns a blip into minutes of a browser that stays stubbornly
    /// empty after the marketplace has already recovered. Re-asking for
    /// nothing is cheap; serving nothing from memory is not.
    pub async fn store_search(&self, key: String, results: Vec<AddonSearchResult>) {
        if results.is_empty() {
            return;
        }
        let mut searches = self.searches.lock().await;
        insert(&mut searches, key, results, SEARCH_TTL);
    }

    pub async fn latest_version(&self, key: &str) -> Option<AddonVersion> {
        let versions = self.versions.lock().await;
        fresh_value(&versions, key, VERSION_TTL)
    }

    pub async fn store_latest_version(&self, key: String, version: AddonVersion) {
        let mut versions = self.versions.lock().await;
        insert(&mut versions, key, version, VERSION_TTL);
    }
}

fn fresh_value<T: Clone>(
    entries: &HashMap<String, Cached<T>>,
    key: &str,
    ttl: Duration,
) -> Option<T> {
    let entry = entries.get(key)?;
    if !entry.is_fresh(ttl) {
        return None;
    }
    Some(entry.value.clone())
}

fn insert<T>(entries: &mut HashMap<String, Cached<T>>, key: String, value: T, ttl: Duration) {
    if entries.len() >= MAX_ENTRIES {
        entries.retain(|_, entry| entry.is_fresh(ttl));
    }
    // Still full even after pruning — drop the whole map rather than let it
    // grow unbounded. Cache misses are cheap; leaks are not.
    if entries.len() >= MAX_ENTRIES {
        entries.clear();
    }

    entries.insert(
        key,
        Cached {
            value,
            stored_at: Instant::now(),
        },
    );
}

/// Cache key for one marketplace search. The loader facet and MC version are
/// part of the key because the same query returns different results per
/// server.
pub fn search_key(
    source: &str,
    project_type: &str,
    loader_facet: &str,
    mc_version: &str,
    query: &str,
) -> String {
    let key = format!("{source}|{project_type}|{loader_facet}|{mc_version}|{query}");
    key
}

/// Cache key for one project's newest compatible version.
pub fn version_key(source: &str, loader_facet: &str, mc_version: &str, project_id: &str) -> String {
    let key = format!("{source}|{loader_facet}|{mc_version}|{project_id}");
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(number: &str) -> AddonVersion {
        AddonVersion {
            version_id: format!("id-{number}"),
            version_number: number.to_string(),
        }
    }

    #[tokio::test]
    async fn stores_and_returns_a_value() {
        let cache = MarketplaceCache::new();
        cache
            .store_latest_version("key".to_string(), version("1.0"))
            .await;

        let hit = cache.latest_version("key").await;
        assert_eq!(hit.map(|v| v.version_number), Some("1.0".to_string()));
    }

    #[tokio::test]
    async fn misses_on_an_unknown_key() {
        let cache = MarketplaceCache::new();
        assert!(cache.latest_version("nothing-here").await.is_none());
    }

    #[tokio::test]
    async fn an_empty_result_list_is_never_cached() {
        let cache = MarketplaceCache::new();
        cache.store_search("key".to_string(), Vec::new()).await;

        // A marketplace outage must not keep answering for itself once it's
        // over — the next browse has to go and ask again.
        assert!(cache.search("key").await.is_none());
    }

    #[test]
    fn expired_entries_are_not_returned() {
        let mut entries = HashMap::new();
        entries.insert(
            "key".to_string(),
            Cached {
                value: version("1.0"),
                stored_at: Instant::now() - Duration::from_secs(60),
            },
        );

        assert!(fresh_value(&entries, "key", Duration::from_secs(30)).is_none());
        assert!(fresh_value(&entries, "key", Duration::from_secs(120)).is_some());
    }

    #[test]
    fn insert_prunes_instead_of_growing_forever() {
        let mut entries = HashMap::new();
        for index in 0..MAX_ENTRIES + 50 {
            insert(
                &mut entries,
                format!("key-{index}"),
                version("1.0"),
                Duration::from_secs(600),
            );
        }

        assert!(entries.len() <= MAX_ENTRIES);
    }

    #[test]
    fn keys_separate_servers_with_different_versions() {
        let first = search_key("modrinth", "plugin", "paper", "1.21", "chat");
        let second = search_key("modrinth", "plugin", "paper", "1.20", "chat");
        assert_ne!(first, second);
    }
}
