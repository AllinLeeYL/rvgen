use rand::{Rng, RngExt};

pub fn cut_cake_randomly(
    total: usize,
    smallest_cake: Option<usize>,
    biggest_cake: Option<usize>,
    rng: &mut (impl Rng + ?Sized),
) -> Vec<usize> {
    let smallest_cake = smallest_cake.unwrap_or(1);
    let biggest_cake = biggest_cake.unwrap_or(32);

    assert!(
        smallest_cake > 0 && smallest_cake <= biggest_cake,
        "cake size must satisfy 1 <= smallest <= biggest"
    );

    let mut remaining = total;
    let mut sizes = Vec::new();

    while remaining > 0 {
        let lower = smallest_cake.min(remaining);
        let upper = biggest_cake.min(remaining);
        let size = rng.random_range(lower..=upper);

        sizes.push(size);
        remaining -= size;
    }

    sizes
}
