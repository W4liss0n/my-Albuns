use std::{
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

use crate::operation::{external_path_root, validate_external_path};

/// Where a Linked file would be if the folders around the Project kept their
/// structure after a copy or a move, nearest first. Read only names: the
/// caller keeps the first candidate that exists.
///
/// Climbing from the Project's own folder, each folder whose name also names
/// a folder of `stored` anchors one candidate: that folder followed by what
/// came after the same name in `stored`. Nothing is listed or searched, so
/// the candidates are few and always the same for the same paths. The
/// Project's root itself never anchors, and a candidate equal to `stored` is
/// left out.
pub fn structural_candidates(project_folder: &Path, stored: &Path) -> Vec<PathBuf> {
    if validate_external_path(project_folder).is_err() || validate_external_path(stored).is_err() {
        return Vec::new();
    }
    let Ok((project_root, _)) = external_path_root(project_folder) else {
        return Vec::new();
    };
    let project_names = normal_names(project_folder);
    let stored_names = normal_names(stored);
    let Some((_, stored_folders)) = stored_names.split_last() else {
        return Vec::new();
    };
    let mut candidates: Vec<PathBuf> = Vec::new();
    for kept in (1..=project_names.len()).rev() {
        let anchor = project_names[kept - 1];
        for position in (0..stored_folders.len()).rev() {
            if !same_name(stored_folders[position], anchor) {
                continue;
            }
            let mut candidate = project_root.clone();
            candidate.extend(&project_names[..kept]);
            candidate.extend(&stored_names[position + 1..]);
            if !same_path(&candidate, stored)
                && !candidates.iter().any(|known| same_path(known, &candidate))
            {
                candidates.push(candidate);
            }
        }
    }
    candidates
}

/// Whether two validated paths name the same place, ignoring case as Windows
/// does, accented letters included. Spellings that reach one file through
/// different roots, such as a mapped drive and its share, stay different.
pub fn same_path(left: &Path, right: &Path) -> bool {
    let (Ok((left_root, _)), Ok((right_root, _))) =
        (external_path_root(left), external_path_root(right))
    else {
        return false;
    };
    let (left_names, right_names) = (normal_names(left), normal_names(right));
    same_name(left_root.as_os_str(), right_root.as_os_str())
        && left_names.len() == right_names.len()
        && left_names
            .iter()
            .zip(&right_names)
            .all(|(left, right)| same_name(left, right))
}

fn normal_names(path: &Path) -> Vec<&OsStr> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect()
}

fn same_name(left: &OsStr, right: &OsStr) -> bool {
    if left == right {
        return true;
    }
    match (left.to_str(), right.to_str()) {
        (Some(left), Some(right)) => left.to_lowercase() == right.to_lowercase(),
        _ => false,
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    const STORED_FOLDER: &str =
        r"\\servidor\Serviços\Prestador\2026\03 - Diagramação\Job 040 - Escola Exemplo";
    const COPIED_PROJECT: &str = r"E:\Jobs\Job 040 - Escola Exemplo\02 - Templates";

    fn stored(suffix: &str) -> PathBuf {
        Path::new(STORED_FOLDER).join(suffix)
    }

    fn candidates(project_folder: &str, stored: &Path) -> Vec<PathBuf> {
        structural_candidates(Path::new(project_folder), stored)
    }

    #[test]
    fn a_file_beside_the_project_is_looked_for_beside_the_copy_first() {
        assert_eq!(
            candidates(COPIED_PROJECT, &stored(r"02 - Templates\MOLDURA.png")),
            [PathBuf::from(
                r"E:\Jobs\Job 040 - Escola Exemplo\02 - Templates\MOLDURA.png"
            )]
        );
    }

    #[test]
    fn a_folder_above_the_project_anchors_its_sibling_folders() {
        assert_eq!(
            candidates(COPIED_PROJECT, &stored(r"03 - Artes\x.png")),
            [PathBuf::from(
                r"E:\Jobs\Job 040 - Escola Exemplo\03 - Artes\x.png"
            )]
        );
    }

    #[test]
    fn subfolders_under_the_project_keep_their_place() {
        assert_eq!(
            candidates(
                COPIED_PROJECT,
                &stored(r"02 - Templates\_qq\QQ\P00084_I.jpg")
            ),
            [PathBuf::from(
                r"E:\Jobs\Job 040 - Escola Exemplo\02 - Templates\_qq\QQ\P00084_I.jpg"
            )]
        );
    }

    #[test]
    fn folder_names_match_regardless_of_case_including_accents() {
        assert_eq!(
            candidates(
                r"E:\Jobs\job 040 - escola exemplo\02 - TEMPLATES",
                &stored(r"02 - Templates\MOLDURA.png"),
            ),
            [PathBuf::from(
                r"E:\Jobs\job 040 - escola exemplo\02 - TEMPLATES\MOLDURA.png"
            )]
        );
        assert_eq!(
            candidates(
                r"E:\SERVIÇOS\Arte",
                Path::new(r"\\srv\s\Serviços\Arte\a.png"),
            ),
            [PathBuf::from(r"E:\SERVIÇOS\Arte\a.png")]
        );
    }

    #[test]
    fn nearer_folders_come_first_and_a_repeated_name_tries_the_deepest_first() {
        assert_eq!(
            candidates(
                r"E:\Album\Fotos",
                Path::new(r"\\srv\s\Fotos\Album\Fotos\a.jpg"),
            ),
            [
                PathBuf::from(r"E:\Album\Fotos\a.jpg"),
                PathBuf::from(r"E:\Album\Fotos\Album\Fotos\a.jpg"),
            ]
        );
        assert_eq!(
            candidates(
                r"E:\Jobs\J040\02 - Templates",
                Path::new(r"\\srv\s\J040\03 - Artes\a.png"),
            ),
            [PathBuf::from(r"E:\Jobs\J040\03 - Artes\a.png")]
        );
    }

    #[test]
    fn nothing_in_common_gives_no_candidate() {
        assert!(candidates(COPIED_PROJECT, Path::new(r"\\srv\Banco\Decorativos\z.png")).is_empty());
    }

    #[test]
    fn the_root_never_anchors_a_candidate() {
        assert!(candidates(r"E:\Templates", Path::new(r"\\srv\s\a.png")).is_empty());
        assert_eq!(
            candidates(r"E:\Templates", Path::new(r"\\srv\s\Templates\a.png")),
            [PathBuf::from(r"E:\Templates\a.png")]
        );
    }

    #[test]
    fn the_stored_path_itself_is_not_a_candidate() {
        let stored = stored(r"02 - Templates\MOLDURA.png");
        let project = Path::new(STORED_FOLDER).join("02 - Templates");
        assert!(structural_candidates(&project, &stored).is_empty());
        let shouted = PathBuf::from(stored.to_string_lossy().to_uppercase());
        assert!(structural_candidates(&project, &shouted).is_empty());
    }

    #[test]
    fn a_mapped_drive_and_its_share_are_different_spellings() {
        assert_eq!(
            candidates(
                r"Z:\J040\02 - Templates",
                Path::new(r"\\srv\s\J040\02 - Templates\a.png"),
            ),
            [PathBuf::from(r"Z:\J040\02 - Templates\a.png")]
        );
    }

    #[test]
    fn invalid_paths_give_no_candidate() {
        assert!(candidates(r"Templates", &stored(r"02 - Templates\a.png")).is_empty());
        assert!(candidates(COPIED_PROJECT, Path::new(r"02 - Templates\a.png")).is_empty());
    }
}
