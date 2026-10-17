//! Seeded shuffling used by the queue: deterministic for a given seed, so it is testable.
use super::QueueEntry;

/// A random seed for shuffling.
pub fn fresh_seed() -> u64 {
    let id = uuid::Uuid::new_v4().as_u128();
    (id as u64) ^ ((id >> 64) as u64)
}

/// splitmix64: tiny, fast, good enough for shuffling, and deterministic.
fn next_random(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Fisher–Yates, then separate equal neighbours and keep `avoid_first` off the front.
pub(super) fn shuffled(
    mut items: Vec<QueueEntry>,
    state: &mut u64,
    avoid_first: Option<&str>,
) -> Vec<QueueEntry> {
    for i in (1..items.len()).rev() {
        let j = (next_random(state) % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
    if let Some(avoid) = avoid_first {
        if items.first().is_some_and(|e| e.track.id == avoid) {
            if let Some(j) = items.iter().position(|e| e.track.id != avoid) {
                items.swap(0, j);
            }
        }
    }
    separate_duplicates(&mut items, avoid_first);
    items
}

fn clashes(items: &[QueueEntry], i: usize) -> bool {
    let same = |a: usize, b: usize| items[a].track.id == items[b].track.id;
    (i > 0 && same(i - 1, i)) || (i + 1 < items.len() && same(i, i + 1))
}

/// Swap entries so equal tracks are not neighbours whenever the queue allows it.
fn separate_duplicates(items: &mut [QueueEntry], avoid_first: Option<&str>) {
    for _ in 0..3 {
        for i in 1..items.len() {
            if items[i].track.id != items[i - 1].track.id {
                continue;
            }
            for j in (0..items.len()).filter(|j| *j != i && *j + 1 != i) {
                items.swap(i, j);
                let front_ok = j != 0 || avoid_first != Some(items[0].track.id.as_str());
                if front_ok && !clashes(items, i) && !clashes(items, j) {
                    break;
                }
                items.swap(i, j);
            }
        }
    }
}
