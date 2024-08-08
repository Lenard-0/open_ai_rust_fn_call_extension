extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, AttributeArgs, DeriveInput, FnArg, Ident, ItemFn, NestedMeta, Pat, Type};

#[proc_macro_derive(FunctionCallType)]
pub fn turn_type_to_function_call(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let name = &ast.ident;
    let fields = match &ast.data {
        syn::Data::Struct(syn::DataStruct { fields: syn::Fields::Named(syn::FieldsNamed { named, .. }), .. }) => named,
        _ => panic!("Only works for structs"),
    };

    let expanded_fields = fields.iter().map(|f| {
        let name = f.ident.as_ref().unwrap();
        let ty = &f.ty;
        quote! { #name: #ty }
    });

    // Convert the struct name to uppercase and ensure it's a valid identifier
    let uppercased_name = syn::Ident::new(&name.to_string().to_uppercase(), name.span());

    // Generate the constant declaration with the uppercased struct name
    let expanded_struct = quote! {
        pub const #uppercased_name: &'static str = concat!(stringify!(#(#expanded_fields),*));
    };

    TokenStream::from(expanded_struct)
}

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

    let description = opt_description.unwrap_or_default();

    // Extract function parameters
    let parameters = input.sig.inputs.iter().filter_map(|arg| {
        if let FnArg::Typed(pat_type) = arg {
            let param_name = match &*pat_type.pat {
                Pat::Ident(pat_ident) => pat_ident.ident.to_string(),
                _ => return None,
            };

            let param_type = match &*pat_type.ty {
                Type::Path(type_path) => type_path.path.segments.last().unwrap().ident.to_string(),
                _ => "unknown".to_string(),
            };

            Some((param_name, param_type))
        } else {
            None
        }
    }).collect::<Vec<_>>();

    let mut params_array: [&str; 100] = [""; 100];
    let mut params_vec = Vec::new();
    for (i, (name, _type)) in parameters.iter().enumerate() {
        let mut combined = name.clone();
        combined.push_str(": ");
        combined.push_str(_type);
        params_vec.push(combined.clone());
    }

    for (i, param) in params_vec.iter().enumerate() {
        params_array[i] = param;
    }

    let fn_name_uppercase = fn_name.to_uppercase();
    let fn_name_uppercase = Ident::new(&fn_name.to_uppercase(), Span::call_site());

    // Generate the FunctionCall struct with the function's name, description, and parameters
    let output = quote! {
        // Define the function call data
        const #fn_name_uppercase: FunctionCall<'static> = FunctionCall {
            name: #fn_name,
            description: #description,
            parameters: [
                #(
                    #params_array
                ),*
            ],
        };

        // Original function definition
        #input
    };

    TokenStream::from(output)
}
