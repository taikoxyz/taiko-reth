//! Abstractions for groups of tests.

use crate::{
    case::{Case, Cases},
    result::assert_tests_pass,
};
use reth_primitives::TransactionSigned;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// A collection of tests.
pub trait Suite {
    /// The type of test cases in this suite.
    type Case: Case;

    /// The name of the test suite used to locate the individual test cases.
    ///
    /// # Example
    ///
    /// - `GeneralStateTests`
    /// - `BlockchainTests/InvalidBlocks`
    /// - `BlockchainTests/TransitionTests`
    fn suite_name(&self) -> String;

    /// Load the cases
    fn load(&self) -> (PathBuf, Cases<Self::Case>);

    /// Load an run each contained test case.
    ///
    /// # Note
    ///
    /// This recursively finds every test description in the resulting path.
    fn run(&self) {
        // Run the test cases and collect the results
        let (suite_path, cases) = self.load();
        let results = cases.run();

        // Assert that all tests in the suite pass
        assert_tests_pass(&self.suite_name(), suite_path.as_path(), &results);
    }

    fn run_l2<TX>(&self, generate_tx: TX)
    where
        TX: Fn() -> Vec<TransactionSigned>,
    {
        // Run the test cases and collect the results
        let (suite_path, mut cases) = self.load();
        for (_, case) in cases.test_cases.iter_mut() {
            case.load_l2_payload(generate_tx())
        }
        let results = cases.run();

        // Assert that all tests in the suite pass
        assert_tests_pass(&self.suite_name(), suite_path.as_path(), &results);
    }
}

/// Recursively find all files with a given extension.
pub fn find_all_files_with_extension(path: &Path, extension: &str) -> Vec<PathBuf> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(extension))
        .map(DirEntry::into_path)
        .collect()
}
