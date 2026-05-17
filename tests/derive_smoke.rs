//! Integration tests for `#[derive(FunctionCall)]`.
//! Validates against a local mock of the consumer-crate types so this crate
//! has no runtime dep on `open_ai_rust`.

#[allow(dead_code)]
mod mock {
    // ── types ────────────────────────────────────────────────────────────────

    #[derive(Debug, Clone)]
    pub struct FunctionCall {
        pub name: String,
        pub description: Option<String>,
        pub parameters: Vec<FunctionParameter>,
    }

    #[derive(Debug, Clone)]
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

    impl PartialEq for FunctionParameter {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name
                && self._type == other._type
                && self.description == other.description
                && self.required == other.required
        }
    }

    // ── FunctionCallable trait (slim: static methods only) ──────────────────

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

    // ── primitive impls ─────────────────────────────────────────────────────

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
    impl<T: FunctionCallable> FunctionCallable for Option<T> {
        fn schema_type() -> FunctionType {
            T::schema_type()
        }
    }
}

use mock::{EnumValues, FunctionCall, FunctionCallable, FunctionParameter, FunctionType};
use open_ai_rust_fn_call_extension::FunctionCall as FunctionCallDerive;

// ── Widget struct ─────────────────────────────────────────────────────────────

/// A widget.
#[derive(FunctionCallDerive)]
#[function_call(crate = "crate::mock")]
#[allow(dead_code)]
struct Widget {
    /// whether it is on
    on: bool,
    #[function_call(rename = "qty")]
    quantity: i64,
    /// optional label
    label: Option<String>,
    #[function_call(skip)]
    internal: String,
}

#[test]
fn widget_fn_schema() {
    let fc = Widget::fn_schema();
    assert_eq!(fc.name, "Widget");
    assert_eq!(fc.description.as_deref(), Some("A widget."));
    // 3 fields (internal skipped)
    assert_eq!(fc.parameters.len(), 3);

    assert_eq!(fc.parameters[0].name, "on");
    assert_eq!(
        fc.parameters[0].description.as_deref(),
        Some("whether it is on")
    );
    assert_eq!(fc.parameters[0]._type, FunctionType::BooleanTy);
    assert!(fc.parameters[0].required);

    assert_eq!(fc.parameters[1].name, "qty");
    assert_eq!(fc.parameters[1]._type, FunctionType::NumberTy);
    assert!(fc.parameters[1].required);

    // Option<String> → required: false
    assert_eq!(fc.parameters[2].name, "label");
    assert!(!fc.parameters[2].required);
}

#[test]
fn widget_schema_type_static() {
    let FunctionType::Object(params) = Widget::schema_type() else {
        panic!("expected Object")
    };
    assert_eq!(params.len(), 3);
    assert_eq!(params[0].name, "on");
    assert_eq!(params[1].name, "qty");
    assert_eq!(params[2].name, "label");
    assert!(!params[2].required);
}

// ── Unit enum ─────────────────────────────────────────────────────────────────

/// Light mode.
#[derive(FunctionCallDerive)]
#[function_call(crate = "crate::mock")]
#[allow(dead_code)]
enum Mode {
    On,
    Off,
}

#[test]
fn unit_enum_schema_type_static() {
    let FunctionType::Enum(EnumValues::String(variants)) = Mode::schema_type() else {
        panic!("expected Enum(String(...))")
    };
    assert_eq!(variants, vec!["On", "Off"]);
}

#[test]
fn unit_enum_fn_schema() {
    let fc = Mode::fn_schema();
    assert_eq!(fc.name, "Mode");
    assert_eq!(fc.description.as_deref(), Some("Light mode."));
    assert_eq!(fc.parameters.len(), 0);
}

// ── Tuple struct ──────────────────────────────────────────────────────────────

#[derive(FunctionCallDerive)]
#[function_call(crate = "crate::mock")]
#[allow(dead_code)]
struct Pair(i64, bool);

#[test]
fn tuple_struct_default_names() {
    let fc = Pair::fn_schema();
    assert_eq!(fc.parameters[0].name, "_0");
    assert_eq!(fc.parameters[1].name, "_1");
    assert!(fc.parameters[0].required);
}
