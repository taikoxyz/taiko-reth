//! Test case definitions

use reth_primitives::TransactionSigned;

use crate::result::{CaseResult, Error};
use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};

/// A single test case, capable of loading a JSON description of itself and running it.
///
/// See <https://ethereum-tests.readthedocs.io/> for test specs.
pub trait Case: Debug + Sync + Sized {
    /// A description of the test.
    fn description(&self) -> String {
        "no description".to_string()
    }

    /// Load the test from the given file path.
    ///
    /// The file can be assumed to be a valid EF test case as described on <https://ethereum-tests.readthedocs.io/>.
    fn load(path: &Path) -> Result<Self, Error>;

    /// Load L1 test from the given file path.
    /// Generate L2 tx list from closure
    fn load_l2_payload(&mut self, l2_payload: Vec<TransactionSigned>);

    /// Run the test.
    fn run(&self) -> Result<(), Error>;

    /// Run the test.
    fn run_l2(&self) -> Result<(), Error>;
}

/// A container for multiple test cases.
#[derive(Debug)]
pub struct Cases<T> {
    /// The contained test cases and the path to each test.
    pub test_cases: Vec<(PathBuf, T)>,
}

impl<T: Case> Cases<T> {
    /// Run the contained test cases.
    pub fn run(&self) -> Vec<CaseResult> {
        self.test_cases.iter().map(|(path, case)| CaseResult::new(path, case, case.run())).collect()
    }

    /// Run the contained test cases.
    pub fn run_l2(&self) -> Vec<CaseResult> {
        self.test_cases
            .iter()
            .map(|(path, case)| CaseResult::new(path, case, case.run_l2()))
            .collect()
    }
}
