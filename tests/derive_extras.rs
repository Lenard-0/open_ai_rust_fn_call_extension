//! Edge-case tests for `#[derive(FunctionCall)]`:
//! - Explicit `description = "..."` field attribute (vs doc-comment fallback).
//! - Non-`Type::Path` field types (references) — exercises the early
//!   `return false` arm of `is_option_type`.
//! - Generic structs propagating generics into the impl block.
//! - Unit structs (zero fields).

#![allow(dead_code)]

#[allow(dead_code)]
mod mock {
    pub struct FunctionCall {
        pub name: String,
        pub description: Option<String>,
        pub parameters: Vec<FunctionParameter>,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub struct FunctionParameter {
        pub name: String,
        pub _type: FunctionType,
        pub description: Option<String>,
        pub required: bool,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub enum FunctionType {
        Object(Vec<FunctionParameter>),
        Enum(EnumValues),
        StringTy,
        NumberTy,
        BooleanTy,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub enum EnumValues {
        String(Vec<String>),
        Int(Vec<i64>),
        Float(Vec<f64>),
    }

    pub trait FunctionCallable {
        fn schema_type() -> FunctionType
        where
            Self: Sized;
        fn fn_schema() -> FunctionCall
        where
            Self: Sized,
        {
            panic!("fn_schema not implemented")
        }
    }

    impl FunctionCallable for bool {
        fn schema_type() -> FunctionType {
            FunctionType::BooleanTy
        }
    }
    impl FunctionCallable for i64 {
        fn schema_type() -> FunctionType {
            FunctionType::NumberTy
        }
    }
    impl FunctionCallable for String {
        fn schema_type() -> FunctionType {
            FunctionType::StringTy
        }
    }
    impl FunctionCallable for &'static str {
        fn schema_type() -> FunctionType {
            FunctionType::StringTy
        }
    }
    impl<T: FunctionCallable> FunctionCallable for Option<T> {
        fn schema_type() -> FunctionType {
            T::schema_type()
        }
    }
}

use mock::{FunctionCall, FunctionCallable, FunctionParameter, FunctionType};
use open_ai_rust_fn_call_extension::FunctionCall as Derive;

// ── description = "..." field attribute (priority over doc-comment) ──────────

#[derive(Derive)]
#[function_call(crate = "crate::mock")]
struct WithDescriptionAttr {
    /// this doc-comment should be overridden
    #[function_call(description = "explicit description override")]
    field: bool,
}

#[test]
fn description_attr_wins_over_doc_comment() {
    let fc = WithDescriptionAttr::fn_schema();
    assert_eq!(
        fc.parameters[0].description.as_deref(),
        Some("explicit description override")
    );
}

// ── non-Type::Path field types ───────────────────────────────────────────────

#[derive(Derive)]
#[function_call(crate = "crate::mock")]
struct WithReference {
    name: &'static str,
}

#[test]
fn reference_field_type_works() {
    let fc = WithReference::fn_schema();
    assert_eq!(fc.parameters.len(), 1);
    assert!(fc.parameters[0].required);
}

// ── generic struct propagates generics ───────────────────────────────────────

#[derive(Derive)]
#[function_call(crate = "crate::mock")]
struct Generic<T: FunctionCallable> {
    value: T,
}

#[test]
fn generic_struct_compiles_and_works() {
    let fc = <Generic<i64> as FunctionCallable>::fn_schema();
    assert_eq!(fc.parameters[0].name, "value");
    assert_eq!(fc.parameters[0]._type, FunctionType::NumberTy);
}

// ── unit struct (zero fields) ────────────────────────────────────────────────

#[derive(Derive)]
#[function_call(crate = "crate::mock")]
struct Marker;

#[test]
fn unit_struct_emits_empty_object() {
    let fc = Marker::fn_schema();
    assert_eq!(fc.parameters.len(), 0);
    assert!(matches!(Marker::schema_type(), FunctionType::Object(ref v) if v.is_empty()));
}

#[test]
fn unit_struct_static_methods() {
    let fc = Marker::fn_schema();
    assert_eq!(fc.name, "Marker");
    assert_eq!(fc.parameters.len(), 0);
}

// ── struct with no description at all ────────────────────────────────────────

#[derive(Derive)]
#[function_call(crate = "crate::mock")]
struct NoDesc {
    plain_field: bool,
}

#[test]
fn missing_description_is_none() {
    let fc = NoDesc::fn_schema();
    assert!(fc.parameters[0].description.is_none());
    assert!(fc.description.is_none());
}
