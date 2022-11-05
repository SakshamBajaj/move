// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use itertools::Itertools;
use move_command_line_common::env::{read_bool_env_var, read_env_var};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::process::Command;

/// Default flags passed to boogie. Additional flags will be added to this via the -B option.
const DEFAULT_RAPID_FLAGS: &[&str] = &[];

//TODO: Fill these correctly
const MIN_RAPID_VERSION: &str = "1.0";
const MIN_VAMPIRE_VERSION: &str = "0.0";


/// Boogie options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RapidOptions {
    /// Path to the boogie executable.
    pub rapid_exe: String,
    /// Path to the z3 executable.
    pub vampire_exe: String,
    /// List of flags to pass on to boogie.
    pub rapid_flags: Vec<String>,
    /// A seed for the prover.
    pub random_seed: usize,
}

impl Default for BoogieOptions {
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
    pub fn get_rapid_command(&self, boogie_file: &str) -> anyhow::Result<Vec<String>> {
        let mut add = |sl: &[&str]| vec![self.boogie_exe.clone()].extend(sl.iter().map(|s| (*s).to_string()));
        add(DEFAULT_RAPID_FLAGS);
        Ok(result)
    }
}
