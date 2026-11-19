//! `#[derive(FunctionCall)]` must expand to fully qualified schema-type paths,
//! so a caller only has to import the derive macro itself.
//!
//! `extern crate self as open_ai_rust;` makes `::open_ai_rust::...` resolve to
//! this test crate, whose `logoi::input::tool` module mirrors the real
//! `open_ai_rust` 1.x layout: the schema types live in `tool`, the trait in
//! `tool::raw_macro`. Nothing below is `use`d at the crate root except the
//! derive macro, so a stray unqualified `FunctionType` / `FunctionParameter` /
//! `FunctionCall` / `EnumValues` in the expansion fails to compile here.

#![allow(dead_code)]

extern crate self as open_ai_rust;

pub mod logoi {
    pub mod input {
        pub mod tool {
            #[derive(Debug, Clone, PartialEq)]
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
            }

            #[derive(Debug, Clone, PartialEq)]
            pub enum EnumValues {
                String(Vec<String>),
            }

            pub mod raw_macro {
                use super::{FunctionCall, FunctionType};

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

                impl FunctionCallable for f64 {
                    fn schema_type() -> FunctionType {
                        FunctionType::NumberTy
                    }
                }
                impl FunctionCallable for String {
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
        }
    }
}

/// A second layout: trait and types re-exported side by side at one module,
/// exercised through `crate = "..."` alone and through an explicit `types`.
pub mod flat {
    pub use crate::logoi::input::tool::raw_macro::FunctionCallable;
    pub use crate::logoi::input::tool::{
        EnumValues, FunctionCall, FunctionParameter, FunctionType,
    };
}

// The only import: the derive macro.
use open_ai_rust_fn_call_extension::FunctionCall;

/// A 2D point.
#[derive(FunctionCall)]
struct Point {
    /// X coordinate.
    x: f64,
    y: f64,
    label: Option<String>,
}

/// Traffic light colour.
#[derive(FunctionCall)]
enum Light {
    Red,
    Amber,
    Green,
}

#[derive(FunctionCall)]
#[function_call(crate = "crate::flat")]
struct ViaFlatCrate {
    name: String,
}

#[derive(FunctionCall)]
#[function_call(
    crate = "::open_ai_rust::logoi::input::tool::raw_macro",
    types = "crate::flat"
)]
enum ViaTypesOverride {
    On,
    Off,
}

fn schema<T: logoi::input::tool::raw_macro::FunctionCallable>() -> logoi::input::tool::FunctionCall
{
    T::fn_schema()
}

fn ty<T: logoi::input::tool::raw_macro::FunctionCallable>() -> logoi::input::tool::FunctionType {
    T::schema_type()
}

#[test]
fn struct_derive_needs_no_type_imports() {
    let s = schema::<Point>();
    assert_eq!(s.name, "Point");
    assert_eq!(s.description.as_deref(), Some("A 2D point."));
    let names: Vec<_> = s.parameters.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["x", "y", "label"]);
    assert_eq!(
        s.parameters[0].description.as_deref(),
        Some("X coordinate.")
    );
    assert_eq!(s.parameters[1].description, None);
    assert!(s.parameters[0].required);
    assert!(!s.parameters[2].required);
    assert_eq!(
        s.parameters[2]._type,
        logoi::input::tool::FunctionType::StringTy
    );

    match ty::<Point>() {
        logoi::input::tool::FunctionType::Object(params) => assert_eq!(params, s.parameters),
        other => panic!("expected Object, got {other:?}"),
    }
}

#[test]
fn unit_enum_derive_needs_no_type_imports() {
    use logoi::input::tool::{EnumValues, FunctionType};
    assert_eq!(
        ty::<Light>(),
        FunctionType::Enum(EnumValues::String(vec![
            "Red".into(),
            "Amber".into(),
            "Green".into()
        ]))
    );
    let s = schema::<Light>();
    assert_eq!(s.name, "Light");
    assert!(s.parameters.is_empty());
}

#[test]
fn crate_override_resolves_types_from_same_module() {
    assert_eq!(schema::<ViaFlatCrate>().parameters[0].name, "name");
}

#[test]
fn types_override_is_honoured() {
    use logoi::input::tool::{EnumValues, FunctionType};
    assert_eq!(
        ty::<ViaTypesOverride>(),
        FunctionType::Enum(EnumValues::String(vec!["On".into(), "Off".into()]))
    );
}
