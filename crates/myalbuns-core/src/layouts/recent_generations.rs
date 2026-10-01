use std::{collections::VecDeque, sync::Arc};

use super::{LayoutGeneration, LayoutQuery, generate_layouts};

/// Queries kept per Project session. Each result holds at most twenty
/// suggestions, so all of them stay within a few hundred kilobytes.
const CAPACITY: usize = 32;

/// Recent Generator results of one Project session, most recently used first.
///
/// The Generator is deterministic, so an equal query reuses the earlier result
/// instead of repeating the search. The Layouts panel asks again after every
/// applied suggestion and whenever it reopens. The whole query is the key:
/// any input the Generator reads is part of it.
#[derive(Clone, Debug, Default)]
pub(crate) struct RecentLayoutGenerations {
    entries: VecDeque<(LayoutQuery, Arc<LayoutGeneration>)>,
}

impl RecentLayoutGenerations {
    pub(crate) fn generate(&mut self, query: &LayoutQuery) -> Arc<LayoutGeneration> {
        if let Some(index) = self.entries.iter().position(|(key, _)| key == query) {
            let entry = self.entries.remove(index).unwrap();
            let generation = Arc::clone(&entry.1);
            self.entries.push_front(entry);
            return generation;
        }
        let generation = Arc::new(generate_layouts(query));
        self.entries
            .push_front((query.clone(), Arc::clone(&generation)));
        self.entries.truncate(CAPACITY);
        generation
    }

    #[cfg(test)]
    pub(crate) fn stored_queries(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FrameOrientation, LayoutParameters, LayoutPermission, LayoutSurface, LayoutSurfaceKind,
    };

    fn query(margin_um: i64) -> LayoutQuery {
        LayoutQuery {
            surface: LayoutSurface {
                kind: LayoutSurfaceKind::DoubleSheet,
                width_um: 600_000,
                height_um: 300_000,
            },
            frame_orientations: vec![FrameOrientation::Vertical, FrameOrientation::Horizontal],
            permission: LayoutPermission::PagesAndSheet,
            parameters: LayoutParameters {
                margin_um,
                gap_um: 5_000,
                minimum_side_um: 20_000,
            },
        }
    }

    #[test]
    fn an_equal_query_reuses_the_generation_and_another_query_runs_the_generator() {
        let mut recent = RecentLayoutGenerations::default();
        let first = recent.generate(&query(15_000));
        let repeated = recent.generate(&query(15_000));
        let other = recent.generate(&query(20_000));

        assert!(Arc::ptr_eq(&first, &repeated));
        assert_eq!(*first, generate_layouts(&query(15_000)));
        assert_eq!(*other, generate_layouts(&query(20_000)));
        assert_ne!(*first, *other, "the Margin is part of the key");
    }

    #[test]
    fn the_least_recently_used_query_leaves_first() {
        let mut recent = RecentLayoutGenerations::default();
        let kept = recent.generate(&query(10_000));
        let dropped = recent.generate(&query(10_001));
        for margin_um in 10_002..10_000 + CAPACITY as i64 {
            recent.generate(&query(margin_um));
        }
        assert!(Arc::ptr_eq(&kept, &recent.generate(&query(10_000))));
        recent.generate(&query(20_000));

        assert_eq!(recent.entries.len(), CAPACITY);
        assert!(Arc::ptr_eq(&kept, &recent.generate(&query(10_000))));
        assert!(!Arc::ptr_eq(&dropped, &recent.generate(&query(10_001))));
    }
}
