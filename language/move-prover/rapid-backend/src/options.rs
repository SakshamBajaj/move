// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use move_command_line_common::env::{read_env_var};
use serde::{Deserialize, Serialize};

/// Default flags passed to rapid. Additional flags will be added to this via the -B option.
const DEFAULT_RAPID_FLAGS: &[&str] = &[];

//TODO: Fill these correctly
// const MIN_RAPID_VERSION: &str = "1.0";
// const MIN_VAMPIRE_VERSION: &str = "0.0";


/// rapid options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RapidOptions {
    /// Path to the rapid executable.
    pub rapid_exe: String,
    /// Path to the vampire executable.
    pub vampire_exe: String,
    /// List of flags to pass on to rapid.
    pub rapid_flags: Vec<String>,
    /// A seed for the prover.
    pub random_seed: usize,
}

impl Default for RapidOptions {
    fn default() -> Self {
        Self {
            rapid_exe: read_env_var("RAPID_EXE"),
            vampire_exe: read_env_var("VAMPIRE_EXE"),
            rapid_flags: vec![],
            random_seed: 1
        }
    }
}

impl RapidOptions {
    
    /// Returns command line to call rapid.
    pub fn get_rapid_command(&self) -> anyhow::Result<Vec<String>> {
        let mut result = vec![self.rapid_exe.clone()];
        let mut add = |sl: &[&str]| result.extend(sl.iter().map(|s| (*s).to_string()));
        add(DEFAULT_RAPID_FLAGS);
        Ok(result)
    }
}
