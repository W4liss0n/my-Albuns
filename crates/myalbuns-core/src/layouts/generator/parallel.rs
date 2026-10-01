use std::{panic, sync::Mutex, thread};

/// Runs `job` for every item on the available processors and returns the
/// results in item order, exactly as a sequential loop would. Starting a
/// thread costs more than a few cheap items, so one is added only for every
/// `items_per_thread` items. Items are handed out in short runs, about eight
/// per thread, so uneven jobs still share the work. Where threads are not
/// available, as in WebAssembly, the calling thread runs every item.
pub(super) fn map_in_order<T: Send, R: Send>(
    items: &mut [T],
    items_per_thread: usize,
    job: impl Fn(&mut T) -> R + Sync,
) -> Vec<R> {
    let workers = thread::available_parallelism()
        .map_or(1, |count| count.get())
        .min(items.len() / items_per_thread.max(1));
    if workers < 2 {
        return items.iter_mut().map(job).collect();
    }
    let run_length = items.len().div_ceil(workers * 8);
    let mut results: Vec<Option<Vec<R>>> = (0..items.len().div_ceil(run_length))
        .map(|_| None)
        .collect();
    let runs = Mutex::new(items.chunks_mut(run_length).enumerate());
    let work = || {
        let mut done = Vec::new();
        loop {
            let Some((index, run)) = runs.lock().unwrap().next() else {
                return done;
            };
            done.push((index, run.iter_mut().map(&job).collect::<Vec<_>>()));
        }
    };
    thread::scope(|scope| {
        // The calling thread works too, so a refused spawn only means fewer helpers.
        let helpers: Vec<_> = (1..workers)
            .map_while(|_| thread::Builder::new().spawn_scoped(scope, work).ok())
            .collect();
        let own = work();
        for done in helpers
            .into_iter()
            .map(|helper| {
                helper
                    .join()
                    .unwrap_or_else(|cause| panic::resume_unwind(cause))
            })
            .chain([own])
        {
            for (index, run) in done {
                results[index] = Some(run);
            }
        }
    });
    results
        .into_iter()
        .flat_map(|run| run.expect("every run is taken exactly once"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_follow_the_item_order_even_when_jobs_take_different_times() {
        let mut items: Vec<u64> = (0..500).collect();
        let job = |item: &mut u64| {
            // Uneven work, so workers finish their items out of order.
            (0..(*item * 7_919) % 5_000).fold(*item, |value, step| value.wrapping_mul(31) ^ step)
        };
        let expected: Vec<_> = items.clone().iter_mut().map(job).collect();

        assert_eq!(map_in_order(&mut items, 1, job), expected);
    }

    #[test]
    fn every_item_is_changed_in_place_exactly_once() {
        let mut items = vec![0_u32; 1_000];
        map_in_order(&mut items, 1, |item| *item += 1);

        assert!(items.iter().all(|&item| item == 1));
    }

    #[test]
    fn short_lists_stay_on_the_calling_thread() {
        let caller = thread::current().id();
        let threads = map_in_order(&mut [0_u8; 63], 64, |_| thread::current().id());

        assert!(threads.iter().all(|&id| id == caller));
    }

    #[test]
    fn empty_and_single_inputs_need_no_helpers() {
        assert!(map_in_order(&mut [] as &mut [u8], 1, |item| *item).is_empty());
        assert_eq!(map_in_order(&mut [7], 1, |item| *item * 2), vec![14]);
    }

    #[test]
    #[should_panic(expected = "item 3")]
    fn a_panicking_job_reaches_the_caller() {
        map_in_order(&mut [1, 2, 3, 4], 1, |item| {
            assert_ne!(*item, 3, "item {item}")
        });
    }
}
