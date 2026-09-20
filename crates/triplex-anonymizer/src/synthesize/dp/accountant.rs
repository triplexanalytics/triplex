//! A privacy-budget accountant using sequential composition: the epsilons of every mechanism run
//! on the same data add up, and the total is never allowed to exceed the budget.

use serde::Serialize;
use triplex_core::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Spend {
    pub mechanism: String,
    pub sensitivity: u64,
    /// The epsilon this mechanism was allotted.
    pub epsilon_allotted: f64,
    /// The epsilon its noise is actually calibrated to (rounded down; never more than allotted).
    pub epsilon_used: f64,
}

#[derive(Debug)]
pub struct Accountant {
    total: f64,
    spent: f64,
    ledger: Vec<Spend>,
}

impl Accountant {
    pub fn new(total: f64) -> Result<Accountant> {
        if !(total.is_finite() && total > 0.0) {
            return Err(Error::Validation(
                "the privacy budget (epsilon) must be a positive, finite number".into(),
            ));
        }
        Ok(Accountant {
            total,
            spent: 0.0,
            ledger: Vec::new(),
        })
    }

    /// Records a mechanism's spend, refusing if it would take the total past the budget.
    pub fn spend(
        &mut self,
        mechanism: &str,
        sensitivity: u64,
        epsilon_allotted: f64,
        epsilon_used: f64,
    ) -> Result<()> {
        if !(epsilon_used.is_finite() && epsilon_used > 0.0) || epsilon_used > epsilon_allotted {
            return Err(Error::Validation(format!(
                "mechanism \"{mechanism}\" has an invalid epsilon"
            )));
        }
        // a whisker of tolerance for the float sum of many equal shares
        if self.spent + epsilon_used > self.total * (1.0 + 1e-9) {
            return Err(Error::Validation(format!(
                "spending {epsilon_used} on \"{mechanism}\" would exceed the privacy budget {} (already spent {})",
                self.total, self.spent
            )));
        }
        self.spent += epsilon_used;
        self.ledger.push(Spend {
            mechanism: mechanism.to_string(),
            sensitivity,
            epsilon_allotted,
            epsilon_used,
        });
        Ok(())
    }

    pub fn total(&self) -> f64 {
        self.total
    }

    pub fn spent(&self) -> f64 {
        self.spent
    }

    pub fn ledger(&self) -> &[Spend] {
        &self.ledger
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_total_is_reported() {
        assert_eq!(Accountant::new(2.5).unwrap().total(), 2.5);
    }

    #[test]
    fn spends_add_up_and_are_recorded() {
        let mut a = Accountant::new(1.0).unwrap();
        a.spend("hist:a", 2, 0.25, 0.25).unwrap();
        a.spend("hist:b", 2, 0.25, 0.25).unwrap();
        assert!((a.spent() - 0.5).abs() < 1e-12);
        assert_eq!(a.ledger().len(), 2);
        assert_eq!(a.ledger()[0].mechanism, "hist:a");
    }

    #[test]
    fn overspending_is_refused_and_leaves_the_spend_unchanged() {
        let mut a = Accountant::new(1.0).unwrap();
        a.spend("first", 2, 0.75, 0.75).unwrap();
        assert!(a.spend("second", 2, 0.5, 0.5).is_err());
        assert!((a.spent() - 0.75).abs() < 1e-12);
        assert_eq!(a.ledger().len(), 1);
    }

    #[test]
    fn spending_exactly_the_budget_is_allowed() {
        let mut a = Accountant::new(1.0).unwrap();
        for i in 0..10 {
            a.spend(&format!("m{i}"), 2, 0.1, 0.1).unwrap();
        }
        assert!(a.spent() <= 1.0 * (1.0 + 1e-9));
        assert!(a.spend("one more", 2, 0.01, 0.01).is_err());
    }

    #[test]
    fn invalid_budgets_and_spends_are_refused() {
        assert!(Accountant::new(0.0).is_err());
        assert!(Accountant::new(-1.0).is_err());
        assert!(Accountant::new(f64::NAN).is_err());
        assert!(Accountant::new(f64::INFINITY).is_err());
        let mut a = Accountant::new(1.0).unwrap();
        assert!(a.spend("x", 2, 0.5, 0.0).is_err(), "zero epsilon used");
        assert!(
            a.spend("x", 2, 0.5, 0.6).is_err(),
            "used more than allotted"
        );
    }
}
