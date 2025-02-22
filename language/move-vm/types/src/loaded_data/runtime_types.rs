// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use move_binary_format::{
    errors::{PartialVMError, PartialVMResult},
    file_format::{AbilitySet, StructDefinitionIndex, StructTypeParameter},
};
use move_core_types::{
    gas_algebra::AbstractMemorySize, identifier::Identifier, language_storage::ModuleId,
    vm_status::StatusCode,
};

pub const TYPE_DEPTH_MAX: usize = 256;

#[derive(Debug, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructType {
    pub fields: Vec<Type>,
    pub field_names: Vec<Identifier>,
    pub abilities: AbilitySet,
    pub type_parameters: Vec<StructTypeParameter>,
    pub name: Identifier,
    pub module: ModuleId,
    pub struct_def: StructDefinitionIndex,
}

impl StructType {
    pub fn type_param_constraints(&self) -> impl ExactSizeIterator<Item = &AbilitySet> {
        self.type_parameters.iter().map(|param| &param.constraints)
    }
}

#[derive(Debug, Copy, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CachedStructIndex(pub usize);

#[derive(Debug, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Type {
    Bool,
    U8,
    U64,
    U128,
    Address,
    Signer,
    Vector(Box<Type>),
    Struct(CachedStructIndex),
    StructInstantiation(CachedStructIndex, Vec<Type>),
    Reference(Box<Type>),
    MutableReference(Box<Type>),
    TyParam(usize),
    U16,
    U32,
    U256,
}

impl Type {
    fn clone_impl(&self, depth: usize) -> PartialVMResult<Type> {
        self.apply_subst(|idx, _| Ok(Type::TyParam(idx)), depth)
    }

    fn apply_subst<F>(&self, subst: F, depth: usize) -> PartialVMResult<Type>
    where
        F: Fn(usize, usize) -> PartialVMResult<Type> + Copy,
    {
        if depth > TYPE_DEPTH_MAX {
            return Err(PartialVMError::new(StatusCode::VM_MAX_TYPE_DEPTH_REACHED));
        }
        let res = match self {
            Type::TyParam(idx) => subst(*idx, depth)?,
            Type::Bool => Type::Bool,
            Type::U8 => Type::U8,
            Type::U16 => Type::U16,
            Type::U32 => Type::U32,
            Type::U64 => Type::U64,
            Type::U128 => Type::U128,
            Type::U256 => Type::U256,
            Type::Address => Type::Address,
            Type::Signer => Type::Signer,
            Type::Vector(ty) => Type::Vector(Box::new(ty.apply_subst(subst, depth + 1)?)),
            Type::Reference(ty) => Type::Reference(Box::new(ty.apply_subst(subst, depth + 1)?)),
            Type::MutableReference(ty) => {
                Type::MutableReference(Box::new(ty.apply_subst(subst, depth + 1)?))
            }
            Type::Struct(def_idx) => Type::Struct(*def_idx),
            Type::StructInstantiation(def_idx, instantiation) => {
                let mut inst = vec![];
                for ty in instantiation {
                    inst.push(ty.apply_subst(subst, depth + 1)?)
                }
                Type::StructInstantiation(*def_idx, inst)
            }
        };
        Ok(res)
    }

    pub fn subst(&self, ty_args: &[Type]) -> PartialVMResult<Type> {
        self.apply_subst(
            |idx, depth| match ty_args.get(idx) {
                Some(ty) => ty.clone_impl(depth),
                None => Err(
                    PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                        .with_message(format!(
                            "type substitution failed: index out of bounds -- len {} got {}",
                            ty_args.len(),
                            idx
                        )),
                ),
            },
            1,
        )
    }

    #[allow(deprecated)]
    const LEGACY_BASE_MEMORY_SIZE: AbstractMemorySize = AbstractMemorySize::new(1);

    /// Returns the abstract memory size the data structure occupies.
    ///
    /// This kept only for legacy reasons.
    /// New applications should not use this.
    pub fn size(&self) -> AbstractMemorySize {
        use Type::*;

        match self {
            TyParam(_) | Bool | U8 | U16 | U32 | U64 | U128 | U256 | Address | Signer => {
                Self::LEGACY_BASE_MEMORY_SIZE
            }
            Vector(ty) | Reference(ty) | MutableReference(ty) => {
                Self::LEGACY_BASE_MEMORY_SIZE + ty.size()
            }
            Struct(_) => Self::LEGACY_BASE_MEMORY_SIZE,
            StructInstantiation(_, tys) => tys
                .iter()
                .fold(Self::LEGACY_BASE_MEMORY_SIZE, |acc, ty| acc + ty.size()),
        }
    }
}
