//! Integration tests for `#[function_call]` and `#[tool]`.
//! Slice-based parameter format (default, not legacy-fixed-array).

// Excluded under `tool_registry` — that path needs the sister-crate mock
// setup that lives only in `tool_registry_smoke.rs`.
#![cfg(not(feature = "tool_registry"))]
#![allow(dead_code)]

use open_ai_rust_fn_call_extension::{function_call, tool};

// ── local consumer-crate stubs ───────────────────────────────────────────────

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

// ── #[function_call] tests ───────────────────────────────────────────────────

#[function_call("Toggle a light")]
fn change_light(on: bool, #[function_call_description = "brightness 0-100"] brightness: u8) {
    let _ = (on, brightness);
}

/// Async tools must round-trip through the macro.
#[function_call("Fetch a remote thing")]
async fn fetch_thing(url: String) -> String {
    url
}

/// Doc-comment fallback when no attribute string is provided.
#[function_call]
fn no_attr_arg(value: i64) {
    let _ = value;
}

// Non-string-lit positional argument: silently ignored by the find_map,
// description falls through to the second arg (`"actually used"`).
// Exercises the `_ => None` arm of `parse_attr_description`.
#[function_call(42, "actually used")]
fn ignores_non_string_args(x: i64) {
    let _ = x;
}

// Anonymous (wildcard) parameter — exercises the `Pat::Ident` early-return
// arm of `extract_param_metadata`. The parameter is silently dropped from
// the emitted schema.
#[function_call("Wildcard param")]
fn with_anon_param(_: i64, named: i64) {
    let _ = named;
}

#[test]
fn const_emitted_for_sync_fn() {
    assert_eq!(CHANGE_LIGHT.name, "change_light");
    assert_eq!(CHANGE_LIGHT.description, "Toggle a light");
    assert_eq!(CHANGE_LIGHT.parameters.len(), 2);
    assert_eq!(
        CHANGE_LIGHT.parameters[0],
        FunctionParamRaw {
            name: "on",
            ty: "bool",
            description: ""
        }
    );
    assert_eq!(
        CHANGE_LIGHT.parameters[1],
        FunctionParamRaw {
            name: "brightness",
            ty: "u8",
            description: "brightness 0-100"
        }
    );
}

#[test]
fn const_emitted_for_async_fn() {
    assert_eq!(FETCH_THING.name, "fetch_thing");
    assert_eq!(FETCH_THING.description, "Fetch a remote thing");
    assert_eq!(
        FETCH_THING.parameters[0],
        FunctionParamRaw {
            name: "url",
            ty: "String",
            description: ""
        }
    );
    // Original async fn must still be callable.
    let fut = fetch_thing("hi".to_string());
    let result = block_on(fut);
    assert_eq!(result, "hi");
}

#[test]
fn non_string_attr_arg_is_skipped() {
    assert_eq!(IGNORES_NON_STRING_ARGS.description, "actually used");
}

#[test]
fn anonymous_param_skipped_in_schema() {
    // Anonymous `_` parameter dropped; only `named` survives.
    assert_eq!(WITH_ANON_PARAM.parameters.len(), 1);
    assert_eq!(WITH_ANON_PARAM.parameters[0].name, "named");
}

#[test]
fn doc_comment_fallback_for_fn_description() {
    assert_eq!(
        NO_ATTR_ARG.description,
        "Doc-comment fallback when no attribute string is provided."
    );
}

// ── #[tool] tests ─────────────────────────────────────────────────────────────

/// Add two integers.
#[tool("Add two integers")]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[tool("Async identity")]
async fn identity(value: String) -> String {
    value
}

/// Fallible tool: returns Result so dispatch unwraps it.
#[tool("Safe divide")]
fn safe_divide(numerator: i64, denominator: i64) -> Result<i64, String> {
    if denominator == 0 {
        Err("division by zero".to_string())
    } else {
        Ok(numerator / denominator)
    }
}

#[test]
fn tool_emits_schema_const() {
    assert_eq!(ADD.name, "add");
    assert_eq!(ADD.description, "Add two integers");
    assert_eq!(ADD.parameters.len(), 2);
    assert_eq!(ADD.parameters[0].name, "a");
    assert_eq!(ADD.parameters[0].ty, "i64");
    assert_eq!(ADD.parameters[1].name, "b");
}

#[test]
fn tool_dispatch_sync() {
    let args = serde_json::json!({ "a": 3_i64, "b": 4_i64 });
    let result = block_on(add_dispatch(args)).unwrap();
    assert_eq!(result, serde_json::json!(7_i64));
}

#[test]
fn tool_dispatch_async() {
    let args = serde_json::json!({ "value": "hello" });
    // identity_dispatch returns Pin<Box<dyn Future>> — one block_on drives it.
    let result = block_on(identity_dispatch(args)).unwrap();
    assert_eq!(result, serde_json::json!("hello"));
}

#[test]
fn tool_dispatch_result_ok() {
    let args = serde_json::json!({ "numerator": 10_i64, "denominator": 2_i64 });
    let result = block_on(safe_divide_dispatch(args)).unwrap();
    assert_eq!(result, serde_json::json!(5_i64));
}

#[test]
fn tool_dispatch_result_err() {
    let args = serde_json::json!({ "numerator": 1_i64, "denominator": 0_i64 });
    let err = block_on(safe_divide_dispatch(args)).unwrap_err();
    assert!(err.contains("division by zero"));
}

// ── zero-dep block_on ────────────────────────────────────────────────────────

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
