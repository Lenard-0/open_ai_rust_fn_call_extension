use open_ai_rust_fn_call_extension::FunctionCall;

#[derive(FunctionCall)]
enum Bad {
    A,
    B(u32),
}

fn main() {}
