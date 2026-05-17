// Unknown container-level `function_call(...)` attribute argument.
// Exercises the Err arm of `parse_container_opts`.
use open_ai_rust_fn_call_extension::FunctionCall;

#[derive(FunctionCall)]
#[function_call(bogus_container_attr)]
struct Bad {
    a: u32,
}

fn main() {}
