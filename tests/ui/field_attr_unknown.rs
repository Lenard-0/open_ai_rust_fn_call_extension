use open_ai_rust_fn_call_extension::FunctionCall;

#[derive(FunctionCall)]
struct Bad {
    #[function_call(bogus)]
    a: u32,
}

fn main() {}
