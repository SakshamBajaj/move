// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use itertools::Itertools;
#[allow(unused_imports)]
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};

use move_model::{
    code_writer::CodeWriter,
    emit, emitln,
    model::GlobalEnv,
    ty::{PrimitiveType, Type},
};
use move_stackless_bytecode::mono_analysis;


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
struct TypeInfo {
    name: String,
    suffix: String,
    has_native_equality: bool,
}

