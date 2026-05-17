//! # open_ai_rust_fn_call_extension
//!
//! Ergonomic procedural macros for building OpenAI function-calling tools in Rust.
//! The companion crate to [`open_ai_rust`].
//!
//! ## What this crate gives you
//!
//! Three macros, one purpose: describe Rust types and functions as structured
//! OpenAI tool schemas with zero boilerplate.
//!
//! | Macro | Apply to | What it emits |
//! |---|---|---|
//! | [`#[derive(FunctionCall)]`](macro@FunctionCall) | structs / unit enums | `FunctionCallable` impl (`schema_type`, `fn_schema`) |
//! | [`#[function_call]`](macro@function_call) | free functions | a `const FN_NAME: FunctionCallRaw<'static>` schema next to the function |
//! | [`#[tool]`](macro@tool) | free functions | the schema const **plus** a JSON-dispatch async wrapper, optionally auto-registered into a global tool registry |
//!
//! ## Quick start
//!
//! Add both crates to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! open_ai_rust = "1"
//! open_ai_rust_fn_call_extension = "0.3"
//! serde_json = "1"
//!
//! # Optional: auto-register #[tool] functions into a global registry.
//! # [dependencies.open_ai_rust]
//! # version = "1"
//! # features = ["tool_registry"]
//! # [dependencies.open_ai_rust_fn_call_extension]
//! # version = "0.3"
//! # features = ["tool_registry"]
//! ```
//!
//! Describe a tool function and a tool argument struct:
//!
//! ```ignore
//! use open_ai_rust::{FunctionCallable, FunctionType};
//! use open_ai_rust_fn_call_extension::{tool, FunctionCall};
//!
//! /// A point in 2D space.
//! #[derive(FunctionCall, serde::Deserialize, serde::Serialize)]
//! struct Point {
//!     /// X coordinate.
//!     x: f64,
//!     /// Y coordinate.
//!     y: f64,
//!     /// Optional label (omittable in the JSON schema's `required` array).
//!     label: Option<String>,
//! }
//!
//! /// Compute the Euclidean distance between two points.
//! #[tool("Compute Euclidean distance between two points")]
//! async fn distance(a: Point, b: Point) -> f64 {
//!     ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
//! }
//! ```
//!
//! After expansion you have:
//!
//! * `Point::schema_type()` returning a JSON-schema `FunctionType::Object`
//!   with three parameters (`label` marked `required: false`).
//! * `Point::fn_schema()` returning a full `FunctionCall` schema.
//! * `const DISTANCE: FunctionCallRaw<'static>` describing the function.
//! * `fn distance_dispatch(args: serde_json::Value) -> Pin<Box<dyn Future<...>>>`,
//!   ready to be wired into a tool-call loop.
//!
//! With the `tool_registry` feature enabled, `distance` is also registered into
//! [`open_ai_rust::tool_registry::TOOLS`] at link time, so you can simply call
//! [`open_ai_rust::tool_registry::invoke_tool`] with the tool name and arguments.
//!
//! ## Feature flags
//!
//! | Feature | Default | Effect |
//! |---|---|---|
//! | `tool_registry` | off | Auto-register `#[tool]` functions into the global `TOOLS` linkme slice. Requires `open_ai_rust/tool_registry`. |
//!
//! ## Description sources
//!
//! Wherever a description string is accepted, the macros resolve it in this
//! priority order:
//!
//! - **Functions, structs, fields, enums:**
//!   1. Explicit macro argument: `#[function_call("...")]` / `#[tool("...")]`
//!      on a fn, or `#[function_call(description = "...")]` on a derive field.
//!   2. Outer doc-comment: `/// ...` directly above the item.
//!   3. Omitted (`None`).
//!
//! - **Function parameters** (for `#[function_call]` / `#[tool]`):
//!   - `#[function_call_description = "..."]` attached to the parameter.
//!     This is the only mechanism — stable Rust rejects `#[doc = "..."]`
//!     (and therefore `///`) on function parameters.
//!
//! ## Compatibility
//!
//! * MSRV: **1.65** (uses `let ... else` syntax).
//! * Requires `syn 2.x` — already on board, no further setup needed.
//! * Pairs with [`open_ai_rust`] ≥ 1.1.
//!
//! ## Status
//!
//! Pre-1.0 of this crate; emitted-code shape may change in minor releases.
//! See [`CHANGELOG.md`](https://github.com/Lenard-0/open_ai_rust_fn_call_extension/blob/master/CHANGELOG.md).
//!
//! [`open_ai_rust`]: https://docs.rs/open_ai_rust
//! [`open_ai_rust::tool_registry::TOOLS`]: https://docs.rs/open_ai_rust/latest/open_ai_rust/tool_registry/static.TOOLS.html
//! [`open_ai_rust::tool_registry::invoke_tool`]: https://docs.rs/open_ai_rust/latest/open_ai_rust/tool_registry/fn.invoke_tool.html

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::{
    parse::Parser, parse_macro_input, punctuated::Punctuated, Attribute, Data, DeriveInput, Expr,
    ExprLit, FnArg, Ident, ItemFn, Lit, Meta, Pat, Token,
};

// ───────────────────────────────────────────────────────────────────────────────
// Helpers
// ───────────────────────────────────────────────────────────────────────────────

/// Render `Option<String>` literal at expansion time:
/// empty string → `None`, otherwise `Some(String::from("..."))`.
fn optional_string(s: &str) -> TokenStream2 {
    if s.is_empty() {
        quote! { None }
    } else {
        quote! { Some(String::from(#s)) }
    }
}

/// Render `Option<&'static str>` literal (no allocation): empty → `None`,
/// otherwise `Some("...")`. Used inside `static` initializers where allocation
/// is forbidden.
fn optional_static_str(s: &str) -> TokenStream2 {
    if s.is_empty() {
        quote! { None }
    } else {
        quote! { Some(#s) }
    }
}

/// Check whether the outermost wrapper of a type is `Option<_>`.
///
/// Used by the derive macro to mark struct fields as non-required in the
/// emitted JSON schema when their type is `Option<T>`.
fn is_option_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else {
        return false;
    };
    tp.path
        .segments
        .last()
        .map(|s| s.ident == "Option")
        .unwrap_or(false)
}

/// Check whether the outermost wrapper of a type is `Result<_, _>`.
///
/// Used by the `#[tool]` macro to decide whether the dispatch wrapper needs
/// to unwrap the function's return value before serialising — so that a
/// `Result<T, E>` payload becomes `T` in the JSON response, not `{"Ok": T}`.
fn is_result_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else {
        return false;
    };
    tp.path
        .segments
        .last()
        .map(|s| s.ident == "Result")
        .unwrap_or(false)
}

/// Render the full token stream of a type as a string so generic parameters
/// survive (e.g. `Option<String>`, `Vec<T>`, `&[u8]`). The `quote`/`ToTokens`
/// default printing inserts extra whitespace around punctuation; this helper
/// strips it so the output reads close to the original source form.
fn type_token_string(ty: &syn::Type) -> String {
    ty.to_token_stream()
        .to_string()
        .replace(" < ", "<")
        .replace(" > ", ">")
        .replace(" >", ">")
        .replace(" ,", ",")
        .replace(" ::", "::")
        .replace(":: ", "::")
}

/// Read `#[function_call_description = "..."]` from a function parameter's
/// attribute list. This is the only way to attach a description to a fn
/// parameter in stable Rust: `#[doc]` is rejected by the resolver on params,
/// so doc-comments can't be used there.
fn extract_parameter_description(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("function_call_description") {
            return None;
        }
        let Meta::NameValue(nv) = &attr.meta else {
            return None;
        };
        if let Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) = &nv.value
        {
            Some(s.value())
        } else {
            None
        }
    })
}

/// Concatenate `///` doc-comment lines (which desugar to `#[doc = "..."]`)
/// into a single trimmed description string. Returns `None` if there are none.
///
/// Used as a fallback description source for functions, struct fields,
/// structs, and enums (not function parameters — see
/// [`extract_parameter_description`]).
fn collect_doc_comment(attrs: &[Attribute]) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let Meta::NameValue(nv) = &attr.meta else {
            continue;
        };
        if let Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) = &nv.value
        {
            lines.push(s.value().trim().to_string());
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join(" ").trim().to_string())
    }
}

// ───────────────────────────────────────────────────────────────────────────────
// Derive option parsing
// ───────────────────────────────────────────────────────────────────────────────

/// Field-level `#[function_call(...)]` options on a derive target.
#[derive(Default)]
struct FieldOpts {
    /// `#[function_call(skip)]` — omit this field from the emitted schema.
    skip: bool,
    /// `#[function_call(rename = "...")]` — schema-facing field name.
    rename: Option<String>,
    /// `#[function_call(description = "...")]` — explicit description override.
    description: Option<String>,
}

/// Container-level `#[function_call(...)]` options on a derive target.
struct ContainerOpts {
    /// `#[function_call(crate = "...")]` — module path that owns the
    /// `FunctionCallable` trait and the supporting schema types
    /// (`FunctionCall`, `FunctionParameter`, `FunctionType`, `EnumValues`).
    /// Defaults to the canonical path inside [`open_ai_rust`].
    crate_path: String,
}

impl Default for ContainerOpts {
    fn default() -> Self {
        Self {
            crate_path: "::open_ai_rust::logoi::input::tool::raw_macro".to_string(),
        }
    }
}

/// Parse `#[function_call(crate = "...")]` from a container's outer attributes.
fn parse_container_opts(attrs: &[Attribute]) -> syn::Result<ContainerOpts> {
    let mut opts = ContainerOpts::default();
    for attr in attrs {
        if !attr.path().is_ident("function_call") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                opts.crate_path = meta.value()?.parse::<syn::LitStr>()?.value();
                return Ok(());
            }
            Err(meta.error("unknown function_call container attribute (expected: crate)"))
        })?;
    }
    Ok(opts)
}

/// Parse `#[function_call(skip|rename = "..."|description = "...")]` from a
/// field's outer attributes.
fn parse_field_opts(attrs: &[Attribute]) -> syn::Result<FieldOpts> {
    let mut opts = FieldOpts::default();
    for attr in attrs {
        if !attr.path().is_ident("function_call") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                opts.skip = true;
                return Ok(());
            }
            if meta.path.is_ident("rename") {
                opts.rename = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                return Ok(());
            }
            if meta.path.is_ident("description") {
                opts.description = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                return Ok(());
            }
            Err(meta.error("unknown function_call attribute (expected: skip, rename, description)"))
        })?;
    }
    Ok(opts)
}

// ───────────────────────────────────────────────────────────────────────────────
// #[derive(FunctionCall)]
// ───────────────────────────────────────────────────────────────────────────────

/// Derive `FunctionCallable` for a struct or unit-variant enum, generating
/// all the methods needed to describe the type as a JSON schema in an OpenAI
/// tool definition.
///
/// # Methods generated
///
/// The derive produces an `impl` block with two static associated methods:
///
/// | Method | Returns | Use case |
/// |---|---|---|
/// | `schema_type()` | `FunctionType` | derive a `FunctionType` for this type — no instance needed |
/// | `fn_schema()` | `FunctionCall` | derive a full `FunctionCall` — pass straight to a tool registration |
///
/// # Container attribute
///
/// `#[function_call(crate = "::path::to::trait_module")]`
///
/// Override the module path used to resolve the `FunctionCallable` trait and
/// its supporting types. Useful for crates that re-export the trait at a
/// different path, or for integration tests that mock the consumer crate.
///
/// The default path is `::open_ai_rust::logoi::input::tool::raw_macro`.
///
/// # Field attributes
///
/// | Attribute | Effect |
/// |---|---|
/// | `#[function_call(skip)]` | Omit the field from the emitted schema entirely. |
/// | `#[function_call(rename = "...")]` | Use this name in the schema instead of the field's Rust identifier. |
/// | `#[function_call(description = "...")]` | Set the parameter description explicitly. |
/// | `/// ...` (doc-comment) | Fallback description if no explicit one is given. |
///
/// # `required` semantics
///
/// Every emitted `FunctionParameter` carries a `required: bool` flag. The
/// derive sets it to `false` automatically when the field's outermost type is
/// `Option<T>`, and `true` otherwise. This mirrors OpenAI's strict-schema
/// convention: optionality lives in the top-level `required` array, not in
/// the parameter's type.
///
/// # Supported shapes
///
/// | Shape | Behaviour |
/// |---|---|
/// | named-field struct | Each field → one `FunctionParameter` |
/// | tuple struct | Fields auto-named `_0`, `_1`, ... |
/// | unit struct | Zero parameters |
/// | unit-variant enum | Emits `FunctionType::Enum(EnumValues::String(...))` |
/// | generic struct/enum | Generics are propagated to the impl block as-is |
///
/// # Not yet supported
///
/// * Enums with data variants (would emit `FunctionType::OneOf`). Compile error.
/// * Unions. Compile error.
///
/// # Example
///
/// ```ignore
/// use open_ai_rust::FunctionCallable;
/// use open_ai_rust_fn_call_extension::FunctionCall;
///
/// /// A 2D point.
/// #[derive(FunctionCall, serde::Deserialize)]
/// struct Point {
///     /// X coordinate.
///     x: f64,
///     /// Y coordinate.
///     y: f64,
///     /// Optional human-readable label.
///     label: Option<String>,
///     #[function_call(skip)]
///     internal_id: u64,
/// }
///
/// // Zero-instance schema retrieval.
/// let schema = Point::fn_schema();
/// assert_eq!(schema.parameters.len(), 3);   // x, y, label (internal_id skipped)
/// assert!(!schema.parameters[2].required);  // label is Option<_>
/// ```
///
/// [`FunctionCallable`]: https://docs.rs/open_ai_rust/latest/open_ai_rust/trait.FunctionCallable.html
/// [`open_ai_rust`]: https://docs.rs/open_ai_rust
#[proc_macro_derive(FunctionCall, attributes(function_call))]
pub fn turn_type_to_function_call(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match derive_impl(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Per-field metadata collected during derive expansion.
struct FieldInfo {
    /// The field's syntactic type — needed both for emitting
    /// `<Ty as FunctionCallable>::schema_type()` and for the `required` check.
    ty: syn::Type,
    /// Schema-facing name (after any `rename = "..."` override).
    name_out: String,
    /// Resolved description (attribute → doc-comment → empty).
    description: String,
    /// `false` when the field's outermost type is `Option<_>`. See the
    /// [`#[derive(FunctionCall)]`](macro@FunctionCall) docs for rationale.
    required: bool,
}

fn collect_fields(s: &syn::DataStruct) -> syn::Result<Vec<FieldInfo>> {
    let mut out = Vec::new();
    for (i, f) in s.fields.iter().enumerate() {
        let opts = parse_field_opts(&f.attrs)?;
        if opts.skip {
            continue;
        }
        let default_name = match &f.ident {
            Some(id) => id.to_string(),
            None => format!("_{i}"),
        };
        let name_out = opts.rename.unwrap_or(default_name);
        let description = opts
            .description
            .or_else(|| collect_doc_comment(&f.attrs))
            .unwrap_or_default();
        let required = !is_option_type(&f.ty);
        out.push(FieldInfo {
            ty: f.ty.clone(),
            name_out,
            description,
            required,
        });
    }
    Ok(out)
}

/// Emit `FunctionParameter { ... }` constructor expressions for a slice of
/// fields, suitable for splicing into a `vec![...]`. The `_type` value is
/// computed via `<#ty as FunctionCallable>::schema_type()`.
fn emit_param_vec<'a>(
    fields: &'a [FieldInfo],
    trait_path: &syn::Path,
) -> impl Iterator<Item = TokenStream2> + 'a {
    let tp = trait_path.clone();
    fields.iter().map(move |f| {
        let name = &f.name_out;
        let desc = optional_string(&f.description);
        let req = f.required;
        let ty = &f.ty;
        quote! {
            FunctionParameter {
                name: String::from(#name),
                _type: <#ty as #tp ::FunctionCallable>::schema_type(),
                description: #desc,
                required: #req,
            }
        }
    })
}

fn derive_impl(input: DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let struct_description = collect_doc_comment(&input.attrs).unwrap_or_default();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let container = parse_container_opts(&input.attrs)?;
    let trait_path: syn::Path = syn::parse_str(&container.crate_path)?;
    let struct_desc_tokens = optional_string(&struct_description);

    match &input.data {
        Data::Struct(s) => {
            let fields = collect_fields(s)?;
            let params: Vec<_> = emit_param_vec(&fields, &trait_path).collect();
            let params2 = params.clone();

            Ok(quote! {
                impl #impl_generics #trait_path ::FunctionCallable for #name #ty_generics #where_clause {
                    fn schema_type() -> FunctionType where Self: ::std::marker::Sized {
                        FunctionType::Object(vec![ #( #params, )* ])
                    }

                    fn fn_schema() -> FunctionCall where Self: ::std::marker::Sized {
                        FunctionCall {
                            name: String::from(stringify!(#name)),
                            description: #struct_desc_tokens,
                            parameters: vec![ #( #params2, )* ],
                        }
                    }
                }
            })
        }

        Data::Enum(e) => {
            let variants: Vec<&Ident> = e
                .variants
                .iter()
                .map(|v| {
                    if !matches!(v.fields, syn::Fields::Unit) {
                        return Err(syn::Error::new_spanned(
                            v,
                            "FunctionCall derive currently only supports unit enum variants; \
                             data-variant enums (oneOf) are planned for a future release",
                        ));
                    }
                    Ok(&v.ident)
                })
                .collect::<syn::Result<_>>()?;

            let variant_strings: Vec<String> = variants.iter().map(|v| v.to_string()).collect();

            Ok(quote! {
                impl #impl_generics #trait_path ::FunctionCallable for #name #ty_generics #where_clause {
                    fn schema_type() -> FunctionType where Self: ::std::marker::Sized {
                        FunctionType::Enum(EnumValues::String(vec![
                            #( String::from(#variant_strings) ),*
                        ]))
                    }

                    fn fn_schema() -> FunctionCall where Self: ::std::marker::Sized {
                        FunctionCall {
                            name: String::from(stringify!(#name)),
                            description: #struct_desc_tokens,
                            parameters: vec![],
                        }
                    }
                }
            })
        }

        Data::Union(u) => Err(syn::Error::new_spanned(
            u.union_token,
            "FunctionCall derive does not support unions",
        )),
    }
}

// ───────────────────────────────────────────────────────────────────────────────
// Shared function-metadata extraction (used by #[function_call] and #[tool])
// ───────────────────────────────────────────────────────────────────────────────

/// Parsed metadata for one function parameter.
struct ParamMeta {
    /// The original binding identifier — preserved so the dispatch wrapper
    /// can pass it positionally to the underlying function.
    ident: Ident,
    /// Schema-facing parameter name (currently `ident.to_string()`).
    name: String,
    /// Original parsed type — required by `#[tool]` to typecheck the
    /// `serde_json::from_value(..)` dispatch call.
    ty: syn::Type,
    /// Rendered type string for the schema (preserves generics).
    ty_string: String,
    /// Resolved description (doc-comment fallback → empty).
    description: String,
}

/// Walk a function signature and collect a `ParamMeta` per typed argument.
/// Per-parameter descriptions come from `#[function_call_description = "..."]`
/// attributes (the only way to attach a description to a fn param in stable
/// Rust). `self` and unnamed patterns (e.g. `_`) are silently skipped.
fn extract_param_metadata(input: &ItemFn) -> Vec<ParamMeta> {
    let mut out = Vec::new();
    for arg in input.sig.inputs.iter() {
        let FnArg::Typed(pat_type) = arg else {
            continue;
        };
        let Pat::Ident(pi) = &*pat_type.pat else {
            continue;
        };
        let ident = pi.ident.clone();
        let name = ident.to_string();
        let ty = (*pat_type.ty).clone();
        let ty_string = type_token_string(&pat_type.ty);
        let description = extract_parameter_description(&pat_type.attrs).unwrap_or_default();
        out.push(ParamMeta {
            ident,
            name,
            ty,
            ty_string,
            description,
        });
    }
    out
}

/// Remove `#[function_call_description = "..."]` from re-emitted function
/// signatures. The attribute is consumed by `#[function_call]` / `#[tool]`
/// at expansion time, so leaving it in the output would be rejected by
/// rustc (the resolver disallows arbitrary attributes on fn parameters).
fn strip_helper_attrs(input: &mut ItemFn) {
    for arg in input.sig.inputs.iter_mut() {
        if let FnArg::Typed(pat_type) = arg {
            pat_type
                .attrs
                .retain(|a| !a.path().is_ident("function_call_description"));
        }
    }
}

/// Build the `const FN_NAME: FunctionCallRaw<'static>` initializer next to a
/// function, using the structured `&'static [FunctionParamRaw<'static>]`
/// parameter representation.
fn build_schema_const(fn_name: &str, description: &str, params: &[ParamMeta]) -> TokenStream2 {
    let fn_name_uppercase = Ident::new(&fn_name.to_uppercase(), Span::call_site());
    let param_lits = params.iter().map(|p| {
        let n = &p.name;
        let t = &p.ty_string;
        let d = &p.description;
        quote! { FunctionParamRaw { name: #n, ty: #t, description: #d } }
    });
    quote! {
        const #fn_name_uppercase: FunctionCallRaw<'static> = FunctionCallRaw {
            name: #fn_name,
            description: #description,
            parameters: &[ #( #param_lits ),* ],
        };
    }
}

/// Resolve a function-level description, in priority order:
///   1. positional string-literal argument to the attribute macro
///   2. outer `///` doc-comment on the function
///   3. empty string
fn parse_attr_description(attr: TokenStream2, fn_attrs: &[Attribute]) -> syn::Result<String> {
    let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
    let attr_args = parser.parse2(attr)?;
    Ok(attr_args
        .iter()
        .find_map(|e| match e {
            Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) => Some(s.value()),
            _ => None,
        })
        .or_else(|| collect_doc_comment(fn_attrs))
        .unwrap_or_default())
}

// ───────────────────────────────────────────────────────────────────────────────
// #[function_call("description")]
// ───────────────────────────────────────────────────────────────────────────────

/// Emit a `const FN_NAME: FunctionCallRaw<'static>` schema next to a free
/// function. The original function body is preserved unchanged.
///
/// # Syntax
///
/// ```ignore
/// #[function_call("Optional description")]
/// fn my_tool(arg1: T1, arg2: T2) -> R { ... }
/// ```
///
/// The function name is uppercased to form the generated constant: `my_tool`
/// becomes `MY_TOOL`. The constant is `const`-evaluable, so it can live in
/// a module's top-level scope alongside the function it describes.
///
/// # Description sources
///
/// Function-level:
/// 1. Positional string literal: `#[function_call("...")]`
/// 2. Outer doc-comment: `/// ...` directly above the function
/// 3. Empty (description omitted)
///
/// Per-parameter: `#[function_call_description = "..."]` on the parameter.
/// (Doc-comments / `#[doc]` aren't accepted on fn parameters in stable Rust.)
///
/// # Example
///
/// Input:
///
/// ```ignore
/// use open_ai_rust_fn_call_extension::function_call;
///
/// /// Toggle a smart-home light.
/// #[function_call("Toggle a smart-home light")]
/// fn change_light(
///     on: bool,
///     #[function_call_description = "brightness 0-100"] brightness: u8,
/// ) { /* ... */ }
/// ```
///
/// Generated alongside the (untouched) function:
///
/// ```ignore
/// const CHANGE_LIGHT: FunctionCallRaw<'static> = FunctionCallRaw {
///     name: "change_light",
///     description: "Toggle a smart-home light",
///     parameters: &[
///         FunctionParamRaw { name: "on",         ty: "bool", description: "" },
///         FunctionParamRaw { name: "brightness", ty: "u8",   description: "brightness 0-100" },
///     ],
/// };
/// ```
///
/// `async fn`, generics, visibility modifiers, and `unsafe` are all passed
/// through untouched.
///
/// [`open_ai_rust`]: https://docs.rs/open_ai_rust
#[proc_macro_attribute]
pub fn function_call(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    match function_call_impl(attr.into(), input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn function_call_impl(attr: TokenStream2, mut input: ItemFn) -> syn::Result<TokenStream2> {
    let fn_name = input.sig.ident.to_string();
    let description = parse_attr_description(attr, &input.attrs)?;
    let params = extract_param_metadata(&input);
    strip_helper_attrs(&mut input);
    let const_body = build_schema_const(&fn_name, &description, &params);
    Ok(quote! { #const_body #input })
}

// ───────────────────────────────────────────────────────────────────────────────
// #[tool("description")]
// ───────────────────────────────────────────────────────────────────────────────

/// Superset of [`#[function_call]`](macro@function_call): emits the schema
/// const **and** a JSON-dispatch wrapper that deserialises arguments, invokes
/// the underlying function, and serialises its return value back to JSON.
///
/// With the `tool_registry` feature enabled, the wrapper is also auto-registered
/// into [`open_ai_rust::tool_registry::TOOLS`] via `linkme`, giving you zero-config
/// runtime dispatch.
///
/// # Emitted items
///
/// For a function `fn foo(...) -> R`, three items are emitted:
///
/// 1. `const FOO: FunctionCallRaw<'static>` — same schema constant as
///    `#[function_call]`.
/// 2. `fn foo_dispatch(args: serde_json::Value)
///    -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send + 'static>>` —
///    the dispatch wrapper. The original function's visibility is preserved.
/// 3. With `tool_registry`: a `static __OA_TOOL_FOO: ToolEntry` registered into
///    the global `TOOLS` slice.
///
/// The original function definition is preserved exactly as written.
///
/// # Dispatch behaviour
///
/// The wrapper:
///
/// 1. Looks up each named parameter in the input `args` JSON object via
///    `args.get(<name>).cloned().unwrap_or(Value::Null)`.
/// 2. Deserialises each into its declared Rust type via `serde_json::from_value`.
/// 3. Calls the original function — `await`ing it if `async`.
/// 4. Serialises the return value via `serde_json::to_value`.
///
/// All `serde_json` errors are converted to `String` via `to_string()` and
/// returned as the `Err` arm of the future.
///
/// # `Result<T, E>` return types
///
/// If the wrapped function returns `Result<T, E>` where `E: Display`, the
/// dispatch wrapper unwraps it so the JSON payload is `T` (not the serialised
/// `Result` enum `{"Ok": T}` / `{"Err": E}`). On `Err`, the error is rendered
/// via `format!("{e}")` and returned as the wrapper's `Err` arm.
///
/// # Async functions
///
/// `async fn` is detected automatically — the underlying call is `await`ed
/// inside the wrapper's async block. No additional configuration required.
///
/// # Description sources
///
/// Identical to [`#[function_call]`](macro@function_call): positional string
/// literal, then outer `///` doc-comment, then empty.
///
/// # Example
///
/// ```ignore
/// use open_ai_rust_fn_call_extension::tool;
///
/// /// Safely divide two integers.
/// #[tool("Safely divide two integers")]
/// fn safe_divide(numerator: i64, denominator: i64) -> Result<i64, String> {
///     if denominator == 0 {
///         Err("division by zero".to_string())
///     } else {
///         Ok(numerator / denominator)
///     }
/// }
///
/// // At runtime:
/// let args = serde_json::json!({ "numerator": 10, "denominator": 2 });
/// let result = futures::executor::block_on(safe_divide_dispatch(args)).unwrap();
/// assert_eq!(result, serde_json::json!(5));
/// ```
///
/// # Requirements
///
/// * `serde_json` must be reachable as a top-level dependency of the consumer
///   crate (the emitted code uses the rooted path `::serde_json::Value`).
/// * With the `tool_registry` feature: the consumer must also depend on
///   [`open_ai_rust`] with its own `tool_registry` feature enabled, since
///   the emitted code resolves `::open_ai_rust::tool_registry::TOOLS` and
///   `::open_ai_rust::__macro_support::linkme`.
///
/// [`open_ai_rust`]: https://docs.rs/open_ai_rust
/// [`open_ai_rust::tool_registry::TOOLS`]: https://docs.rs/open_ai_rust/latest/open_ai_rust/tool_registry/static.TOOLS.html
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    match tool_impl(attr.into(), input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn tool_impl(attr: TokenStream2, mut input: ItemFn) -> syn::Result<TokenStream2> {
    let fn_name = input.sig.ident.to_string();
    let fn_ident = input.sig.ident.clone();
    let description = parse_attr_description(attr, &input.attrs)?;
    let params = extract_param_metadata(&input);
    strip_helper_attrs(&mut input);
    let const_body = build_schema_const(&fn_name, &description, &params);

    let is_async = input.sig.asyncness.is_some();
    let vis = &input.vis;
    let dispatch_ident = Ident::new(&format!("{fn_name}_dispatch"), Span::call_site());

    // Deserialise each argument by name from the JSON args object.
    let arg_extractions: Vec<TokenStream2> = params
        .iter()
        .map(|p| {
            let ident = &p.ident;
            let name_lit = &p.name;
            let ty = &p.ty;
            quote! {
                let #ident: #ty = ::serde_json::from_value(
                    args.get(#name_lit).cloned().unwrap_or(::serde_json::Value::Null)
                ).map_err(|e| e.to_string())?;
            }
        })
        .collect();

    let arg_idents: Vec<&Ident> = params.iter().map(|p| &p.ident).collect();

    // Detect return type to handle `Result<T, E>` vs plain `T`.
    let ret_is_result = match &input.sig.output {
        syn::ReturnType::Type(_, ty) => is_result_type(ty),
        _ => false,
    };

    let raw_call = if is_async {
        quote! { #fn_ident( #( #arg_idents ),* ).await }
    } else {
        quote! { #fn_ident( #( #arg_idents ),* ) }
    };

    // Serialise the call result into `Result<Value, String>`. For
    // `Result<T, E>` returns, unwrap first so the JSON payload is `T`,
    // not `{"Ok": T}` / `{"Err": E}`.
    let serialize_expr = if ret_is_result {
        quote! {
            let __r = #raw_call.map_err(|e| format!("{e}"))?;
            ::serde_json::to_value(__r).map_err(|e| e.to_string())
        }
    } else {
        quote! {
            let __r = #raw_call;
            ::serde_json::to_value(__r).map_err(|e| e.to_string())
        }
    };

    // Standalone dispatch function — `fn(Value) -> Pin<Box<dyn Future<...>>>`.
    // We don't emit `async fn` because the resulting function pointer needs to
    // match the `DispatchFn` type alias in the consumer's tool registry.
    let dispatch_fn = quote! {
        #vis fn #dispatch_ident(
            args: ::serde_json::Value,
        ) -> ::std::pin::Pin<
                ::std::boxed::Box<
                    dyn ::std::future::Future<
                        Output = ::std::result::Result<::serde_json::Value, String>
                    > + Send + 'static
                >
            >
        {
            ::std::boxed::Box::pin(async move {
                #( #arg_extractions )*
                #serialize_expr
            })
        }
    };

    // ── tool_registry feature-gated ToolEntry registration ───────────────────
    let tool_registry_entry = if cfg!(feature = "tool_registry") {
        let desc_tokens = optional_static_str(&description);

        // Named helper fns are needed because `static` initialisers require
        // function pointers, not closures.
        let schema_helper = Ident::new(&format!("__oa_tool_schema_{fn_name}"), Span::call_site());
        let dispatch_helper =
            Ident::new(&format!("__oa_tool_dispatch_{fn_name}"), Span::call_site());
        let static_ident = Ident::new(
            &format!("__OA_TOOL_{}", fn_name.to_uppercase()),
            Span::call_site(),
        );

        // Build a full `FunctionCall` schema in the schema helper, with
        // `FunctionParameter` items derived statically via `schema_type()`.
        let schema_params = params.iter().map(|p| {
            let n = &p.name;
            let d = optional_string(&p.description);
            let ty = &p.ty;
            let req = !is_option_type(&p.ty);
            quote! {
                ::open_ai_rust::logoi::input::tool::FunctionParameter {
                    name: String::from(#n),
                    _type: <#ty as ::open_ai_rust::logoi::input::tool::raw_macro::FunctionCallable>::schema_type(),
                    description: #d,
                    required: #req,
                }
            }
        });

        let fn_description = optional_string(&description);

        quote! {
            fn #schema_helper() -> ::open_ai_rust::logoi::input::tool::FunctionCall {
                ::open_ai_rust::logoi::input::tool::FunctionCall {
                    name: String::from(#fn_name),
                    description: #fn_description,
                    parameters: vec![ #( #schema_params, )* ],
                }
            }

            fn #dispatch_helper(
                args: ::serde_json::Value,
            ) -> ::open_ai_rust::tool_registry::DispatchFuture {
                #dispatch_ident(args)
            }

            #[::open_ai_rust::__macro_support::linkme::distributed_slice(
                ::open_ai_rust::tool_registry::TOOLS
            )]
            #[linkme(crate = ::open_ai_rust::__macro_support::linkme)]
            static #static_ident: ::open_ai_rust::tool_registry::ToolEntry =
                ::open_ai_rust::tool_registry::ToolEntry {
                    name: #fn_name,
                    description: #desc_tokens,
                    schema: #schema_helper,
                    dispatch: #dispatch_helper,
                };
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #const_body
        #dispatch_fn
        #tool_registry_entry
        #input
    })
}
