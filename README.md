# open_ai_rust_fn_extensions
Macros for Open AI function calling. Designed specifically to be used with the open_ai_rust crate for syntactic sugar which I also developed.
That crate can convert it into working function calls and that README should be used as reference.

Example:
#[function_call("This function changes the light state.")]
fn change_light(on: bool, extra_data: Arg) {
    // Function body
}

Becomes this:
FUNCTION_CALL FunctionCall { name: "change_light", description: "This function changes the light state.", parameters: ["on: bool", "extra_data: Arg", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""] }

I am currently working on this overtime to make it fully comprehensive and impressive.