use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::StoredLayout;

/// Identity of the portable copy owned by one Project.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, TS)]
pub struct LayoutFavoriteId(#[ts(type = "string")] Uuid);

impl Serialize for LayoutFavoriteId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        super::identity::serialize(self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for LayoutFavoriteId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        super::identity::deserialize(deserializer).map(Self)
    }
}

impl LayoutFavoriteId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn is_valid(self) -> bool {
        super::identity::is_valid(self.0)
    }
}

impl std::fmt::Display for LayoutFavoriteId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FavoriteLayout {
    pub id: LayoutFavoriteId,
    pub order: u64,
    pub layout: StoredLayout,
}
