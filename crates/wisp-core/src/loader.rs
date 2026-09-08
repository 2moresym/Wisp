//! Explicit Windows loader lifecycle state machine.
//!
//! This layer deliberately separates policy/order from PE byte parsing. A
//! future loader can attach real callbacks and mapped modules without ever
//! being able to execute an entry point before dependencies, DLL init, CRT,
//! and TLS have reached their required states.

use std::fmt;

use crate::ModuleGraphError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoaderPhase {
    Validate,
    Map,
    Relocate,
    ResolveImports,
    InitializeDependencies,
    InitializeMain,
    InitializeCrt,
    InitializeTls,
    Enter,
    Running,
    Exited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoaderState {
    phase: LoaderPhase,
}

impl LoaderState {
    pub const fn new() -> Self { Self { phase: LoaderPhase::Validate } }

    #[inline]
    pub const fn phase(&self) -> LoaderPhase { self.phase }

    /// Advance exactly one legal phase. This makes accidental out-of-order
    /// loader execution a construction-time error in the runtime layer.
    pub fn advance(&mut self, next: LoaderPhase) -> Result<(), LoaderError> {
        let expected = match self.phase {
            LoaderPhase::Validate => LoaderPhase::Map,
            LoaderPhase::Map => LoaderPhase::Relocate,
            LoaderPhase::Relocate => LoaderPhase::ResolveImports,
            LoaderPhase::ResolveImports => LoaderPhase::InitializeDependencies,
            LoaderPhase::InitializeDependencies => LoaderPhase::InitializeMain,
            LoaderPhase::InitializeMain => LoaderPhase::InitializeCrt,
            LoaderPhase::InitializeCrt => LoaderPhase::InitializeTls,
            LoaderPhase::InitializeTls => LoaderPhase::Enter,
            LoaderPhase::Enter => LoaderPhase::Running,
            LoaderPhase::Running => LoaderPhase::Exited,
            LoaderPhase::Exited => return Err(LoaderError::AlreadyExited),
        };
        if next != expected {
            return Err(LoaderError::InvalidTransition { from: self.phase, to: next, expected });
        }
        self.phase = next;
        Ok(())
    }
}

impl Default for LoaderState {
    fn default() -> Self { Self::new() }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoaderError {
    InvalidTransition { from: LoaderPhase, to: LoaderPhase, expected: LoaderPhase },
    MissingDependency(String),
    DependencyCycle(String),
    AlreadyExited,
}

impl From<ModuleGraphError> for LoaderError {
    fn from(value: ModuleGraphError) -> Self {
        match value {
            ModuleGraphError::Missing(name) => Self::MissingDependency(name),
            ModuleGraphError::Cycle(name) => Self::DependencyCycle(name),
        }
    }
}

impl fmt::Display for LoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition { from, to, expected } => {
                write!(f, "invalid loader transition {from:?} -> {to:?}; expected {expected:?}")
            }
            Self::MissingDependency(name) => write!(f, "missing dependency: {name}"),
            Self::DependencyCycle(name) => write!(f, "dependency cycle: {name}"),
            Self::AlreadyExited => f.write_str("loader already exited"),
        }
    }
}

impl std::error::Error for LoaderError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_loader_order_is_enforced() {
        let phases = [
            LoaderPhase::Validate,
            LoaderPhase::Map,
            LoaderPhase::Relocate,
            LoaderPhase::ResolveImports,
            LoaderPhase::InitializeDependencies,
            LoaderPhase::InitializeMain,
            LoaderPhase::InitializeCrt,
            LoaderPhase::InitializeTls,
            LoaderPhase::Enter,
            LoaderPhase::Running,
            LoaderPhase::Exited,
        ];
        let mut state = LoaderState::new();
        for phase in phases.into_iter().skip(1) {
            state.advance(phase).unwrap();
        }
        assert_eq!(state.phase(), LoaderPhase::Exited);
    }

    #[test]
    fn invalid_transition_is_rejected() {
        let mut state = LoaderState::new();
        assert!(matches!(
            state.advance(LoaderPhase::Enter),
            Err(LoaderError::InvalidTransition { .. })
        ));
    }
}
