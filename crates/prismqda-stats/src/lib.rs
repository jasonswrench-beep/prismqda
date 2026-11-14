//! The statistics PrismQDA reports, written from published definitions and checked against R.
//!
//! This crate has no dependency on the rest of PrismQDA: it takes numbers in and gives numbers out, so
//! every function can be tested on its own against reference values (`tests/reference`).

pub mod adjust;
pub mod agreement;
pub mod anova;
pub mod bootstrap;
pub mod charts;
pub mod contingency;
pub mod curve;
pub mod correlation;
pub mod describe;
pub mod discrete;
pub mod dist;
pub mod factorial;
pub mod formula;
pub mod kolmogorov;
pub mod logistic;
pub mod nonparametric;
pub mod noncentral;
pub mod normality;
pub mod rank;
pub mod regression;
pub mod rng;
pub mod scale;
pub mod studentized;
pub mod transform;
pub mod ttest;

pub use agreement::{
    agreement, bootstrap_interval, bootstrap_intervals, code_agreement, confusion_table, Agreement, CodeAgreement, Coefficient, Interval, Ratings,
};
pub use rng::SplitMix64;
