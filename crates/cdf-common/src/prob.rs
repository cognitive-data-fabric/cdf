//! Probabilistic data types for uncertain/AI-generated values.

use serde::{Deserialize, Serialize};

/// A probabilistic value representing uncertainty explicitly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "params")]
pub enum ProbabilisticValue {
    /// Normal (Gaussian) distribution.
    Normal { mean: f64, std_dev: f64 },

    /// Confidence interval.
    ConfidenceInterval { lower: f64, upper: f64, confidence: f64 },

    /// Discrete probability distribution.
    Discrete { values: Vec<(String, f64)> },

    /// Beta distribution (bounded 0-1, good for probabilities).
    Beta { alpha: f64, beta: f64 },

    /// Raw distribution samples.
    Samples { data: Vec<f64> },
}

impl ProbabilisticValue {
    /// Get the expected value (mean).
    pub fn expected(&self) -> f64 {
        match self {
            ProbabilisticValue::Normal { mean, .. } => *mean,
            ProbabilisticValue::ConfidenceInterval { lower, upper, .. } => (lower + upper) / 2.0,
            ProbabilisticValue::Discrete { values } => {
                values.iter().map(|(_, p)| p).sum::<f64>() / values.len() as f64
            }
            ProbabilisticValue::Beta { alpha, beta } => alpha / (alpha + beta),
            ProbabilisticValue::Samples { data } => {
                if data.is_empty() {
                    0.0
                } else {
                    data.iter().sum::<f64>() / data.len() as f64
                }
            }
        }
    }

    /// Get the variance.
    pub fn variance(&self) -> Option<f64> {
        match self {
            ProbabilisticValue::Normal { std_dev, .. } => Some(std_dev * std_dev),
            ProbabilisticValue::ConfidenceInterval { lower, upper, confidence } => {
                // Approximate from confidence interval assuming normal
                let z = approximate_z(*confidence);
                let margin = (upper - lower) / 2.0;
                Some((margin / z).powi(2))
            }
            ProbabilisticValue::Discrete { values } => {
                let mean = values.iter().map(|(_, p)| p).sum::<f64>() / values.len() as f64;
                Some(
                    values
                        .iter()
                        .map(|(_, p)| (p - mean).powi(2))
                        .sum::<f64>()
                        / values.len() as f64,
                )
            }
            ProbabilisticValue::Beta { alpha, beta } => {
                Some((alpha * beta) / ((alpha + beta).powi(2) * (alpha + beta + 1.0)))
            }
            ProbabilisticValue::Samples { data } => {
                if data.len() < 2 {
                    return None;
                }
                let mean = data.iter().sum::<f64>() / data.len() as f64;
                Some(data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / data.len() as f64)
            }
        }
    }

    /// Check if confidence is above a threshold.
    pub fn is_confident(&self, min_confidence: f64) -> bool {
        match self {
            ProbabilisticValue::ConfidenceInterval { confidence, .. } => {
                *confidence >= min_confidence
            }
            ProbabilisticValue::Normal { std_dev, .. } => {
                // Coefficient of variation < threshold as proxy
                let mean = self.expected().abs();
                if mean == 0.0 {
                    *std_dev < 0.1
                } else {
                    (*std_dev / mean) < (1.0 - min_confidence)
                }
            }
            _ => true,
        }
    }

    /// Convert to a scalar point estimate.
    pub fn to_scalar(&self) -> f64 {
        self.expected()
    }
}

fn approximate_z(confidence: f64) -> f64 {
    // Rough approximations for common confidence levels
    match (confidence * 100.0).round() as i32 {
        50 => 0.674,
        80 => 1.282,
        90 => 1.645,
        95 => 1.96,
        99 => 2.576,
        _ => 1.96, // default to 95%
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_expected() {
        let p = ProbabilisticValue::Normal {
            mean: 10.0,
            std_dev: 2.0,
        };
        assert!((p.expected() - 10.0).abs() < 1e-10);
        assert!((p.variance().unwrap() - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_beta_expected() {
        let p = ProbabilisticValue::Beta {
            alpha: 2.0,
            beta: 3.0,
        };
        assert!((p.expected() - 0.4).abs() < 1e-10);
    }

    #[test]
    fn test_confidence_check() {
        let p = ProbabilisticValue::ConfidenceInterval {
            lower: 0.0,
            upper: 10.0,
            confidence: 0.95,
        };
        assert!(p.is_confident(0.90));
        assert!(!p.is_confident(0.99));
    }
}
