# open_ai_rust_fn_call_extension

[![crates.io](https://img.shields.io/crates/v/open_ai_rust_fn_call_extension.svg)](https://crates.io/crates/open_ai_rust_fn_call_extension)
[![docs.rs](https://docs.rs/open_ai_rust_fn_call_extension/badge.svg)](https://docs.rs/open_ai_rust_fn_call_extension)
[![license](https://img.shields.io/crates/l/open_ai_rust_fn_call_extension.svg)](LICENSE)

Ergonomic procedural macros for building OpenAI function-calling tools in
Rust. The companion crate to [`open_ai_rust`](https://docs.rs/open_ai_rust).

Describe Rust types and async functions as structured OpenAI tool schemas
with zero boilerplate. Three macros, one purpose: make tool calling feel
native.

---

## Highlights

- **Derive JSON schemas from any Rust type** — `#[derive(FunctionCall)]`
  generates a static `FunctionCallable` impl with `schema_type()` and
  `fn_schema()` methods. No instance required.
- **Annotate functions with their tool schema** — `#[function_call("...")]`
  emits a `const` schema next to your function.
- **One-line tool registration** — `#[tool("...")]` adds a JSON-dispatch
  wrapper that deserialises arguments, calls your function (sync or
  `async`), and serialises the result. With the `tool_registry` feature it
  also auto-registers into a global tool registry — no manual wiring.
- **`Option<T>` → `required: false`** — optionality flows naturally into
  the JSON schema.
- **`Result<T, E>` returns unwrapped automatically** — your JSON payload is
  `T`, not `{"Ok": T}`.
- **Doc-comment descriptions** — `///` on items becomes the description in
  the schema if you don't override it.
- **Async- and generics-aware** — the wrapped function's signature is
  preserved verbatim.

---

## Quick start

```toml
[dependencies]
open_ai_rust = "1"
open_ai_rust_fn_call_extension = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```rust
use open_ai_rust::FunctionCallable;
use open_ai_rust_fn_call_extension::{tool, FunctionCall};

/// A 2D point.
#[derive(FunctionCall, serde::Deserialize, serde::Serialize)]
struct Point {
    /// X coordinate.
    x: f64,
    /// Y coordinate.
    y: f64,
    /// Optional human-readable label.
    label: Option<String>,
}

/// Compute the Euclidean distance between two points.
#[tool("Compute the Euclidean distance between two points")]
async fn distance(a: Point, b: Point) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}
```

That's it. You now have:

- `Point::fn_schema()` — a full `FunctionCall` describing `Point` as a JSON
  schema object with three parameters (`x`, `y`, `label` with
  `required: false`).
- `Point::schema_type()` — the same as a `FunctionType::Object(...)`, suitable
  for nesting inside other schemas.
- `const DISTANCE: FunctionCallRaw<'static>` — the function's tool schema as
  a `const`-evaluable value.
- `fn distance_dispatch(args: serde_json::Value) -> Pin<Box<dyn Future<...>>>` —
  a typed dispatch wrapper, ready to plug into an OpenAI tool-call loop.

---

## The three macros

### `#[derive(FunctionCall)]`

Derive `FunctionCallable` for a struct or unit-variant enum.

```rust
use open_ai_rust_fn_call_extension::FunctionCall;

/// A widget.
#[derive(FunctionCall)]
struct Widget {
    /// whether it's on
    on: bool,

    #[function_call(rename = "qty")]
    quantity: i64,

    /// optional human-readable label
    label: Option<String>,

    #[function_call(skip)]
    internal_id: u64,
}
```

**Field attributes:**

| Attribute | Effect |
|---|---|
| `#[function_call(skip)]` | omit field from the schema |
| `#[function_call(rename = "...")]` | rename in the schema |
| `#[function_call(description = "...")]` | explicit description |
| `/// ...` | doc-comment fallback for description |

**Container attribute:**

| Attribute | Effect |
|---|---|
| `#[function_call(crate = "::path")]` | override the module path that owns `FunctionCallable` (defaults to `::open_ai_rust::logoi::input::tool::raw_macro`) |

**Generated methods:**

| Method | Returns | Use case |
|---|---|---|
| `schema_type()` | `FunctionType` | nest the type inside another schema |
| `fn_schema()` | `FunctionCall` | pass directly to tool registration |

**Supported shapes:** named-field structs, tuple structs (fields named
`_0`, `_1`, …), unit structs, unit-variant enums, and generic types.
Enums with data variants and unions produce a clear compile error.

### `#[function_call("description")]`

Emit a `const` schema next to a free function.

```rust
use open_ai_rust_fn_call_extension::function_call;

/// Toggle a smart-home light.
#[function_call("Toggle a smart-home light")]
fn change_light(
    on: bool,
    #[function_call_description = "brightness 0-100"] brightness: u8,
) { /* ... */ }
```

Expansion (alongside the original function, untouched):

```rust
const CHANGE_LIGHT: FunctionCallRaw<'static> = FunctionCallRaw {
    name: "change_light",
    description: "Toggle a smart-home light",
    parameters: &[
        FunctionParamRaw { name: "on",         ty: "bool", description: "" },
        FunctionParamRaw { name: "brightness", ty: "u8",   description: "brightness 0-100" },
    ],
};
```

`async fn`, generics, visibility modifiers, and `unsafe` all pass through
untouched.

> **Note on per-parameter descriptions.** Stable Rust rejects `#[doc]` and
> `///` doc-comments on function parameters, so the only way to attach a
> description to a parameter is the `#[function_call_description = "..."]`
> attribute shown above. The macro consumes it and strips it from the
> emitted function signature.

### `#[tool("description")]`

A superset of `#[function_call]`: the same schema const, plus a typed
JSON-dispatch wrapper and (optionally) global auto-registration.

```rust
use open_ai_rust_fn_call_extension::tool;

/// Safely divide two integers.
#[tool("Safely divide two integers")]
fn safe_divide(numerator: i64, denominator: i64) -> Result<i64, String> {
    if denominator == 0 {
        Err("division by zero".to_string())
    } else {
        Ok(numerator / denominator)
    }
}
```

Expansion adds:

```rust
fn safe_divide_dispatch(
    args: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<
    Output = Result<serde_json::Value, String>
> + Send + 'static>> { /* ... */ }
```

The wrapper:

1. Pulls each parameter from `args[<name>]`, defaulting to `Value::Null`.
2. Deserialises each into its declared Rust type via `serde_json::from_value`.
3. Calls the function (awaiting it if `async`).
4. For `Result<T, E>` returns: unwraps to `T` on `Ok`, returns the rendered
   error on `Err`. For plain returns: serialises directly.

#### Global auto-registration (`tool_registry` feature)

Enable the feature in both this crate and `open_ai_rust`:

```toml
[dependencies]
open_ai_rust = { version = "1", features = ["tool_registry"] }
open_ai_rust_fn_call_extension = { version = "0.3", features = ["tool_registry"] }
```

Each `#[tool]`-annotated function is then linked into
`open_ai_rust::tool_registry::TOOLS` at build time. Dispatch by name:

```rust
let result = open_ai_rust::tool_registry::invoke_tool(
    "safe_divide",
    serde_json::json!({ "numerator": 10, "denominator": 2 }),
).await?;
```

No manual registration code. Tools are discovered across crate boundaries
via [`linkme`](https://docs.rs/linkme).

---

## Description sources

| Item kind | Priority order |
|---|---|
| Function (`#[function_call]` / `#[tool]`) | `"..."` arg → `/// doc-comment` → omitted |
| Struct / unit enum | `/// doc-comment` → omitted |
| Derive field | `#[function_call(description = "...")]` → `/// doc-comment` → omitted |
| Function parameter | `#[function_call_description = "..."]` → omitted (no `#[doc]` allowed on params in stable Rust) |

---

## `Option<T>` and the `required` flag

When you derive `FunctionCall` on a struct, each emitted `FunctionParameter`
carries a `required: bool` flag:

- `field: T` → `required: true`
- `field: Option<T>` → `required: false`

This matches OpenAI's strict-schema convention: optionality lives in the
top-level `required` array, not in the parameter's type.

---

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `tool_registry` | off | Auto-register `#[tool]` functions into `open_ai_rust::tool_registry::TOOLS` via linkme. Requires `open_ai_rust/tool_registry`. |

---

## Compatibility

- **MSRV:** Rust 1.65 (uses `let ... else`).
- **`syn`:** 2.x.
- **`open_ai_rust`:** ≥ 1.1.

---

## License

[MIT](LICENSE)
