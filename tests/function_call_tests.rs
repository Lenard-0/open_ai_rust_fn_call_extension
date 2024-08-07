// use open_ai_rust::logoi::input::tool::FunctionCall;
use open_ai_rust_fn_call_extension::function_call;
// tests/function_call_tests.rs

#[derive(Debug, PartialEq)]
struct FunctionCall<'a> {
    pub name: &'a str,
    pub description: &'a str,
    // pub parameters: Vec<FunctionParameter>,
}

#[function_call("This function changes the light state.")]
fn change_light(on: bool) {
    // Function body
}

// trait FunctionCallTrait {
//     fn get_function_call() -> FunctionCall;
// }

#[test]
fn test_function_call_attribute_macro() {
    assert_eq!(FUNCTION_CALL, FunctionCall {
        name: "change_light",
        description: "This function changes the light state."
    });
}