use uuid::Uuid;

use super::*;
use crate::CoreError;

/// Immutable geometry and its captured mapping. Only the Session can commit it.
#[derive(Clone, Debug)]
pub struct LayoutPatch {
    definition: LayoutDefinition,
    frame_ids: Vec<Uuid>,
    placeholder_ids: Vec<Uuid>,
    last_layout: Option<StoredLayout>,
}

impl LayoutPatch {
    pub fn definition(&self) -> &LayoutDefinition {
        &self.definition
    }
    pub fn frame_ids(&self) -> &[Uuid] {
        &self.frame_ids
    }
    pub fn placeholder_ids(&self) -> &[Uuid] {
        &self.placeholder_ids
    }
    pub fn last_layout(&self) -> Option<&StoredLayout> {
        self.last_layout.as_ref()
    }
}

pub struct LayoutRules;

#[derive(Clone, Copy, Default)]
pub struct LayoutSources<'a> {
    pub last: Option<&'a StoredLayout>,
    pub custom: &'a [CustomLayout],
    pub favorites: &'a [FavoriteLayout],
}

impl LayoutSources<'_> {
    pub(crate) fn favorite_id(&self, layout: &StoredLayout) -> Option<LayoutFavoriteId> {
        self.favorites
            .iter()
            .find(|item| {
                item.layout.origin == layout.origin
                    && LayoutRules::same_definition(&item.layout.definition, &layout.definition)
            })
            .map(|item| item.id)
    }

    pub(crate) fn custom_id(&self, layout: &StoredLayout) -> Option<CustomLayoutId> {
        if layout.origin != LayoutOrigin::Custom {
            return None;
        }
        self.custom
            .iter()
            .find(|item| LayoutRules::same_definition(&item.definition, &layout.definition))
            .map(|item| item.id)
    }
}

impl LayoutRules {
    /// Captures only ordered geometry. Page blocks are centered independently;
    /// the source composition and its Frame mapping remain untouched.
    pub fn capture_custom(
        surface: LayoutSurface,
        positions: Vec<crate::RectUm>,
    ) -> Result<LayoutDefinition, CoreError> {
        let crosses = surface.kind == LayoutSurfaceKind::DoubleSheet
            && positions.iter().any(|r| {
                2 * i128::from(r.x) < i128::from(surface.width_um)
                    && 2 * (i128::from(r.x) + i128::from(r.width)) > i128::from(surface.width_um)
            });
        let mut definition = LayoutDefinition {
            surface,
            scope: if crosses {
                LayoutScope::Sheet
            } else {
                LayoutScope::Page
            },
            positions,
        };
        if !Self::definition_is_valid(&definition) {
            return Err(CoreError::IncompatibleLayout);
        }
        if definition.scope == LayoutScope::Page {
            center_page_blocks(&mut definition);
        }
        Ok(definition)
    }

    pub fn list_for_lock(query: &LayoutQuery, sources: LayoutSources<'_>) -> LayoutListing {
        Self::listing(query, sources, true)
    }

    pub fn list(query: &LayoutQuery, sources: LayoutSources<'_>) -> LayoutListing {
        Self::listing(query, sources, false)
    }

    fn listing(
        query: &LayoutQuery,
        sources: LayoutSources<'_>,
        allow_larger: bool,
    ) -> LayoutListing {
        let generation = generate_layouts(query);
        let mut listing = LayoutListing {
            algorithm_version: generation.algorithm_version,
            generation_status: generation.status,
            candidates: Vec::new(),
        };
        if generation.status == LayoutGenerationStatus::InvalidQuery {
            return listing;
        }
        let accepts = |definition: &LayoutDefinition| {
            compatible(definition, query)
                || allow_larger
                    && definition.positions.len() > query.frame_orientations.len()
                    && (query.permission == LayoutPermission::PagesAndSheet
                        || definition.scope == LayoutScope::Page)
                    && scaled_definition(definition, &query.surface).is_some()
        };
        if let Some(last) = sources.last.filter(|last| accepts(&last.definition)) {
            listing.candidates.push(LayoutCandidate {
                layout: last.clone(),
                is_last_applied: true,
                custom_id: sources.custom_id(last),
                favorite_id: sources.favorite_id(last),
            });
        }
        let mut favorites: Vec<_> = sources.favorites.iter().collect();
        favorites.sort_by_key(|item| (item.order, item.id));
        for favorite in favorites
            .into_iter()
            .filter(|item| accepts(&item.layout.definition))
        {
            if listing.candidates.iter().any(|item| {
                item.layout.origin == favorite.layout.origin
                    && Self::same_definition(&item.layout.definition, &favorite.layout.definition)
            }) {
                continue;
            }
            listing.candidates.push(LayoutCandidate {
                layout: favorite.layout.clone(),
                is_last_applied: false,
                custom_id: sources.custom_id(&favorite.layout),
                favorite_id: Some(favorite.id),
            });
        }
        for custom in sources
            .custom
            .iter()
            .filter(|item| accepts(&item.definition))
        {
            if listing.candidates.iter().any(|item| {
                item.layout.origin == LayoutOrigin::Custom
                    && Self::same_definition(&item.layout.definition, &custom.definition)
            }) {
                continue;
            }
            listing.candidates.push(LayoutCandidate {
                layout: StoredLayout {
                    definition: custom.definition.clone(),
                    origin: LayoutOrigin::Custom,
                },
                is_last_applied: false,
                custom_id: Some(custom.id),
                favorite_id: None,
            });
        }
        for candidate in generation.candidates {
            if listing.candidates.iter().any(|item| {
                item.layout.origin == LayoutOrigin::Automatic
                    && Self::same_definition(&item.layout.definition, &candidate.definition)
            }) {
                continue;
            }
            listing.candidates.push(LayoutCandidate {
                layout: StoredLayout {
                    definition: candidate.definition,
                    origin: LayoutOrigin::Automatic,
                },
                is_last_applied: false,
                custom_id: None,
                favorite_id: None,
            });
        }
        listing
    }

    pub fn resolve(
        layout: &StoredLayout,
        surface: &LayoutSurface,
        frame_ids: &[Uuid],
        permission: LayoutPermission,
    ) -> Result<LayoutPatch, CoreError> {
        Self::resolve_for_lock(layout, surface, frame_ids, &[], permission)
    }

    pub fn resolve_for_lock(
        layout: &StoredLayout,
        surface: &LayoutSurface,
        frame_ids: &[Uuid],
        placeholder_ids: &[Uuid],
        permission: LayoutPermission,
    ) -> Result<LayoutPatch, CoreError> {
        if layout.definition.positions.is_empty()
            || layout.definition.positions.len() != frame_ids.len() + placeholder_ids.len()
            || permission == LayoutPermission::PagesOnly
                && layout.definition.scope == LayoutScope::Sheet
        {
            return Err(CoreError::IncompatibleLayout);
        }
        let definition =
            scaled_definition(&layout.definition, surface).ok_or(CoreError::IncompatibleLayout)?;
        Ok(LayoutPatch {
            definition,
            frame_ids: frame_ids.to_vec(),
            placeholder_ids: placeholder_ids.to_vec(),
            last_layout: Some(layout.clone()),
        })
    }

    pub fn automatic(
        query: &LayoutQuery,
        sources: LayoutSources<'_>,
        frame_ids: &[Uuid],
    ) -> Result<LayoutPatch, CoreError> {
        if query.frame_orientations.len() != frame_ids.len() {
            return Err(CoreError::IncompatibleLayout);
        }
        let listing = Self::list(query, sources);
        if listing.generation_status == LayoutGenerationStatus::InvalidQuery {
            return Err(CoreError::InvalidLayoutQuery);
        }
        if let Some(candidate) = listing.candidates.first() {
            Self::resolve(
                &candidate.layout,
                &query.surface,
                frame_ids,
                query.permission,
            )
        } else {
            Self::reserve(&query.surface, frame_ids)
        }
    }

    pub fn same_definition(a: &LayoutDefinition, b: &LayoutDefinition) -> bool {
        if a.scope != b.scope
            || !same_proportion(&a.surface, &b.surface)
            || a.positions.len() != b.positions.len()
        {
            return false;
        }
        a.positions
            .iter()
            .zip(&b.positions)
            .all(|(a_rect, b_rect)| {
                [(a_rect.x, b_rect.x), (a_rect.width, b_rect.width)]
                    .iter()
                    .all(|(x, y)| {
                        i128::from(*x) * i128::from(b.surface.width_um)
                            == i128::from(*y) * i128::from(a.surface.width_um)
                    })
                    && [(a_rect.y, b_rect.y), (a_rect.height, b_rect.height)]
                        .iter()
                        .all(|(x, y)| {
                            i128::from(*x) * i128::from(b.surface.height_um)
                                == i128::from(*y) * i128::from(a.surface.height_um)
                        })
            })
    }

    pub(crate) fn definition_is_valid(definition: &LayoutDefinition) -> bool {
        if !definition.surface.is_valid() || definition.positions.is_empty() {
            return false;
        }
        let w = definition.surface.width_um;
        let h = definition.surface.height_um;
        if definition.positions.iter().any(|r| {
            r.x < 0
                || r.y < 0
                || r.width <= 0
                || r.height <= 0
                || i128::from(r.x) + i128::from(r.width) > i128::from(w)
                || i128::from(r.y) + i128::from(r.height) > i128::from(h)
        }) {
            return false;
        }
        let crosses = definition.surface.kind == LayoutSurfaceKind::DoubleSheet
            && definition
                .positions
                .iter()
                .any(|r| 2 * r.x < w && 2 * (r.x + r.width) > w);
        (definition.scope == LayoutScope::Sheet) == crosses
    }

    /// Reserve is deliberately absent from listings and never becomes Last Layout.
    /// Its input contains neither Photo metadata nor orientation or search settings.
    pub fn reserve(surface: &LayoutSurface, frame_ids: &[Uuid]) -> Result<LayoutPatch, CoreError> {
        if !surface.is_valid() {
            return Err(CoreError::InvalidLayoutQuery);
        }
        let mut positions = Vec::with_capacity(frame_ids.len());
        if surface.kind == LayoutSurfaceKind::DoubleSheet {
            let left_count = frame_ids.len().div_ceil(2);
            reserve_page(
                &mut positions,
                left_count,
                0,
                surface.width_um / 2,
                surface.height_um,
            );
            reserve_page(
                &mut positions,
                frame_ids.len() - left_count,
                surface.width_um - surface.width_um / 2,
                surface.width_um / 2,
                surface.height_um,
            );
        } else {
            reserve_page(
                &mut positions,
                frame_ids.len(),
                0,
                surface.width_um,
                surface.height_um,
            );
        }
        Ok(LayoutPatch {
            definition: LayoutDefinition {
                surface: surface.clone(),
                scope: LayoutScope::Page,
                positions,
            },
            frame_ids: frame_ids.to_vec(),
            placeholder_ids: Vec::new(),
            last_layout: None,
        })
    }
}

fn center_page_blocks(definition: &mut LayoutDefinition) {
    let double = definition.surface.kind == LayoutSurfaceKind::DoubleSheet;
    let width = definition.surface.width_um;
    let page_width = if double { width / 2 } else { width };
    for right in [false, true].into_iter().take(if double { 2 } else { 1 }) {
        let on_page = |r: &&crate::RectUm| !double || (2 * r.x >= width) == right;
        let frames = definition
            .positions
            .iter()
            .filter(on_page)
            .collect::<Vec<_>>();
        let Some(min_x) = frames.iter().map(|r| r.x).min() else {
            continue;
        };
        let min_y = frames.iter().map(|r| r.y).min().unwrap();
        let max_x = frames.iter().map(|r| r.x + r.width).max().unwrap();
        let max_y = frames.iter().map(|r| r.y + r.height).max().unwrap();
        let page_x = if right { width - page_width } else { 0 };
        let dx = page_x + (page_width - (max_x - min_x)) / 2 - min_x;
        let dy = (definition.surface.height_um - (max_y - min_y)) / 2 - min_y;
        for frame in definition
            .positions
            .iter_mut()
            .filter(|r| !double || (2 * r.x >= width) == right)
        {
            frame.x += dx;
            frame.y += dy;
        }
    }
}

fn same_proportion(a: &LayoutSurface, b: &LayoutSurface) -> bool {
    a.kind == b.kind
        && a.is_valid()
        && b.is_valid()
        && i128::from(a.width_um) * i128::from(b.height_um)
            == i128::from(b.width_um) * i128::from(a.height_um)
}

fn compatible(definition: &LayoutDefinition, query: &LayoutQuery) -> bool {
    definition.positions.len() == query.frame_orientations.len()
        && (query.permission == LayoutPermission::PagesAndSheet
            || definition.scope == LayoutScope::Page)
        && scaled_definition(definition, &query.surface).is_some()
}

fn scaled_definition(
    definition: &LayoutDefinition,
    surface: &LayoutSurface,
) -> Option<LayoutDefinition> {
    if !LayoutRules::definition_is_valid(definition)
        || !same_proportion(&definition.surface, surface)
    {
        return None;
    }
    let scale = |value: i64| {
        let denominator = i128::from(definition.surface.width_um);
        ((i128::from(value) * i128::from(surface.width_um) + denominator / 2) / denominator) as i64
    };
    let positions = definition
        .positions
        .iter()
        .map(|r| {
            let mut x = scale(r.x);
            let y = scale(r.y);
            let mut right = scale(r.x + r.width);
            let bottom = scale(r.y + r.height);
            if definition.scope == LayoutScope::Page
                && surface.kind == LayoutSurfaceKind::DoubleSheet
            {
                if 2 * (r.x + r.width) <= definition.surface.width_um {
                    right = right.min(surface.width_um / 2);
                }
                if 2 * r.x >= definition.surface.width_um {
                    x = x.max(surface.width_um - surface.width_um / 2);
                }
            }
            RectUm {
                x,
                y,
                width: right - x,
                height: bottom - y,
            }
        })
        .collect();
    let scaled = LayoutDefinition {
        surface: surface.clone(),
        scope: definition.scope,
        positions,
    };
    LayoutRules::definition_is_valid(&scaled).then_some(scaled)
}

fn reserve_page(output: &mut Vec<RectUm>, count: usize, offset: i64, width: i64, height: i64) {
    if count == 0 {
        return;
    }
    let columns = ((count as f64 * width as f64 / height as f64).sqrt().ceil() as usize)
        .clamp(1, count)
        .min(width as usize);
    let rows = count.div_ceil(columns).min(height as usize);
    let gap = (width.min(height) / 30)
        .min(width / (3 * columns as i64 + 1))
        .min(height / (3 * rows as i64 + 1));
    let cell_width = (width - (columns as i64 + 1) * gap) / columns as i64;
    let cell_height = (height - (rows as i64 + 1) * gap) / rows as i64;
    let x = offset + (width - columns as i64 * cell_width - (columns as i64 - 1) * gap) / 2;
    let y = (height - rows as i64 * cell_height - (rows as i64 - 1) * gap) / 2;
    for index in 0..count {
        // At a surface's integer precision limit, repeated cells remain valid
        // editable Frames. Reserve must not truncate the supported Frame list.
        output.push(RectUm {
            x: x + (index % columns) as i64 * (cell_width + gap),
            y: y + (index / columns % rows) as i64 * (cell_height + gap),
            width: cell_width,
            height: cell_height,
        });
    }
}
