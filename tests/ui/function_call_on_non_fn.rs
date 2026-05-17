// Malformed positional attribute argument (`;` is not an expression).
// Forces `parse_attr_description` to return Err, exercising the
// `Err(e) => e.to_compile_error()` arm of `pub fn function_call(...)`.
use open_ai_rust_fn_call_extension::function_call;

#[function_call(;)]
fn boom(_a: i64) {}

fn main() {}
