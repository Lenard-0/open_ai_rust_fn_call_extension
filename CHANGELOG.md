# Changelog

All notable changes to `open_ai_rust_fn_call_extension`. Format loosely
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## 0.3.0 — 2026-05-17

Major release aligned with [`open_ai_rust`](https://docs.rs/open_ai_rust)
≥ 1.1.

### Breaking changes

- `#[function_call]` / `#[tool]` now emit
  `parameters: &'static [FunctionParamRaw<'static>]`. The legacy
  `[&'static str; 100]` padded array (and the `legacy-fixed-array` feature
  that toggled it) have been removed.
- `#[derive(FunctionCall)]` now emits only static associated methods
  (`schema_type` / `fn_schema`). The `to_fn_call(&self)` / `to_fn_type(&self)`
  instance methods have been removed from both the derive emission and the
  `FunctionCallable` trait. Migration: replace `value.to_fn_call()` with
  `<Type>::fn_schema()`.
- `#[derive(FunctionCall)]` now requires either the canonical
  `::open_ai_rust::logoi::input::tool::raw_macro` trait module or an
  explicit `#[function_call(crate = "...")]` override.
- The derive's legacy `#[function_call_description = "..."]` attribute on
  derive **fields** has been removed. Use
  `#[function_call(description = "...")]` or a `///` doc-comment instead.
  (The attribute is still supported on function **parameters** of
  `#[function_call]` / `#[tool]` — that's the only stable-Rust mechanism
  for parameter-level descriptions, since `#[doc]` is not allowed on fn
  params.)

### Added

- **`#[tool("...")]` attribute macro.** Superset of `#[function_call]`:
  emits the schema constant plus a JSON-dispatch wrapper
  `<fn>_dispatch(args: serde_json::Value) -> Pin<Box<dyn Future<Output =
  Result<Value, String>> + Send + 'static>>`. Sync and `async fn` are both
  supported. `Result<T, E>` returns are automatically unwrapped so the JSON
  payload is `T`, not `{"Ok": T}`. The wrapped function's visibility is
  preserved on the dispatch wrapper.
- **`tool_registry` cargo feature.** When enabled, `#[tool]` additionally
  emits a `linkme`-registered `ToolEntry` into
  `::open_ai_rust::tool_registry::TOOLS`, giving zero-config global dispatch
  by name via `open_ai_rust::tool_registry::invoke_tool`. Requires the
  consumer crate to also enable `open_ai_rust/tool_registry`.
- **Static schema methods on the derive.** `schema_type() -> FunctionType`
  and `fn_schema() -> FunctionCall` (no instance required).
- **`required: bool` on every emitted `FunctionParameter`.** Set to `false`
  automatically when the field's outermost type is `Option<_>`, mirroring
  OpenAI's strict-schema convention.
- **Container attribute** `#[function_call(crate = "::path")]` for the
  derive, allowing the trait path to be overridden — useful for tests and
  consumer crates with a different module layout.
- **Field attributes** `#[function_call(skip)]`, `#[function_call(rename =
  "...")]`, `#[function_call(description = "...")]`.
- **Description fallback to `///` doc-comments** on functions, structs,
  fields, and enums.
- **Full type-token preservation** in the schema for `#[function_call]` /
  `#[tool]` parameters — `Option<String>`, `Vec<T>`, `&[u8]` etc. all
  survive the round-trip.
- **Tuple-struct support** in the derive (fields auto-named `_0`, `_1`, …).
- **Generic struct/enum support** in the derive — generics propagate
  verbatim into the impl block.
- **Structured error diagnostics** via `syn::Error::to_compile_error` for
  all macro misuse, replacing the prior `.unwrap()` panics.
- **Comprehensive test suite.** `trybuild` UI tests for failure paths;
  integration tests against a local mock of the consumer trait module.
- **GitHub Actions CI** workflow: `cargo fmt`, `clippy -D warnings`,
  `build`, `test` on every push.

### Changed

- Migrated from `syn 1.x` to `syn 2.x`.
- Removed unused `serde`, `serde_json`, and `lazy_static` dependencies.
- Bumped MSRV to **1.65** (uses `let ... else`).

### Removed

- The `legacy-fixed-array` cargo feature (no longer needed: the slice form
  is the only supported output shape, matching `open_ai_rust ≥ 1.1`).
- The `to_fn_call(&self)` / `to_fn_type(&self)` instance methods.
- Dead `for_fn_type` / `initializers` codepaths in the derive expansion.

## 0.2.17 and earlier

See git history.
