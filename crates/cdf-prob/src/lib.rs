//! Probabilistic arithmetic: operations on uncertain values that propagate confidence.

use cdf_common::ProbabilisticValue;

/// Arithmetic on probabilistic values with error propagation.
pub struct ProbArithmetic;

impl ProbArithmetic {
    /// Add two probabilistic values (independent).
    pub fn add(a: &ProbabilisticValue, b: &ProbabilisticValue) -> ProbabilisticValue {
        match (a, b) {
            (
                ProbabilisticValue::Normal { mean: m1, std_dev: s1 },
                ProbabilisticValue::Normal { mean: m2, std_dev: s2 },
            ) => ProbabilisticValue::Normal {
                mean: m1 + m2,
                std_dev: (s1 * s1 + s2 * s2).sqrt(),
            },
            _ => ProbabilisticValue::Samples {
                data: vec![a.expected() + b.expected()],
            },
        }
    }

    /// Multiply two probabilistic values (independent).
    pub fn mul(a: &ProbabilisticValue, b: &ProbabilisticValue) -> ProbabilisticValue {
        let e1 = a.expected();
        let e2 = b.expected();
        let v1 = a.variance().unwrap_or(0.0);
        let v2 = b.variance().unwrap_or(0.0);

        // Var(XY) ≈ E[X]^2 Var(Y) + E[Y]^2 Var(X) for independent
        ProbabilisticValue::Normal {
            mean: e1 * e2,
            std_dev: (e1 * e1 * v2 + e2 * e2 * v1).sqrt(),
        }
    }

    /// Confidence-weighted average of multiple values.
    pub fn weighted_avg(values: &[(ProbabilisticValue, f64)]) -> ProbabilisticValue {
        if values.is_empty() {
            return ProbabilisticValue::Normal { mean: 0.0, std_dev: 0.0 };
        }

        let total_weight: f64 = values.iter().map(|(_, w)| w).sum();
        let weighted_mean: f64 = values
            .iter()
            .map(|(v, w)| v.expected() * w)
            .sum::<f64>()
            / total_weight;

        let weighted_var: f64 = values
            .iter()
            .map(|(v, w)| v.variance().unwrap_or(0.0) * w * w)
            .sum::<f64>()
            / (total_weight * total_weight);

        ProbabilisticValue::Normal {
            mean: weighted_mean,
            std_dev: weighted_var.sqrt(),
        }
    }
}
