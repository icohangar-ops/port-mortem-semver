//! Prints how long each hardened regex takes to compile.
//!
//! `cargo run --release --example re_timing`

use std::time::Instant;

use node_semver_rs::re::{safe_re, safe_src, t};

fn main() {
    let mut rows: Vec<(u128, usize, usize)> = Vec::new();
    for i in 0..t::COUNT {
        let start = Instant::now();
        let r = safe_re(i);
        // Include one match so the cost of building the lazy DFA is counted.
        r.is_match("1.2.3-alpha.1+build.5");
        let micros = start.elapsed().as_micros();
        rows.push((micros, i, r.as_str().len()));
    }

    rows.sort_by_key(|r| std::cmp::Reverse(r.0));
    let total: u128 = rows.iter().map(|r| r.0).sum();

    for (micros, i, len) in rows.iter().take(12) {
        println!(
            "{:>8}us  token {:>2}  pattern {} chars  {}",
            micros,
            i,
            len,
            &safe_src(*i).chars().take(60).collect::<String>()
        );
    }
    println!("total {}us for {} patterns", total, t::COUNT);
}
