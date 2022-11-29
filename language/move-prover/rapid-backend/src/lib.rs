// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]
mod rapid_helpers;
pub mod options;
pub mod bytecode_translator;
pub mod spec_translator;

#[allow(unused_imports)]
use tera::{Context, Tera};
use serde::{Serialize, Deserialize};
use move_model::{
    code_writer::CodeWriter,
    emit, emitln,
    model::GlobalEnv
};
use crate::{
    options::{RapidOptions},
    spec_trn
};

const PRELUDE_TEMPLATE: &[u8] = include_bytes!("prelude/prelude.bpl");
/// Adds the prelude to the generated output.
pub fn add_prelude(
    env: &GlobalEnv,
    options: &RapidOptions,
    writer: &CodeWriter,
) -> anyhow::Result<()> {
    emit!(writer, "\n// ** Expanded prelude\n\n");
    let templ = |name: &'static str, cont: &[u8]| (name, String::from_utf8_lossy(cont).to_string());

    // Add the prelude template.
    let templates = vec![
        templ("prelude", PRELUDE_TEMPLATE),  
    ];

    let mut context = Context::new();
    context.insert("options", options);

    // TODO: we have defined {{std}} for adaptable resolution of stdlib addresses but
    //   not used it yet in the templates.
    let std_addr = format!("${}", env.get_stdlib_address());
    let ext_addr = format!("${}", env.get_extlib_address());
    context.insert("std", &std_addr);
    context.insert("Ext", &ext_addr);
    

    let mut tera = Tera::default();
    tera.add_raw_templates(templates)?;
    let expanded_content = tera.render("prelude", &context)?;
    emitln!(writer, &expanded_content);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
struct TypeInfo {
    name: String,
    suffix: String,
    has_native_equality: bool,
}

