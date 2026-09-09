use super::*;

fn similar(a: f64, b: f64) -> bool {
    (a - b).abs() <= 0.04 * a.max(b)
}
fn aligned(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-8
}
fn area(s: &Slot) -> f64 {
    s.bounds.w * s.bounds.h
}

pub(super) fn repetitive(slots: &[Slot]) -> bool {
    if slots.len() < 4 {
        return false;
    }
    for a in slots {
        let peers = slots
            .iter()
            .filter(|b| similar(a.bounds.w, b.bounds.w) && similar(a.bounds.h, b.bounds.h))
            .count();
        if peers == slots.len() || peers >= 4 && peers as f64 / slots.len() as f64 >= 0.75 {
            return true;
        }
    }
    let uniform = |group: Vec<&Slot>| {
        group.len() < 2
            || group.iter().map(|s| area(s)).fold(0.0, f64::max)
                / group.iter().map(|s| area(s)).fold(f64::INFINITY, f64::min)
                < 1.35
    };
    let groups = ORIENTATIONS.map(|o| {
        slots
            .iter()
            .filter(|s| s.orientation == o)
            .collect::<Vec<_>>()
    });
    if groups.iter().any(|g| g.len() >= 4 && uniform(g.clone()))
        || slots.len() >= 6 && groups.iter().all(|g| uniform(g.clone()))
    {
        return true;
    }
    let maximum = slots.iter().map(area).fold(0.0, f64::max);
    if slots.len() >= 6
        && slots.iter().any(|a| {
            maximum >= 2.5 * area(a)
                && [false, true].iter().any(|rows| {
                    slots
                        .iter()
                        .filter(|b| {
                            (a.bounds.w - b.bounds.w).abs() < 1e-9
                                && (a.bounds.h - b.bounds.h).abs() < 1e-9
                                && (if *rows {
                                    a.bounds.y - b.bounds.y
                                } else {
                                    a.bounds.x - b.bounds.x
                                })
                                .abs()
                                    < 1e-9
                        })
                        .count()
                        >= 5
                })
        })
    {
        return true;
    }
    for rows in [false, true] {
        let bounds = bounding_box(slots);
        let middle = if rows {
            bounds.y + bounds.h / 2.0
        } else {
            bounds.x + bounds.w / 2.0
        };
        let before: Vec<_> = slots
            .iter()
            .filter(|s| {
                let r = s.bounds;
                (if rows { r.y + r.h } else { r.x + r.w }) <= middle + 1e-8
            })
            .copied()
            .collect();
        let after: Vec<_> = slots
            .iter()
            .filter(|s| (if rows { s.bounds.y } else { s.bounds.x }) >= middle - 1e-8)
            .copied()
            .collect();
        if before.len() >= 2
            && before.len() == after.len()
            && before.len() + after.len() == slots.len()
        {
            let local_key = |group: &[Slot]| {
                let bounds = bounding_box(group);
                geometry_key(
                    &group
                        .iter()
                        .map(|s| Slot {
                            bounds: Bounds {
                                x: s.bounds.x - bounds.x,
                                y: s.bounds.y - bounds.y,
                                ..s.bounds
                            },
                            ..*s
                        })
                        .collect::<Vec<_>>(),
                    1.0,
                )
            };
            if local_key(&before) == local_key(&after) {
                return true;
            }
        }
        let position = |r: Bounds| if rows { r.y } else { r.x };
        let extent = |r: Bounds| if rows { r.h } else { r.w };
        let cross = |r: Bounds| if rows { r.x } else { r.y };
        let cross_extent = |r: Bounds| if rows { r.w } else { r.h };
        let mut tracks: Vec<Vec<&Slot>> = Vec::new();
        for s in slots {
            if let Some(track) = tracks.iter_mut().find(|t| {
                aligned(position(t[0].bounds), position(s.bounds))
                    && aligned(extent(t[0].bounds), extent(s.bounds))
            }) {
                track.push(s);
            } else {
                tracks.push(vec![s]);
            }
        }
        for track in &mut tracks {
            track.sort_by(|a, b| cross(a.bounds).total_cmp(&cross(b.bounds)));
        }
        for a in &tracks {
            let repeated: Vec<_> = tracks
                .iter()
                .filter(|b| {
                    a.len() == b.len()
                        && a.iter().zip(*b).all(|(r, s)| {
                            r.orientation == s.orientation
                                && similar(extent(r.bounds), extent(s.bounds))
                                && aligned(cross(r.bounds), cross(s.bounds))
                                && similar(cross_extent(r.bounds), cross_extent(s.bounds))
                        })
                })
                .collect();
            let covered: usize = repeated.iter().map(|t| t.len()).sum();
            if repeated.len() >= 2 && covered == slots.len()
                || repeated.len() >= 3 && covered as f64 / slots.len() as f64 >= 0.75
            {
                return true;
            }
        }
    }
    false
}
