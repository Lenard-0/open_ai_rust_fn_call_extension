
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

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
    fn _change_light(
        _on: bool,
        _extra_data: _Arg
    ) {
        // Function body
    }

    pub struct _Arg {
        pub name: String,
        pub value: String
    }

    // trait FunctionCallTrait {
    //     fn get_function_call() -> FunctionCall;
    // }

    #[test]
    fn test_function_call_attribute_macro() {
        let mut parameters = [""; 100];
        parameters[0] = "_on: bool";
        parameters[1] = "_extra_data: _Arg";
        println!("FUNCTION_CALL {:?}", _CHANGE_LIGHT);
        assert_eq!(_CHANGE_LIGHT, FunctionCallRaw {
            name: "_change_light",
            description: "This function changes the light state.",
            parameters: parameters
        });
    }


    #[test]
    fn test_expand_struct() {
        #[derive(FunctionCallType)]
        struct _TestStruct {
            field1: u32,
            field2: String,
            field3: String,
        }
        let expected_output = r#"_TestStruct { field1 : u32, field2 : String, field3 : String"#;
        println!("TEST {:?}", _TESTSTRUCT);
        assert_eq!(_TESTSTRUCT, expected_output);
    }

    #[test]
    fn test_expand_struct_w_vec() {
        #[derive(FunctionCallType)]
        struct _VecStruct {
            field1: Vec<String>
        }
        let expected_output = r#"_VecStruct { field1 : Vec < String >"#;
        println!("VECSTRUCT {:?}", _VECSTRUCT);
        assert_eq!(_VECSTRUCT, expected_output);
    }

    #[test]
    fn test_expand_struct_w_hashmap() {
        #[derive(FunctionCallType)]
        struct _HashMapStruct {
            obj: HashMap<String, String>
        }
        let expected_output = r#"_HashMapStruct { obj : HashMap < String, String >"#;
        println!("HASHMAPSTRUCT {:?}", _HASHMAPSTRUCT);
        assert_eq!(_HASHMAPSTRUCT, expected_output);
    }

    #[test]
    fn test_expand_struct_w_vec_wrapping_hashmap() {
        #[derive(FunctionCallType)]
        struct _VecHashMapStruct {
            objs: Vec<HashMap<String, String>>
        }
        let expected_output = r#"_VecHashMapStruct { objs : Vec < HashMap < String, String > >"#;
        println!("HASHMAPSTRUCT {:?}", _VECHASHMAPSTRUCT);
        assert_eq!(_VECHASHMAPSTRUCT, expected_output);
    }
}

