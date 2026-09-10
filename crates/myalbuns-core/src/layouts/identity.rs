use serde::Deserialize;
use uuid::Uuid;

pub(super) fn serialize<S: serde::Serializer>(id: Uuid, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&id.hyphenated().to_string())
}

pub(super) fn deserialize<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Uuid, D::Error> {
    let value = String::deserialize(deserializer)?;
    let id = Uuid::parse_str(&value).map_err(serde::de::Error::custom)?;
    if !is_valid(id) || id.hyphenated().to_string() != value {
        return Err(serde::de::Error::custom(
            "Identidade de Layout inválida; esperado UUID v4 canônico",
        ));
    }
    Ok(id)
}

pub(super) fn is_valid(id: Uuid) -> bool {
    id.get_version() == Some(uuid::Version::Random)
}
