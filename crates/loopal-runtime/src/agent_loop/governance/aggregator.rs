use super::traits::Verdict;

#[derive(Debug)]
pub enum AggregatedVerdict {
    Continue,
    Warnings(Vec<String>),
    Abort {
        reason: String,
        feedback_to_model: String,
    },
}

pub trait VerdictAggregator: Send + Sync {
    fn aggregate(&self, verdicts: Vec<Verdict>) -> AggregatedVerdict;
}

pub struct FirstDenyWins;

impl VerdictAggregator for FirstDenyWins {
    fn aggregate(&self, verdicts: Vec<Verdict>) -> AggregatedVerdict {
        let mut warnings = Vec::new();
        for v in verdicts {
            match v {
                Verdict::Continue => {}
                Verdict::InjectWarning(msg) => warnings.push(msg),
                Verdict::AbortTurn {
                    reason,
                    feedback_to_model,
                } => {
                    return AggregatedVerdict::Abort {
                        reason,
                        feedback_to_model,
                    };
                }
            }
        }
        if warnings.is_empty() {
            AggregatedVerdict::Continue
        } else {
            AggregatedVerdict::Warnings(warnings)
        }
    }
}
