use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::StoredLayout;

/// Identity of the portable copy owned by one Project.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, TS)]
pub struct LayoutFavoriteId(#[ts(type = "string")] Uuid);

impl Serialize for LayoutFavoriteId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.hyphenated().to_string())
    }
}

impl<'de> Deserialize<'de> for LayoutFavoriteId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        let parsed = Uuid::parse_str(&value).map_err(serde::de::Error::custom)?;
        let id = Self(parsed);
        if !id.is_valid() || parsed.hyphenated().to_string() != value {
            return Err(serde::de::Error::custom(
                "Identidade de Layout inválida; esperado UUID v4 canônico",
            ));
        }
        Ok(id)
    }
}

impl LayoutFavoriteId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn is_valid(self) -> bool {
        self.0.get_version() == Some(uuid::Version::Random)
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
