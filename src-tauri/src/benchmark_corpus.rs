use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use myalbuns_core::{MediaCatalogItem, MediaKind, PhotoSnapshot, ProjectCore, ProjectSession};
use myalbuns_imaging_protocol::MediaSource;
use serde::Deserialize;

use crate::sample_project::SampleProject;

const CORPUS_SCHEMA_VERSION: u32 = 2;

#[derive(Clone)]
pub(crate) struct BenchmarkCorpus {
    first: BenchmarkAlbum,
    second: BenchmarkAlbum,
}

#[derive(Clone)]
pub(crate) struct BenchmarkAlbum {
    photo_sources: Vec<BenchmarkSource>,
    decorative_source: BenchmarkSource,
}

#[derive(Clone)]
pub(crate) struct BenchmarkSource {
    media_source: MediaSource,
    name: String,
    source_width_px: u32,
    source_height_px: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CorpusManifest {
    schema_version: u32,
    root: PathBuf,
    decorative: ManifestPhoto,
    albums: Vec<ManifestAlbum>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestAlbum {
    slot: String,
    name: String,
    directory: PathBuf,
    photos: Vec<ManifestPhoto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestPhoto {
    media_id: String,
    name: String,
    source_path: PathBuf,
    source_width_px: u32,
    source_height_px: u32,
    source_bytes: u64,
    source_sha256: String,
}

impl BenchmarkCorpus {
    pub(crate) fn load(manifest_path: &Path) -> Result<Self, String> {
        let manifest: CorpusManifest = serde_json::from_slice(
            &fs::read(manifest_path)
                .map_err(|error| format!("Não foi possível ler o manifesto do corpus: {error}"))?,
        )
        .map_err(|error| format!("Manifesto do corpus inválido: {error}"))?;
        if manifest.schema_version != CORPUS_SCHEMA_VERSION {
            return Err(format!(
                "Versão do manifesto do corpus não suportada: {}.",
                manifest.schema_version
            ));
        }
        if manifest.albums.len() != 2 {
            return Err("O corpus precisa conter exatamente dois Álbuns.".into());
        }

        let root = canonical_directory(&manifest.root, "raiz do corpus")?;
        let generated_directory = canonical_directory(
            manifest_path
                .parent()
                .ok_or_else(|| "O manifesto do corpus não possui pasta.".to_string())?,
            "pasta gerada do corpus",
        )?;
        let decorative_source = load_manifest_source(
            manifest.decorative,
            &generated_directory,
            is_png,
            "Decorativo",
        )?;
        let mut first = None;
        let mut second = None;
        for album in manifest.albums {
            let slot = album.slot.clone();
            let loaded = BenchmarkAlbum::load(album, &root, decorative_source.clone())?;
            match slot.as_str() {
                "a" if first.is_none() => first = Some(loaded),
                "b" if second.is_none() => second = Some(loaded),
                "a" | "b" => {
                    return Err(format!("O slot {slot} aparece mais de uma vez no corpus."));
                }
                _ => return Err(format!("Slot de Álbum inválido: {slot}.")),
            }
        }

        Ok(Self {
            first: first.ok_or_else(|| "O corpus não contém o Álbum A.".to_string())?,
            second: second.ok_or_else(|| "O corpus não contém o Álbum B.".to_string())?,
        })
    }

    pub(crate) fn album_for(&self, sample: SampleProject) -> &BenchmarkAlbum {
        match sample {
            SampleProject::Horizon => &self.first,
            SampleProject::Aurora => &self.second,
        }
    }
}

impl BenchmarkAlbum {
    fn load(
        album: ManifestAlbum,
        root: &Path,
        decorative_source: BenchmarkSource,
    ) -> Result<Self, String> {
        if album.name.trim().is_empty() {
            return Err("O nome de um Álbum do corpus está vazio.".into());
        }
        let directory = canonical_directory(&album.directory, "pasta de Álbum")?;
        if directory.parent() != Some(root) {
            return Err(format!(
                "A pasta do Álbum {} não está diretamente na raiz autorizada.",
                album.slot
            ));
        }
        if album.photos.is_empty() {
            return Err(format!("O Álbum {} não contém Fotos.", album.slot));
        }

        let mut media_ids = HashSet::from([decorative_source.media_source.media_id().to_owned()]);
        let mut photo_sources = Vec::with_capacity(album.photos.len());
        for photo in album.photos {
            if !media_ids.insert(photo.media_id.clone()) {
                return Err(format!(
                    "Identificador de mídia duplicado no Álbum {}.",
                    album.slot
                ));
            }
            photo_sources.push(load_manifest_source(photo, &directory, is_jpeg, "Foto")?);
        }

        Ok(Self {
            photo_sources,
            decorative_source,
        })
    }

    pub(crate) fn open_session(
        &self,
        sample: SampleProject,
        sheet_count: usize,
    ) -> Result<ProjectSession, String> {
        let template_source = sample
            .persisted_source(sheet_count)
            .map_err(|error| error.to_string())?;
        let template = ProjectCore::open_editable_session(&template_source)
            .map_err(|error| error.to_string())?;
        let mut state = template.state();
        let palettes = state
            .album
            .media
            .iter()
            .filter(|media| media.kind == MediaKind::Photo)
            .map(|media| media.palette.clone())
            .collect::<Vec<_>>();

        for (index, frame) in state
            .album
            .sheets
            .iter_mut()
            .flat_map(|sheet| &mut sheet.frames)
            .filter(|frame| frame.photo.is_some())
            .enumerate()
        {
            let source = &self.photo_sources[index % self.photo_sources.len()];
            frame.photo = Some(PhotoSnapshot {
                media_id: source.media_source.media_id().to_owned(),
                transform: Default::default(),
            });
        }

        for sheet in &mut state.album.sheets {
            if sheet.overlay_media_id.is_some() {
                sheet.overlay_media_id =
                    Some(self.decorative_source.media_source.media_id().to_owned());
            }
        }

        state.album.media = self
            .photo_sources
            .iter()
            .enumerate()
            .map(|(index, source)| MediaCatalogItem {
                id: source.media_source.media_id().to_owned(),
                kind: MediaKind::Photo,
                name: source.name.clone(),
                source_width_px: source.source_width_px,
                source_height_px: source.source_height_px,
                palette: palettes[index % palettes.len()].clone(),
            })
            .chain(std::iter::once(MediaCatalogItem {
                id: self.decorative_source.media_source.media_id().to_owned(),
                kind: MediaKind::Decorative,
                name: self.decorative_source.name.clone(),
                source_width_px: self.decorative_source.source_width_px,
                source_height_px: self.decorative_source.source_height_px,
                palette: ["#17344a".into(), "#88b7c5".into(), "#d4a15e".into()],
            }))
            .collect();

        let mut document: serde_json::Value = serde_json::from_str(
            &template
                .persisted_revision()
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("Fixture persistida inválida: {error}"))?;
        document["album"] = serde_json::to_value(state.album)
            .map_err(|error| format!("Não foi possível montar o Álbum do corpus: {error}"))?;
        ProjectCore::open_editable_session(
            &serde_json::to_string(&document)
                .map_err(|error| format!("Não foi possível serializar o corpus: {error}"))?,
        )
        .map_err(|error| error.to_string())
    }

    #[cfg(test)]
    pub(crate) fn sources(&self) -> &[BenchmarkSource] {
        &self.photo_sources
    }

    pub(crate) fn media_sources(&self) -> Vec<MediaSource> {
        self.photo_sources
            .iter()
            .chain(std::iter::once(&self.decorative_source))
            .map(|source| source.media_source.clone())
            .collect()
    }
}

impl BenchmarkSource {
    #[cfg(test)]
    pub(crate) fn source_path(&self) -> &Path {
        self.media_source.source_path()
    }
}

fn load_manifest_source(
    source: ManifestPhoto,
    authorized_directory: &Path,
    accepts_format: fn(&Path) -> bool,
    kind: &str,
) -> Result<BenchmarkSource, String> {
    if source.name.trim().is_empty() || source.source_width_px == 0 || source.source_height_px == 0
    {
        return Err(format!(
            "Metadados inválidos para a mídia {}.",
            source.media_id
        ));
    }
    let source_path = fs::canonicalize(&source.source_path).map_err(|error| {
        format!(
            "A mídia {} não pôde ser localizada: {error}",
            source.media_id
        )
    })?;
    if !source_path.starts_with(authorized_directory) || !accepts_format(&source_path) {
        return Err(format!(
            "{kind} {} está fora da pasta autorizada ou usa formato inválido.",
            source.media_id
        ));
    }
    let metadata = fs::metadata(&source_path).map_err(|error| {
        format!(
            "Os metadados da mídia {} estão indisponíveis: {error}",
            source.media_id
        )
    })?;
    if !metadata.is_file() || metadata.len() != source.source_bytes {
        return Err(format!(
            "O tamanho da mídia {} mudou desde a criação do manifesto.",
            source.media_id
        ));
    }

    Ok(BenchmarkSource {
        media_source: MediaSource::new(
            source.media_id,
            source_path,
            source.source_bytes,
            source.source_sha256.to_ascii_lowercase(),
        )
        .map_err(|error| format!("Fonte inválida no corpus: {error}"))?,
        name: source.name,
        source_width_px: source.source_width_px,
        source_height_px: source.source_height_px,
    })
}

fn canonical_directory(path: &Path, description: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("Não foi possível localizar {description}: {error}"))?;
    if !canonical.is_dir() {
        return Err(format!("{description} não é uma pasta."));
    }
    Ok(canonical)
}

fn is_jpeg(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg")
        })
}

fn is_png(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use crate::sample_project::SampleProject;

    use super::BenchmarkCorpus;

    #[test]
    fn loads_two_authorized_albums_as_real_photo_fixtures() {
        let root = tempfile::tempdir().expect("temporary corpus root");
        let first_album = root.path().join("album-a");
        let second_album = root.path().join("album-b");
        fs::create_dir_all(&first_album).expect("first album exists");
        fs::create_dir_all(&second_album).expect("second album exists");
        let first_photo = first_album.join("photo-a.jpg");
        let second_photo = second_album.join("photo-b.jpg");
        let decorative = root.path().join("decorative-overlay.png");
        fs::write(&first_photo, b"jpeg-a").expect("first fixture exists");
        fs::write(&second_photo, b"jpeg-b").expect("second fixture exists");
        fs::write(&decorative, b"png").expect("decorative fixture exists");
        let manifest_path = root.path().join("manifest.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&json!({
                "schemaVersion": 2,
                "root": root.path(),
                "decorative": {
                    "mediaId": "decorative-overlay",
                    "name": "decorative-overlay.png",
                    "sourcePath": decorative,
                    "sourceWidthPx": 2400,
                    "sourceHeightPx": 1800,
                    "sourceBytes": 3,
                    "sourceSha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                },
                "albums": [
                    {
                        "slot": "a",
                        "name": "album-a",
                        "directory": first_album,
                        "photos": [{
                            "mediaId": "benchmark-a-001",
                            "name": "photo-a.jpg",
                            "sourcePath": first_photo,
                            "sourceWidthPx": 6000,
                            "sourceHeightPx": 4000,
                            "sourceBytes": 6,
                            "sourceSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        }]
                    },
                    {
                        "slot": "b",
                        "name": "album-b",
                        "directory": second_album,
                        "photos": [{
                            "mediaId": "benchmark-b-001",
                            "name": "photo-b.jpg",
                            "sourcePath": second_photo,
                            "sourceWidthPx": 4000,
                            "sourceHeightPx": 6000,
                            "sourceBytes": 6,
                            "sourceSha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        }]
                    }
                ]
            }))
            .expect("manifest serializes"),
        )
        .expect("manifest is written");

        let corpus = BenchmarkCorpus::load(&manifest_path).expect("valid corpus loads");
        let horizon = corpus.album_for(SampleProject::Horizon);
        let aurora = corpus.album_for(SampleProject::Aurora);

        let horizon_state = horizon
            .open_session(SampleProject::Horizon, 12)
            .expect("the first corpus album opens through ProjectCore")
            .state();
        let aurora_state = aurora
            .open_session(SampleProject::Aurora, 12)
            .expect("the second corpus album opens through ProjectCore")
            .state();
        assert_eq!(horizon_state.album.media.len(), 2);
        assert_eq!(horizon_state.album.media[0].id, "benchmark-a-001");
        assert_eq!(horizon_state.album.media[1].id, "decorative-overlay");
        assert_eq!(
            horizon_state.album.sheets[0].overlay_media_id.as_deref(),
            Some("decorative-overlay")
        );
        assert_eq!(aurora_state.album.media[0].source_height_px, 6000);
        assert_eq!(horizon.media_sources().len(), 2);
        assert_eq!(
            horizon.sources()[0].source_path(),
            fs::canonicalize(first_album.join("photo-a.jpg"))
                .expect("the expected source path canonicalizes")
        );
    }
}
