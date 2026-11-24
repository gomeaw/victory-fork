mod baseline {
fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    let whole = a.len().min(b.len()) / 8 * 8;
    let from = a[..whole]
        .as_chunks::<8>().0.iter()
        .zip(b[..whole].chunks_exact(8))
        .position(|(x, y)| x != y)
        .map_or(whole, |word| word * 8);
    a[from..].iter().zip(&b[from..]).position(|(x, y)| x != y).map(|i| from + i)
}

/// Where `a` and `b` (of one length) last differ.
fn last_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    let whole = a.len().min(b.len()) / 8 * 8;
    if let Some(i) = a[whole..].iter().zip(&b[whole..]).rposition(|(x, y)| x != y) {
        return Some(whole + i);
    }
    let word = a[..whole]
        .as_chunks::<8>().0.iter()
        .zip(b[..whole].chunks_exact(8))
        .rposition(|(x, y)| x != y)?
        * 8;
    a[word..word + 8]
        .iter()
        .zip(&b[word..word + 8])
        .rposition(|(x, y)| x != y)
        .map(|i| word + i)
}


#[inline(never)]
pub fn old_span(a: &[u8], b: &[u8]) -> Option<(usize, usize)> {
    let first = first_difference(a, b)?;
    let last = last_difference(a, b).unwrap_or(first);
    Some((first, last))
}

}
use baseline::old_span;
// Fixed-size equality compiles to two SIMD comparisons on Windows x64.
// Only the first or last unequal chunk needs a byte-wise search.
const DIFF_CHUNK: usize = 32;

/// Where `a` and `b` first differ within their common length.
fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    let len = a.len().min(b.len());
    let (a_chunks, a_tail) = a[..len].as_chunks::<DIFF_CHUNK>();
    let (b_chunks, b_tail) = b[..len].as_chunks::<DIFF_CHUNK>();
    let (offset, a, b) = match a_chunks.iter().zip(b_chunks).position(|(x, y)| x != y) {
        Some(i) => (i * DIFF_CHUNK, &a_chunks[i][..], &b_chunks[i][..]),
        None => (len - a_tail.len(), a_tail, b_tail),
    };
    a.iter().zip(b).position(|(x, y)| x != y).map(|i| offset + i)
}

/// Where `a` and `b` last differ within their common length.
fn last_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    let len = a.len().min(b.len());
    let (a_chunks, a_tail) = a[..len].as_chunks::<DIFF_CHUNK>();
    let (b_chunks, b_tail) = b[..len].as_chunks::<DIFF_CHUNK>();
    if let Some(i) = a_tail.iter().zip(b_tail).rposition(|(x, y)| x != y) {
        return Some(len - a_tail.len() + i);
    }
    let chunk = a_chunks.iter().zip(b_chunks).rposition(|(x, y)| x != y)?;
    a_chunks[chunk]
        .iter()
        .zip(&b_chunks[chunk])
        .rposition(|(x, y)| x != y)
        .map(|i| chunk * DIFF_CHUNK + i)
}

#[inline(never)]
pub fn new_span(a: &[u8], b: &[u8]) -> Option<(usize, usize)> {
    let first = first_difference(a, b)?;
    let last = last_difference(a, b).unwrap_or(first);
    Some((first, last))
}
use std::hint::black_box;
use std::time::Instant;
fn main() {
    let mut s = 0x12345678_u32;
    let len = 64 * 1024;
    let buf: Vec<u8> = (0..(len + 11) * 128).map(|_| {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s as u8
    }).collect();
    type Search = fn(&[u8], &[u8]) -> Option<(usize, usize)>;
    let funcs: &[(&str, Search)] = &[
        ("base", old_span), ("patched", new_span),
    ];
    for (name, at, len) in [
        ("equal64k", &[][..], len),
        ("mid64k", &[len/2+3], len),
        ("head64k", &[8], len),
        ("tail64k", &[len-3], len),
        ("sparse64k", &[len/3+1, 2*len/3+3], len),
        ("edges64k", &[3, len-5], len),
        ("equal64", &[], 64),
        ("mid64", &[33], 64),
        ("edges64", &[0, 62], 64),
        ("equal13", &[], 13),
        ("ends13", &[1, 12], 13),
    ] {
        let stride = len + 11;
        let a = &buf[..stride * 128];
        let mut b = a.to_vec();
        for page in b.chunks_exact_mut(stride) {
            for &off in at { page[off] ^= 0x80; }
        }
        print!("{name:>9}");
        for (label, fun) in funcs {
            let runs = if len >= 1024 { 12 } else { 2000 };
            for (a, b) in a.chunks_exact(stride).zip(b.chunks_exact(stride)) {
                assert_eq!(fun(&a[..len], &b[..len]), at.first().copied().zip(at.last().copied()));
            }
            let mut elapsed = [0.0f64; 9];
            for dt in &mut elapsed {
                let start = Instant::now();
                for _ in 0..runs {
                    for (a, b) in a.chunks_exact(stride).zip(b.chunks_exact(stride)) {
                        black_box(fun(black_box(&a[..len]), black_box(&b[..len])));
                    }
                }
                *dt = start.elapsed().as_secs_f64() * 1e9 / (runs * 128) as f64;
            }
            elapsed.sort_by(f64::total_cmp);
            print!(" {label}={:.1}", elapsed[4]);
        }
        println!();
    }
}
