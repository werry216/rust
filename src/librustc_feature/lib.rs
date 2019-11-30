//! # Feature gating
//!
//! This module implements the gating necessary for preventing certain compiler
//! features from being used by default. This module will crawl a pre-expanded
//! AST to ensure that there are no features which are used that are not
//! enabled.
//!
//! Features are enabled in programs via the crate-level attributes of
//! `#![feature(...)]` with a comma-separated list of features.
//!
//! For the purpose of future feature-tracking, once code for detection of feature
//! gate usage is added, *do not remove it again* even once the feature
//! becomes stable.

mod accepted;
mod removed;
mod active;
mod builtin_attrs;

use std::fmt;
use std::num::NonZeroU32;
use syntax_pos::{Span, edition::Edition, symbol::Symbol};

#[derive(Clone, Copy)]
pub enum State {
    Accepted,
    Active { set: fn(&mut Features, Span) },
    Removed { reason: Option<&'static str> },
    Stabilized { reason: Option<&'static str> },
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            State::Accepted { .. } => write!(f, "accepted"),
            State::Active { .. } => write!(f, "active"),
            State::Removed { .. } => write!(f, "removed"),
            State::Stabilized { .. } => write!(f, "stabilized"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Feature {
    pub state: State,
    pub name: Symbol,
    pub since: &'static str,
    issue: Option<u32>,  // FIXME: once #58732 is done make this an Option<NonZeroU32>
    pub edition: Option<Edition>,
    description: &'static str,
}

impl Feature {
    // FIXME(Centril): privatize again.
    pub fn issue(&self) -> Option<NonZeroU32> {
        self.issue.and_then(|i| NonZeroU32::new(i))
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Stability {
    Unstable,
    // First argument is tracking issue link; second argument is an optional
    // help message, which defaults to "remove this attribute".
    Deprecated(&'static str, Option<&'static str>),
}

#[derive(Clone, Copy, Hash)]
pub enum UnstableFeatures {
    /// Hard errors for unstable features are active, as on beta/stable channels.
    Disallow,
    /// Allow features to be activated, as on nightly.
    Allow,
    /// Errors are bypassed for bootstrapping. This is required any time
    /// during the build that feature-related lints are set to warn or above
    /// because the build turns on warnings-as-errors and uses lots of unstable
    /// features. As a result, this is always required for building Rust itself.
    Cheat
}

impl UnstableFeatures {
    pub fn from_environment() -> UnstableFeatures {
        // `true` if this is a feature-staged build, i.e., on the beta or stable channel.
        let disable_unstable_features = option_env!("CFG_DISABLE_UNSTABLE_FEATURES").is_some();
        // `true` if we should enable unstable features for bootstrapping.
        let bootstrap = std::env::var("RUSTC_BOOTSTRAP").is_ok();
        match (disable_unstable_features, bootstrap) {
            (_, true) => UnstableFeatures::Cheat,
            (true, _) => UnstableFeatures::Disallow,
            (false, _) => UnstableFeatures::Allow
        }
    }

    pub fn is_nightly_build(&self) -> bool {
        match *self {
            UnstableFeatures::Allow | UnstableFeatures::Cheat => true,
            UnstableFeatures::Disallow => false,
        }
    }
}

pub use accepted::ACCEPTED_FEATURES;
pub use active::{ACTIVE_FEATURES, Features, INCOMPLETE_FEATURES};
pub use removed::{REMOVED_FEATURES, STABLE_REMOVED_FEATURES};
pub use builtin_attrs::{
    AttributeGate, AttributeTemplate, AttributeType, find_gated_cfg, GatedCfg,
    BuiltinAttribute, BUILTIN_ATTRIBUTES, BUILTIN_ATTRIBUTE_MAP,
    deprecated_attributes, is_builtin_attr_name,
};
