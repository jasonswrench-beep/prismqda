//! Adjusting p-values for the number of comparisons made (the same methods as R's `p.adjust`).

/// How p-values are adjusted when several are looked at together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Adjustment {
    /// No adjustment.
    None,
    /// Each p-value times the number of comparisons, at most 1.
    Bonferroni,
    /// Holm's step-down method: never less powerful than Bonferroni, and as safe.
    Holm,
    /// Benjamini and Hochberg's false discovery rate.
    BenjaminiHochberg,
}

/// The adjusted p-values, in the order given. Missing values (NaN) stay missing and are not counted as comparisons.
pub fn p_adjust(p: &[f64], method: Adjustment) -> Vec<f64> {
    let order: Vec<usize> = {
        let mut o: Vec<usize> = (0..p.len()).filter(|&i| !p[i].is_nan()).collect();
        o.sort_by(|&a, &b| p[a].total_cmp(&p[b]));
        o
    };
    let m = order.len();
    let mut out = vec![f64::NAN; p.len()];
    if m == 0 {
        return out;
    }
    let mf = m as f64;
    match method {
        Adjustment::None => {
            for &i in &order {
                out[i] = p[i];
            }
        }
        Adjustment::Bonferroni => {
            for &i in &order {
                out[i] = (p[i] * mf).min(1.0);
            }
        }
        Adjustment::Holm => {
            let mut running = 0.0f64;
            for (rank, &i) in order.iter().enumerate() {
                running = running.max(p[i] * (m - rank) as f64);
                out[i] = running.min(1.0);
            }
        }
        Adjustment::BenjaminiHochberg => {
            let mut running = 1.0f64;
            for (rank, &i) in order.iter().enumerate().rev() {
                running = running.min(p[i] * mf / (rank + 1) as f64);
                out[i] = running.min(1.0);
            }
        }
    }
    out
}
