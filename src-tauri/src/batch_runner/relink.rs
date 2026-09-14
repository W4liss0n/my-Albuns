use super::*;

impl BatchRunner {
    /// Batch relinks are explicit temporary maps; the editor's folder-only relink
    /// is a separate operation and never reaches this recursive search.
    pub(crate) fn relink(&mut self, item_id: &str, folder: &Path) -> Result<(), String> {
        if !self.items.iter().any(|item| item.id == item_id) {
            return Err("Projeto não encontrado no lote.".into());
        }
        self.find_relinks(folder, Some(item_id))
    }

    pub(crate) fn relink_all(&mut self, folder: &Path) -> Result<(), String> {
        self.find_relinks(folder, None)
    }

    fn find_relinks(&mut self, folder: &Path, individual: Option<&str>) -> Result<(), String> {
        if self.phase != BatchPhase::Prepared {
            return Err("Verifique o lote antes de religar as imagens.".into());
        }
        self.paths
            .capture(folder)
            .map_err(|error| error.to_string())?;
        let files = walk_files(folder, &mut self.paths, &|_| true)?;
        let mut proposals = vec![];
        for (index, item) in self.items.iter().enumerate() {
            if item.status != BatchItemStatus::Pending || individual.is_some_and(|id| id != item.id)
            {
                continue;
            }
            let missing = item
                .problems
                .iter()
                .filter(|problem| problem.kind == BatchProblemKind::MissingMedia)
                .filter_map(|problem| problem.media_id.as_deref())
                .collect::<HashSet<_>>();
            if missing.is_empty() {
                continue;
            }
            let loaded =
                load_item(&self.core, &mut self.paths, item).map_err(|problem| problem.message)?;
            for media in loaded.project().media() {
                let id = media.id().to_string();
                if !missing.contains(id.as_str()) {
                    continue;
                }
                let matches = files
                    .iter()
                    .filter(|candidate| {
                        candidate.file_name() == media.path().file_name()
                            && (individual.is_some()
                                || candidate.parent().is_some_and(|parent| {
                                    parent
                                        .ancestors()
                                        .take_while(|ancestor| ancestor.starts_with(folder))
                                        .any(|ancestor| {
                                            ancestor.file_name() == Some(item.name.as_ref())
                                        })
                                }))
                    })
                    .collect::<Vec<_>>();
                if let [replacement] = matches.as_slice() {
                    proposals.push((
                        index,
                        id,
                        TemporaryRelink {
                            original: media.path().to_path_buf(),
                            replacement: (*replacement).clone(),
                        },
                    ));
                }
            }
        }
        // A global search never assigns one candidate to different projects,
        // including claims retained from a previous partial search.
        let mut claims: HashMap<PathBuf, HashSet<usize>> = HashMap::new();
        for (index, item) in self.items.iter().enumerate() {
            for link in item
                .relinks
                .individual
                .values()
                .chain(item.relinks.global.values())
            {
                claims
                    .entry(link.replacement.clone())
                    .or_default()
                    .insert(index);
            }
        }
        for (index, _, link) in &proposals {
            claims
                .entry(link.replacement.clone())
                .or_default()
                .insert(*index);
        }
        for (index, id, link) in proposals {
            if individual.is_some() {
                self.items[index].relinks.individual.insert(id, link);
            } else if claims[&link.replacement].len() == 1 {
                self.items[index].relinks.global.insert(id, link);
            }
        }
        self.recheck();
        Ok(())
    }
}
