//! Walk logic for reading the MFT record by record with `FSCTL_GET_NTFS_FILE_RECORD`, used
//! when raw volume reads are blocked (for example by an anti-cheat file-system filter).
//! Pure logic without I/O, so it is unit-tested.
//!
//! The FSCTL returns the highest in-use record whose number is at most the one requested.
//! Walking each range downward and asking next for one below the record that came back
//! visits every in-use record exactly once with about one call per in-use record.

/// After this many failed requests in a row, a range gives up with the last error.
pub(crate) const MAX_CONSECUTIVE_FAILURES: u64 = 64;

/// Splits record numbers `0..count` into at most `parts` contiguous, non-empty, inclusive
/// ranges `(lo, hi)`, lowest first.
pub(crate) fn split_ranges(count: u64, parts: usize) -> Vec<(u64, u64)> {
    if count == 0 {
        return Vec::new();
    }
    let parts = (parts.max(1) as u64).min(count);
    let chunk = count.div_ceil(parts);
    (0..parts)
        .map(|i| (i * chunk, ((i + 1) * chunk).min(count) - 1))
        .filter(|(lo, hi)| lo <= hi)
        .collect()
}

/// Counters from [`walk_down`].
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct WalkCounts {
    /// Calls made to `fetch`.
    pub requests: u64,
    /// Requests that failed or returned a record above the one requested.
    pub failed: u64,
}

/// Visits the in-use records of the inclusive range `lo..=hi`, from the top down.
///
/// `fetch(n)` returns the highest in-use record number `r <= n` and its payload. A record
/// inside the range is passed to `keep` and the walk continues below it; a record below
/// `lo` ends the walk (the rest belongs to another range). An answer above `n` is invalid:
/// it is counted as failed and the walk steps down by one. A failed request is counted and
/// skipped the same way; after [`MAX_CONSECUTIVE_FAILURES`] failures in a row the walk stops
/// with that error. `cancelled` is checked before every request.
pub(crate) fn walk_down<T, E>(
    lo: u64,
    hi: u64,
    mut fetch: impl FnMut(u64) -> Result<(u64, T), E>,
    mut keep: impl FnMut(u64, T),
    cancelled: impl Fn() -> bool,
) -> Result<WalkCounts, E> {
    let mut counts = WalkCounts::default();
    let below = |n: u64| n.checked_sub(1).filter(|&m| m >= lo);
    let mut next = Some(hi).filter(|&h| h >= lo);
    let mut failures_in_a_row = 0u64;

    while let Some(n) = next {
        if cancelled() {
            break;
        }
        counts.requests += 1;
        match fetch(n) {
            Ok((r, payload)) if r <= n => {
                failures_in_a_row = 0;
                if r < lo {
                    next = None;
                } else {
                    keep(r, payload);
                    next = below(r);
                }
            }
            Ok(_) => {
                counts.failed += 1;
                next = below(n);
            }
            Err(e) => {
                counts.failed += 1;
                failures_in_a_row += 1;
                if failures_in_a_row >= MAX_CONSECUTIVE_FAILURES {
                    return Err(e);
                }
                next = below(n);
            }
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeSet;

    use super::*;

    /// Simulates the FSCTL over a set of in-use record numbers.
    fn fsctl(in_use: &BTreeSet<u64>, n: u64) -> Result<(u64, ()), String> {
        in_use
            .range(..=n)
            .next_back()
            .map(|&r| (r, ()))
            .ok_or_else(|| "no record".to_string())
    }

    fn walk_all(in_use: &BTreeSet<u64>, count: u64, parts: usize) -> (Vec<u64>, u64) {
        let mut seen = Vec::new();
        let mut requests = 0;
        for (lo, hi) in split_ranges(count, parts) {
            let counts =
                walk_down(lo, hi, |n| fsctl(in_use, n), |r, ()| seen.push(r), || false).unwrap();
            requests += counts.requests;
        }
        (seen, requests)
    }

    #[test]
    fn ranges_cover_everything_once() {
        assert_eq!(split_ranges(10, 3), vec![(0, 3), (4, 7), (8, 9)]);
        assert_eq!(split_ranges(2, 8), vec![(0, 0), (1, 1)]);
        assert_eq!(split_ranges(5, 0), vec![(0, 4)]);
        assert!(split_ranges(0, 4).is_empty());
        for count in 1..40u64 {
            for parts in 1..9 {
                let ranges = split_ranges(count, parts);
                let covered: u64 = ranges.iter().map(|(lo, hi)| hi - lo + 1).sum();
                assert_eq!(covered, count, "{count} {parts}");
                assert!(ranges.windows(2).all(|w| w[0].1 + 1 == w[1].0));
            }
        }
    }

    #[test]
    fn every_in_use_record_is_visited_exactly_once() {
        let in_use: BTreeSet<u64> = [0, 1, 2, 5, 6, 9, 15, 16, 17, 30, 31].into();
        for parts in 1..8 {
            let (mut seen, requests) = walk_all(&in_use, 32, parts);
            seen.sort_unstable();
            assert_eq!(seen, in_use.iter().copied().collect::<Vec<_>>(), "{parts}");
            // One request per record plus at most one closing request per range.
            assert!(requests <= in_use.len() as u64 + parts as u64, "{requests}");
        }
    }

    #[test]
    fn walk_goes_downward_and_skips_free_gaps() {
        // Record 0 ($MFT) is always in use, so every walk ends there.
        let in_use: BTreeSet<u64> = [0, 3, 100, 1000].into();
        let asked = std::cell::RefCell::new(Vec::new());
        let mut seen = Vec::new();
        walk_down(
            0,
            2000,
            |n| {
                asked.borrow_mut().push(n);
                fsctl(&in_use, n)
            },
            |r, ()| seen.push(r),
            || false,
        )
        .unwrap();
        assert_eq!(seen, vec![1000, 100, 3, 0]);
        assert_eq!(*asked.borrow(), vec![2000, 999, 99, 2]);
    }

    #[test]
    fn answer_below_the_range_ends_it() {
        let in_use: BTreeSet<u64> = [2, 50].into();
        let mut seen = Vec::new();
        let counts = walk_down(
            10,
            40,
            |n| fsctl(&in_use, n),
            |r, ()| seen.push(r),
            || false,
        )
        .unwrap();
        assert!(seen.is_empty());
        assert_eq!(counts.requests, 1);
    }

    #[test]
    fn bogus_answers_and_errors_are_skipped_and_counted() {
        let mut seen = Vec::new();
        // n = 9: fails; n = 8: answers 12 (above n, invalid); n = 7: record 7; then record 4.
        let counts = walk_down(
            4,
            9,
            |n| match n {
                9 => Err("boom"),
                8 => Ok((12, ())),
                7 => Ok((7, ())),
                _ => Ok((4, ())),
            },
            |r, ()| seen.push(r),
            || false,
        )
        .unwrap();
        assert_eq!(seen, vec![7, 4]);
        assert_eq!(
            counts,
            WalkCounts {
                requests: 4,
                failed: 2
            }
        );
    }

    #[test]
    fn too_many_failures_in_a_row_stop_the_walk() {
        let result = walk_down(
            0,
            1000,
            |_| Err::<(u64, ()), _>("blocked"),
            |_, ()| {},
            || false,
        );
        assert_eq!(result, Err("blocked"));
    }

    #[test]
    fn cancellation_stops_before_the_next_request() {
        let calls = Cell::new(0);
        let counts = walk_down(
            0,
            100,
            |n| {
                calls.set(calls.get() + 1);
                Ok::<_, ()>((n, ()))
            },
            |_, ()| {},
            || calls.get() >= 3,
        )
        .unwrap();
        assert_eq!(counts.requests, 3);
    }
}
