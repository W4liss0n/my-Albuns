//! A validated index belongs to one operation, never to a timestamp-based cache.
//! The Engine holds its transition gate across mutations and publication.
use std::collections::BTreeMap;

use super::{
    CacheFailure, CacheMediaSource, CacheMetadata, CacheMetadataEntry, CachePathPlan,
    CacheReusableGeneration, PreparedCacheStorage, current_metadata, load_metadata,
    metadata_is_current, publish_metadata,
};

#[derive(Clone, Debug)]
pub(super) struct CacheIndex {
    entries: BTreeMap<String, CacheMetadataEntry>,
}

impl CacheIndex {
    pub(super) fn read(
        storage: &PreparedCacheStorage,
        paths: &CachePathPlan,
        project_id: &str,
    ) -> Option<Self> {
        let metadata = load_metadata(storage, paths)?;
        metadata_is_current(&metadata, project_id, paths).then(|| Self {
            entries: metadata
                .entries
                .into_iter()
                .map(|entry| (entry.media_id.clone(), entry))
                .collect(),
        })
    }

    pub(super) fn read_or_empty(
        storage: &PreparedCacheStorage,
        paths: &CachePathPlan,
        project_id: &str,
    ) -> Self {
        Self::read(storage, paths, project_id).unwrap_or(Self {
            entries: BTreeMap::new(),
        })
    }

    pub(super) fn entry(&self, media_id: &str) -> Option<&CacheMetadataEntry> {
        self.entries.get(media_id)
    }

    pub(super) fn reusable(&self, source: &CacheMediaSource) -> Option<CacheReusableGeneration> {
        self.entry(source.media_id())
            .filter(|entry| entry.matches_source_path(source.source_path()))
            .and_then(|entry| entry.reusable().ok())
    }

    pub(super) fn upsert(&mut self, entry: CacheMetadataEntry) -> Option<CacheMetadataEntry> {
        self.entries.insert(entry.media_id.clone(), entry)
    }

    pub(super) fn commit(
        self,
        storage: &PreparedCacheStorage,
        paths: &CachePathPlan,
        project_id: &str,
    ) -> Result<CacheMetadata, CacheFailure> {
        let metadata = current_metadata(project_id, self.entries.into_values().collect())?;
        publish_metadata(storage, paths, &metadata)?;
        Ok(metadata)
    }
}
