extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, AttributeArgs, ItemFn, NestedMeta};

#[proc_macro_attribute]
pub fn function_call(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = match syn::parse_macro_input::parse::<ItemFn>(item) {
        Ok(data) => data,
        Err(err) => return err.to_compile_error().into(),
    };

    // Extract the function name
    let function_name = input.sig.ident.to_string();
    let fn_name = function_name.as_str();

    // Parse the attribute to extract the description
    let opt_description = parse_macro_input!(attr as AttributeArgs)
        .iter()
        .find_map(|meta| match meta {
            NestedMeta::Lit(syn::Lit::Str(lit_str)) => Some(lit_str.value()),
            _ => None,
        });

    let description: &str = match &opt_description {
        Some(description) => description,
        None => ""
    };

    // Generate the FunctionCall struct with the function's name and description

    let output = quote! {
        // Generate the FunctionCall struct with the function's name and description
        const FUNCTION_CALL: FunctionCall = FunctionCall {
            name: #fn_name,
            description: #description
        };

        // Original function definition
        #input
    };

    TokenStream::from(output)
}
