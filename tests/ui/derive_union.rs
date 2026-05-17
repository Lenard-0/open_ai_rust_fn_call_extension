use open_ai_rust_fn_call_extension::FunctionCall;

#[derive(FunctionCall)]
union Bad {
    a: u32,
    b: u32,
}

fn main() {}
