use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{LayoutDefinition, LayoutRules};

/// The persistent catalog owns this identity; geometry equality belongs to LayoutRules.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, TS)]
pub struct CustomLayoutId(#[ts(type = "string")] Uuid);

impl Serialize for CustomLayoutId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.hyphenated().to_string())
    }
}

impl<'de> Deserialize<'de> for CustomLayoutId {
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

impl CustomLayoutId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn is_valid(self) -> bool {
        self.0.get_version() == Some(uuid::Version::Random)
    }
}

impl std::fmt::Display for CustomLayoutId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomLayout {
    pub id: CustomLayoutId,
    pub definition: LayoutDefinition,
}

/// A confirmed read of the global catalog, outside every Project revision.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutCatalogSnapshot {
    pub revision: u64,
    pub entries: Vec<CustomLayout>,
}

impl LayoutCatalogSnapshot {
    pub fn is_valid(&self) -> bool {
        self.revision <= crate::project_document::MAX_SAFE_INTEGER
            && (self.revision > 0 || self.entries.is_empty())
            && self.entries.iter().enumerate().all(|(index, item)| {
                item.id.is_valid()
                    && LayoutRules::definition_is_valid(&item.definition)
                    && !self.entries[..index].iter().any(|previous| {
                        previous.id == item.id
                            || LayoutRules::same_definition(&previous.definition, &item.definition)
                    })
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveCustomLayoutResult {
    pub catalog_revision: u64,
    pub layout_id: CustomLayoutId,
    pub created: bool,
}
