// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

//! Helpers for emitting Rapid code.


/// Return rapid variable declaration for given type and name
pub fn rapid_var_declaration(env: &GlobalEnv, ty: &Type, id: &string) -> String{
    use Type::*;
    match ty{//TODO: see which types to match for const
        Primitive(p) => match p {
            U8 | U64 | U128 | Num | Address | Bool => format!("Int {}", id),
            _ => panic!("unexpected type")
        },
        Vector(et) => match et{
            U8 | U64 | U128 | Num | Address | Bool => format!("Int[] {}", id),
            _ => panic!("unsupported non-integer based vectors")
        }
        // Reference(_, bt) => format!("$Mutation ({})", boogie_type(env, bt)),
        _ => {
            format!("<<unsupported: {:?}>>", ty)
        }
    }
}

/// Returns the suffix to specialize a name for the given type instance.
pub fn rapid_type_suffix(env: &GlobalEnv, ty: &Type) -> String {
    use PrimitiveType::*;
    use Type::*;
    match ty {
        Primitive(p) => match p {
            U8 => "u8".to_string(),
            U64 => "u64".to_string(),
            U128 => "u128".to_string(),
            Num => "num".to_string(),
            Bool => "bool".to_string(),
            _ => format!("<<unsupported {:?}>>", ty),
        },
        Vector(et) => "vec".to_string(),
        Struct(..) | Fun(..) | Tuple(..) | TypeDomain(..) | ResourceDomain(..) | Error | Var(..)
        | Reference(..) => format!("<<unsupported {:?}>>", ty),
    }
}