
#[cfg(test)]
mod tests {
    // use open_ai_rust::logoi::input::tool::FunctionCall;
    use open_ai_rust_fn_call_extension::{function_call, FunctionCallType};
    // tests/function_call_tests.rs

    #[derive(Debug, PartialEq)]
    struct FunctionCallRaw<'a> {
        pub name: &'a str,
        pub description: &'a str,
        pub parameters: [&'a str; 100]
    }

    #[function_call("This function changes the light state.")]
    fn change_light(
        on: bool,
        extra_data: Arg
    ) {
        // Function body
    }

    pub struct Arg {
        pub name: String,
        pub value: String
    }

    // trait FunctionCallTrait {
    //     fn get_function_call() -> FunctionCall;
    // }

    #[test]
    fn test_function_call_attribute_macro() {
        let mut parameters = [""; 100];
        parameters[0] = "on: bool";
        parameters[1] = "extra_data: Arg";

        println!("FUNCTION_CALL {:?}", CHANGE_LIGHT);

        assert_eq!(CHANGE_LIGHT, FunctionCallRaw {
            name: "change_light",
            description: "This function changes the light state.",
            parameters: parameters
        });
    }


    #[test]
    fn test_expand_struct() {
        #[derive(FunctionCallType)]
        struct TestStruct {
            field1: u32,
            field2: String,
            field3: String,
        }

        // Define the expected output
        let expected_output = r#"TESTSTRUCT { field1 : u32, field2 : String, field3 : String"#;

        // Assert that the expanded code matches the expected output
        println!("TEST {:?}", TESTSTRUCT);
        assert_eq!(TESTSTRUCT, expected_output);
    }
}

