// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

//! This module translates the bytecode of a module to Rapid Spec.

use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
#[allow(unused_imports)]
use log::{debug, info, log, warn, Level};

use move_model::{
    code_writer::CodeWriter,
    emit, emitln,
    model::{GlobalEnv, QualifiedInstId, StructEnv, StructId},
    pragmas::{ADDITION_OVERFLOW_UNCHECKED_PRAGMA, SEED_PRAGMA, TIMEOUT_PRAGMA},
    ty::{PrimitiveType, Type},
};
use move_stackless_bytecode::{
    function_target::FunctionTarget,
    function_target_pipeline::{FunctionTargetsHolder, VerificationFlavor},
    mono_analysis,
    stackless_bytecode::{BorrowEdge, BorrowNode, Bytecode, Constant, HavocKind, Operation},
};
use crate::{
    rapid_helpers::{
        rapid_var_declaration, rapid_type_suffix
    },
    options::RapidOptions
};
use codespan::LineIndex;
use move_model::{
    ast::{TempIndex, TraceKind},
    model::{Loc, NodeId},
    ty::{TypeDisplayContext, BOOL_TYPE},
};
use move_stackless_bytecode::{
    function_target_pipeline::FunctionVariant,
    stackless_bytecode::{AbortAction, PropKind},
};

pub struct RapidTranslator<'env> {
    env: &'env GlobalEnv,
    options: &'env RapidOptions,
    writer: &'env CodeWriter,
    targets: &'env FunctionTargetsHolder,
}

pub struct FunctionTranslator<'env> {
    parent: &'env RapidTranslator<'env>,
    fun_target: &'env FunctionTarget<'env>,
    type_inst: &'env [Type],
}

impl<'env> RapidTranslator<'env> {
    pub fn new(
        env: &'env GlobalEnv,
        options: &'env RapidOptions,
        targets: &'env FunctionTargetsHolder,
        writer: &'env CodeWriter,
    ) -> Self {
        Self {
            env,
            options,
            targets,
            writer,
        }
    }

    pub fn translate(&mut self) {
        let writer = self.writer;
        let env = self.env;

        let mono_info = mono_analysis::get_info(self.env);
        let empty = &BTreeSet::new();

        emitln!(
            writer,
            "\n\n//==================================\n// Begin Translation\n"
        );
        
        let mut translated_funs = BTreeSet::new();
        let mut verified_functions_count = 0;
        info!("generating verification conditions");
        for module_env in self.env.get_modules() {
            self.writer.set_location(&module_env.env.internal_loc());

            for ref fun_env in module_env.get_functions() {
                if fun_env.is_native_or_intrinsic() {
                    continue;
                }
                for (variant, ref fun_target) in self.targets.get_targets(fun_env) {
                    if variant.is_verified() {
                        verified_functions_count += 1;
                        // Always produce a verified functions with an empty instantiation such that
                        // there is at least one top-level entry points for a VC.
                        FunctionTranslator {
                            parent: self,
                            fun_target,
                            type_inst: &[],
                        }
                        .translate();

                        // There maybe more verification targets that needs to be produced as we
                        // defer the instantiation of verified functions to this stage
                        for type_inst in mono_info
                            .funs
                            .get(&(fun_target.func_env.get_qualified_id(), variant))
                            .unwrap_or(empty)
                        {
                            // Skip the none instantiation (i.e., each type parameter is
                            // instantiated to itself as a concrete type). This has the same
                            // effect as `type_inst: &[]` and is already captured above.
                            let is_none_inst = type_inst.iter().enumerate().all(
                                |(i, t)| matches!(t, Type::TypeParameter(idx) if *idx == i as u16),
                            );
                            if is_none_inst {
                                continue;
                            }

                            verified_functions_count += 1;
                            FunctionTranslator {
                                parent: self,
                                fun_target,
                                type_inst,
                            }
                            .translate();
                        }
                    } else {
                        // This variant is inlined, so translate for all type instantiations.
                        for type_inst in mono_info
                            .funs
                            .get(&(
                                fun_target.func_env.get_qualified_id(),
                                FunctionVariant::Baseline,
                            ))
                            .unwrap_or(empty)
                        {
                            let fun_name = "main".to_string();
                            if !translated_funs.insert(fun_name) {
                                continue;
                            }
                            FunctionTranslator {
                                parent: self,
                                fun_target,
                                type_inst,
                            }
                            .translate();
                        }
                    }
                }
            }
        }
        info!("{} verification conditions", verified_functions_count);
    }
    
}

// =================================================================================================
// Function Translation

impl<'env> FunctionTranslator<'env> {
    fn inst(&self, ty: &Type) -> Type {
        ty.instantiate(self.type_inst)
    }

    fn inst_slice(&self, tys: &[Type]) -> Vec<Type> {
        tys.iter().map(|ty| self.inst(ty)).collect()
    }

    fn get_local_type(&self, idx: TempIndex) -> Type {
        self.fun_target
            .get_local_type(idx)
            .instantiate(self.type_inst)
    }

    fn emit_main_function(self){
        let writer = self.parent.writer;
        emitln!(writer, "func main");
        writer.indent();
        self.generate_function_body();
        writer.unindent();
        emitln!(writer, "}");
    }
    /// Translates the given function. Only the main function for now.
    fn translate(self) {
        let writer = self.parent.writer;
        let fun_target = self.fun_target;
        let env = fun_target.global_env();
        self.emit_main_function();
    }


    /// Generates rapid main body.
    fn generate_function_body(&self) {
        let writer = self.parent.writer;
        let fun_target = self.fun_target;
        
        let env = fun_target.global_env();

        // Be sure to set back location to the whole function definition as a default.
        writer.set_location(&fun_target.get_loc().at_start());

        // Generate local variable declarations. They need to appear first in rapid.
        emitln!(writer, "// declare local variables");
        let num_args = fun_target.get_parameter_count();
        for i in num_args..fun_target.get_local_count() {
            let local_type = &self.get_local_type(i);
            emitln!(writer, &rapid_var_declaration(env, local_type, &i.to_string()));
        }
        // Generate declarations for renamed parameters.
        let proxied_parameters = self.get_mutable_parameters();
        for (idx, ty) in &proxied_parameters {
            emitln!(
                writer,
                &rapid_var_declaration(env, &ty.instantiate(self.type_inst), &idx.to_string())
            );
        }

        // Generate bytecode
        emitln!(writer, "\n// bytecode translation starts here");
        let mut last_tracked_loc = None;
        let code = fun_target.get_bytecode();
        for bytecode in code.iter() {
            self.translate_bytecode(&mut last_tracked_loc, bytecode);
        }

        // writer.unindent();
        emitln!(writer, "}");
    }

    fn get_mutable_parameters(&self) -> Vec<(TempIndex, Type)> {
        let fun_target = self.fun_target;
        (0..fun_target.get_parameter_count())
            .filter_map(|i| {
                if self.parameter_needs_to_be_mutable(fun_target, i) {
                    Some((i, fun_target.get_local_type(i).clone()))
                } else {
                    None
                }
            })
            .collect_vec()
    }

    /// Determines whether the parameter of a function needs to be mutable.
    /// Boogie does not allow to assign to procedure parameters. In some cases
    /// (e.g. for memory instrumentation, but also as a result of copy propagation),
    /// we may need to assign to parameters.
    fn parameter_needs_to_be_mutable(
        &self,
        _fun_target: &FunctionTarget<'_>,
        _idx: TempIndex,
    ) -> bool {
        // For now, we just always say true. This could be optimized because the actual (known
        // so far) sources for mutability are parameters which are used in WriteBack(LocalRoot(p))
        // position.
        true
    }

    fn translate_verify_entry_assumptions(&self, fun_target: &FunctionTarget<'_>) {
        let writer = self.parent.writer;
        emitln!(writer, "\n// verification entrypoint assumptions");

        // Prelude initialization
        emitln!(writer, "call $InitVerification();");

        // Assume reference parameters to be based on the Param(i) Location, ensuring
        // they are disjoint from all other references. This prevents aliasing and is justified as
        // follows:
        // - for mutual references, by their exclusive access in Move.
        // - for immutable references because we have eliminated them
        for i in 0..fun_target.get_parameter_count() {
            let ty = fun_target.get_local_type(i);
            if ty.is_reference() {
                emitln!(writer, "assume l#$Mutation($t{}) == $Param({});", i, i);
            }
        }
    }
}

// =================================================================================================
// Bytecode Translation

impl<'env> FunctionTranslator<'env> {
    /// Translates one bytecode instruction.
    fn translate_bytecode(
        &self,
        last_tracked_loc: &mut Option<(Loc, LineIndex)>,
        bytecode: &Bytecode,
    ) {
        use Bytecode::*;

        let writer = self.parent.writer;
        
        let options = self.parent.options;
        let fun_target = self.fun_target;
        let env = fun_target.global_env();

        // Set location of this code in the CodeWriter.
        let attr_id = bytecode.get_attr_id();
        let loc = fun_target.get_bytecode_loc(attr_id);
        writer.set_location(&loc);

        // Print location.
        emitln!(
            writer,
            "// {} {}",
            bytecode.display(fun_target, &BTreeMap::default()),
            loc.display(env)
        );

        // Print debug comments.
        if let Some(comment) = fun_target.get_debug_comment(attr_id) {
            if comment.starts_with("info: ") {
                // if the comment is annotated with "info: ", it should be displayed to the user
                emitln!(
                    writer,
                    "assume {{:print \"${}(){}\"}} true;",
                    &comment[..4],
                    &comment[4..]
                );
            } else {
                emitln!(writer, "// {}", comment);
            }
        }

        // Helper function to get a string for a local. TODO: Check if this is right
        let str_local = |idx: usize| format!("t{}", idx);

        // Translate the bytecode instruction.
        match bytecode {
            
            // //Rapid does not support assertions at arbitrary points in the code
            // Prop(id, kind, exp) => match kind {
            //     PropKind::Assert => {
            //         emit!(writer, "assert ");
            //         let info = fun_target
            //             .get_vc_info(*id)
            //             .map(|s| s.as_str())
            //             .unwrap_or("unknown assertion failed");
            //         emit!(
            //             writer,
            //             "{{:msg \"assert_failed{}: {}\"}}\n  ",
            //             self.loc_str(&loc),
            //             info
            //         );
                    
            //         emitln!(writer, ";");
            //     }
            //     PropKind::Assume => {
            //         emit!(writer, "assume ");
            //         spec_translator.translate(exp, self.type_inst);
            //         emitln!(writer, ";");
            //     }
            //     PropKind::Modifies => {
            //         let ty = &self.inst(&env.get_node_type(exp.node_id()));
            //         let (mid, sid, inst) = ty.require_struct();
            //         let memory = boogie_resource_memory_name(
            //             env,
            //             &mid.qualified_inst(sid, inst.to_owned()),
            //             &None,
            //         );
            //         let exists_str = boogie_temp(env, &BOOL_TYPE, 0);
            //         emitln!(writer, "havoc {};", exists_str);
            //         emitln!(writer, "if ({}) {{", exists_str);
            //         writer.with_indent(|| {
            //             let val_str = boogie_temp(env, ty, 0);
            //             emitln!(writer, "havoc {};", val_str);
            //             emit!(writer, "{} := $ResourceUpdate({}, ", memory, memory);
            //             spec_translator.translate(&exp.call_args()[0], self.type_inst);
            //             emitln!(writer, ", {});", val_str);
            //         });
            //         emitln!(writer, "} else {");
            //         writer.with_indent(|| {
            //             emit!(writer, "{} := $ResourceRemove({}, ", memory, memory);
            //             spec_translator.translate(&exp.call_args()[0], self.type_inst);
            //             emitln!(writer, ");");
            //         });
            //         emitln!(writer, "}");
            //     }
            // },
            
            Branch(_, then_target, else_target, idx) => emitln!(
                writer,
                "if (t{}) {{",
                idx
            ),
            Assign(_, dest, src, _) => {
                emitln!(writer, "t{} = {};", str_local(*dest), str_local(*src));
            }
            
            Load(_, dest, c) => {
                let value = match c {
                    Constant::Bool(true) => "1".to_string(),
                    Constant::Bool(false) => "0".to_string(),
                    Constant::U8(num) => num.to_string(),
                    Constant::U64(num) => num.to_string(),
                    Constant::U128(num) => num.to_string(),
                    Constant::U256(num) => num.to_string(),
                    _ => panic!("Cannot load type {}", c)
                };
                let dest_str = str_local(*dest);
                emitln!(writer, "{} = {};", dest_str, value);
            }
            Call(_, dests, oper, srcs, aa) => {
                use Operation::*;
                match oper {
                    FreezeRef => unreachable!(),
                    UnpackRef | UnpackRefDeep | PackRef | PackRefDeep => {
                        // No effect
                    }
                    OpaqueCallBegin(_, _, _) | OpaqueCallEnd(_, _, _) => {
                        // These are just markers.  There is no generated code.
                    }
                    
                    Not => {
                        let src = srcs[0];
                        let dest = dests[0];
                        emitln!(
                            writer,
                            "{} = !{};",
                            str_local(dest),
                            str_local(src)
                        );
                    }
                    Add => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        
                        emitln!(
                            writer,
                            "{} = {} + {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    Sub => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        emitln!(
                            writer,
                            "{} = {} - {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    Mul => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        
                        emitln!(
                            writer,
                            "{} = {} * {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    Div => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        emitln!(
                            writer,
                            "{} = {} / {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    Mod => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        emitln!(
                            writer,
                            "{} = {} % {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    Lt => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        emitln!(
                            writer,
                            "{} = {} < {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    Gt => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        emitln!(
                            writer,
                            "{} = {} > {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    Le => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        emitln!(
                            writer,
                            "{} = {} <= {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    Ge => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        emitln!(
                            writer,
                            "{} = {} >= {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    Or => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        emitln!(
                            writer,
                            "{} = {} || {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    And => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        emitln!(
                            writer,
                            "{} = {} && {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    Eq | Neq => {
                        let dest = dests[0];
                        let op1 = srcs[0];
                        let op2 = srcs[1];
                        emitln!(
                            writer,
                            "{} = {} == {};",
                            str_local(dest),
                            str_local(op1),
                            str_local(op2)
                        );
                    }
                    BitOr | BitAnd | Xor => {
                        env.error(&loc, "Unsupported operator");
                        emitln!(
                            writer,
                            "// bit operation not supported: {:?}\nassert false;",
                            bytecode
                        );
                    }
                    Uninit => {
                        env.error(&loc, "Unsupported operator");
                        emitln!(
                            writer,
                            "// uninit operation not supported: {:?}\nassert false;",
                            bytecode
                        );
                    }
                    Destroy => {}
                    CastU256 => unimplemented!(),
                    _ => unimplemented!(),
                }
                    // writer.indent();
                    *last_tracked_loc = None;
                
            }
            Nop(..) | Label(_, _) | Ret(_, _) | Abort(_, _)=> {}
            _ => unimplemented!("{:?} operation is unimplemented", bytecode)
        }
        emitln!(writer);
    }

    /// Compute temporaries needed for the compilation of given function. Because boogie does
    /// not allow to declare locals in arbitrary blocks, we need to compute them upfront.
    fn compute_needed_temps(&self) -> BTreeMap<String, (Type, usize)> {
        use Bytecode::*;
        use Operation::*;

        let fun_target = self.fun_target;
        let env = fun_target.global_env();

        let mut res: BTreeMap<String, (Type, usize)> = BTreeMap::new();
        let mut need = |ty: &Type, n: usize| {
            // Index by type suffix, which is more coarse grained then type.
            let ty = ty.skip_reference();
            let suffix = rapid_type_suffix(env, ty);
            let cnt = res.entry(suffix).or_insert_with(|| (ty.to_owned(), 0));
            (*cnt).1 = (*cnt).1.max(n);
        };
        
        res
    }
}
