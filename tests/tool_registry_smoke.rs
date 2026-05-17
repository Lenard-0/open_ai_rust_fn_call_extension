//! Integration test for the `tool_registry` feature.
//!
//! `#[tool]` emits absolute paths rooted at `::open_ai_rust::...`. To exercise
//! this without depending on the sister crate at test time, we alias the
//! current crate as `open_ai_rust` via `extern crate self as open_ai_rust;`
//! and shadow each required module/type locally.

#![cfg(feature = "tool_registry")]
#![allow(dead_code)]

// Alias the test crate as `open_ai_rust` so absolute paths in macro output
// resolve against the modules defined below.
extern crate self as open_ai_rust;

// ── raw schema-const types (referenced unqualified by the macro) ─────────────

#[derive(Debug, PartialEq, Eq)]
pub struct FunctionParamRaw<'a> {
    pub name: &'a str,
    pub ty: &'a str,
    pub description: &'a str,
}

#[derive(Debug)]
pub struct FunctionCallRaw<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub parameters: &'a [FunctionParamRaw<'a>],
}

// ── module mirror of the consumer crate ──────────────────────────────────────

pub mod __macro_support {
    pub use linkme;
}

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
                BooleanTy,
            }

            #[derive(Debug, Clone, PartialEq)]
            pub enum EnumValues {
                String(Vec<String>),
                Int(Vec<i64>),
                Float(Vec<f64>),
            }

            pub mod raw_macro {
                use super::FunctionType;

                pub trait FunctionCallable {
                    fn schema_type() -> FunctionType
                    where
                        Self: Sized;
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
                impl FunctionCallable for bool {
                    fn schema_type() -> FunctionType {
                        FunctionType::BooleanTy
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

pub mod tool_registry {
    use crate::logoi::input::tool::FunctionCall;
    use serde_json::Value;
    use std::future::Future;
    use std::pin::Pin;

    pub type DispatchFuture = Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'static>>;
    pub type DispatchFn = fn(Value) -> DispatchFuture;

    pub struct ToolEntry {
        pub name: &'static str,
        pub description: Option<&'static str>,
        pub schema: fn() -> FunctionCall,
        pub dispatch: DispatchFn,
    }

    #[linkme::distributed_slice]
    pub static TOOLS: [ToolEntry] = [..];
}

// ── tool definitions ─────────────────────────────────────────────────────────

use open_ai_rust_fn_call_extension::tool;

/// Adds two integers — required-param case.
#[tool("Adds two integers")]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

/// Optional second parameter — exercises the `is_option_type` `required = false`
/// path inside `tool_registry` schema emission.
#[tool("Greets, optionally with a custom prefix")]
fn greet(name: String, prefix: Option<String>) -> String {
    format!("{}, {name}", prefix.unwrap_or_else(|| "Hello".to_string()))
}

// No description (no macro arg, no doc-comment) — exercises the `None` branch
// of `optional_static_str` inside the tool_registry ToolEntry initializer.
#[tool]
fn no_desc(x: i64) -> i64 {
    x
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn registry_contains_both_tools() {
    let names: Vec<&str> = tool_registry::TOOLS.iter().map(|t| t.name).collect();
    assert!(names.contains(&"add"));
    assert!(names.contains(&"greet"));
}

#[test]
fn registry_schema_emits_correct_required_flags() {
    let entry = tool_registry::TOOLS
        .iter()
        .find(|t| t.name == "greet")
        .expect("greet should be registered");
    let schema = (entry.schema)();
    assert_eq!(schema.name, "greet");
    assert_eq!(
        schema.description.as_deref(),
        Some("Greets, optionally with a custom prefix")
    );
    assert_eq!(schema.parameters.len(), 2);
    assert!(schema.parameters[0].required); // name: String
    assert!(!schema.parameters[1].required); // prefix: Option<String>
}

#[test]
fn registry_dispatch_works() {
    let entry = tool_registry::TOOLS
        .iter()
        .find(|t| t.name == "add")
        .expect("add should be registered");
    let fut = (entry.dispatch)(serde_json::json!({ "a": 10_i64, "b": 5_i64 }));
    let result = block_on(fut).unwrap();
    assert_eq!(result, serde_json::json!(15_i64));
}

#[test]
fn no_desc_tool_has_none_description() {
    let entry = tool_registry::TOOLS
        .iter()
        .find(|t| t.name == "no_desc")
        .expect("no_desc should be registered");
    assert_eq!(entry.description, None);
}

#[test]
fn registry_entry_descriptions_are_static_str() {
    // The `description` field on `ToolEntry` is `Option<&'static str>`,
    // which is constructed via `optional_static_str` for the `Some` case.
    let entry = tool_registry::TOOLS
        .iter()
        .find(|t| t.name == "add")
        .unwrap();
    assert_eq!(entry.description, Some("Adds two integers"));
}

fn block_on<F: std::future::Future>(mut fut: F) -> F::Output {
    use std::pin::Pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    fn raw_waker() -> RawWaker {
        fn no_op(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            raw_waker()
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    let waker = unsafe { Waker::from_raw(raw_waker()) };
    let mut cx = Context::from_waker(&waker);
    let mut fut = unsafe { Pin::new_unchecked(&mut fut) };
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => continue,
        }
    }
}
