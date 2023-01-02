#![feature(int_roundings)]

pub fn row_major<Xs, Ys> (xs: Xs, ys: Ys)
    -> impl Iterator<Item = (Xs::Item, Ys::Item)>
where
    Xs: Iterator + Clone,
    Xs::Item: 'static,
    Ys: Iterator,
    Ys::Item: Clone + 'static,
{
    ys.flat_map(move |y| xs.clone().map(move |x| (x, y.clone())))
}

pub const fn fnv1a_64(bs: &[u8]) -> u64 {
    const H0: u64 = 0xcbf29ce4_84222325;
    const A:  u64 = 0x00000100_000001B3;

    let mut h = H0;
    let mut i = 0;
    while i != bs.len() {
        h ^= bs[i] as u64;
        h = h.wrapping_mul(A);
        i += 1;
    }
    h
}

pub mod unorm;
pub use unorm::{UNorm8, UNorm16, un8, un16};

