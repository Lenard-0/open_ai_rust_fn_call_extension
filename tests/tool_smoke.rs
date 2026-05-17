//! Integration tests for the `#[tool]` attribute macro.
//! All dispatch wrappers return `Pin<Box<dyn Future>>` — tests use `block_on`.
//!
//! Skipped under the `legacy-fixed-array` feature.

// Excluded under `tool_registry` — exercised separately by
// `tool_registry_smoke.rs`.
#![cfg(not(feature = "tool_registry"))]
#![allow(dead_code)]

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

use open_ai_rust_fn_call_extension::tool;
use serde_json::json;

#[tool("Add two integers")]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

/// Echo a string back to the caller.
#[tool]
fn echo(message: String) -> String {
    message
}

#[tool("Async greet")]
async fn greet(name: String) -> String {
    format!("hello, {name}")
}

/// Tool with no return type (returns unit `()`).
/// Exercises the `ReturnType::Default` branch in `tool_impl`.
#[tool("Side effect only")]
fn log_event(message: String) {
    let _ = message;
}

/// Tool returning a tuple — exercises the non-`Type::Path` arm of
/// `is_result_type`.
#[tool("Returns a (x, y) tuple")]
fn split(value: String) -> (String, String) {
    (value.clone(), value)
}

#[test]
fn tool_emits_schema_const() {
    assert_eq!(ADD.name, "add");
    assert_eq!(ADD.description, "Add two integers");
    assert_eq!(ADD.parameters.len(), 2);
    assert_eq!(ADD.parameters[0].name, "a");
    assert_eq!(ADD.parameters[1].name, "b");
}

#[test]
fn tool_dispatch_sync() {
    let out = block_on(add_dispatch(json!({ "a": 2_i64, "b": 3_i64 }))).unwrap();
    assert_eq!(out, json!(5_i64));
}

#[test]
fn tool_dispatch_doc_comment_description() {
    assert_eq!(ECHO.description, "Echo a string back to the caller.");
    let out = block_on(echo_dispatch(json!({ "message": "hi" }))).unwrap();
    assert_eq!(out, json!("hi"));
}

#[test]
fn tool_dispatch_missing_arg_is_error() {
    let err = block_on(add_dispatch(json!({ "a": 1_i64 }))).unwrap_err();
    assert!(
        err.to_lowercase().contains("null") || err.to_lowercase().contains("invalid"),
        "unexpected error: {err}"
    );
}

#[test]
fn tool_dispatch_async() {
    let result = block_on(greet_dispatch(json!({ "name": "world" }))).unwrap();
    assert_eq!(result, json!("hello, world"));
}

#[test]
fn tool_dispatch_no_return_type() {
    let result = block_on(log_event_dispatch(json!({ "message": "hi" }))).unwrap();
    // `()` serialises to `null`.
    assert_eq!(result, json!(null));
}

#[test]
fn tool_dispatch_tuple_return() {
    let result = block_on(split_dispatch(json!({ "value": "x" }))).unwrap();
    // A 2-tuple serialises to a JSON array.
    assert_eq!(result, json!(["x", "x"]));
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
