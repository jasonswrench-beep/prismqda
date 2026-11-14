//! Ranks and ties, the base of every rank-based test.

/// The ranks of the values, with tied values sharing the average of the ranks they cover (1 is the smallest).
pub fn rank_average(x: &[f64]) -> Vec<f64> {
    let mut order: Vec<usize> = (0..x.len()).collect();
    order.sort_by(|&a, &b| x[a].total_cmp(&x[b]));
    let mut ranks = vec![0.0; x.len()];
    let mut i = 0;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && x[order[j + 1]] == x[order[i]] {
            j += 1;
        }
        // Positions i..=j are tied; their ranks are i + 1 .. j + 1.
        let average = (i + j) as f64 / 2.0 + 1.0;
        for &k in &order[i..=j] {
            ranks[k] = average;
        }
        i = j + 1;
    }
    ranks
}

/// The sizes of the groups of equal values, for groups of two or more.
pub fn tie_sizes(x: &[f64]) -> Vec<usize> {
    let mut sorted: Vec<f64> = x.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mut out = Vec::new();
    let mut i = 0;
    while i < sorted.len() {
        let mut j = i;
        while j + 1 < sorted.len() && sorted[j + 1] == sorted[i] {
            j += 1;
        }
        if j > i {
            out.push(j - i + 1);
        }
        i = j + 1;
    }
    out
}
