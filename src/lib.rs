extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, Attribute, DeriveInput, FnArg, Ident, ItemFn, Meta, NestedMeta, Pat, Type};

#[proc_macro_derive(FunctionCallType)]
pub fn turn_type_to_function_call(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let name: &Ident = &ast.ident;
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
    let mut name = name.to_string();
    name.push_str(" { ");

    // Generate the constant declaration with the uppercased struct name
    let expanded_struct = quote! {
        pub const #uppercased_name: &'static str = concat!(#name, concat!(stringify!(#(#expanded_fields),*)));
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

    // Parse the attribute to extract the function description
    let description = parse_macro_input!(attr as syn::AttributeArgs)
        .iter()
        .find_map(|meta| match meta {
            NestedMeta::Lit(syn::Lit::Str(lit_str)) => Some(lit_str.value()),
            _ => None,
        })
        .unwrap_or_default();

    // Extract function parameters and their descriptions
    let parameters: Vec<(String, String, String)> = input.sig.inputs.iter().filter_map(|arg| {
        if let FnArg::Typed(pat_type) = arg {
            let param_name = match &*pat_type.pat {
                Pat::Ident(pat_ident) => pat_ident.ident.to_string(),
                _ => return None,
            };

            let param_type = match &*pat_type.ty {
                Type::Path(type_path) => type_path.path.segments.last().unwrap().ident.to_string(),
                _ => "unknown".to_string(),
            };

            // Extract custom attributes for the parameter
            let param_description = extract_parameter_description(&pat_type.attrs);

            Some((param_name, param_type, param_description))
        } else {
            None
        }
    }).collect();

    // Create a fixed-size array with 100 elements
    let mut parameters_formatted = vec![];
    for (name, _type, comment) in parameters {
        parameters_formatted.push(format!("{name}: {_type} ({comment})"))
    }
    let mut params_array: [&str; 100] = [""; 100];
    let mut i = 0;
    while i < parameters_formatted.len() {
        params_array[i] = &parameters_formatted[i];
        i += 1;
    }

    let fn_name_uppercase = Ident::new(&fn_name.to_uppercase(), Span::call_site());

    let output = quote! {
        // Define the function call data
        const #fn_name_uppercase: FunctionCallRaw<'static> = FunctionCallRaw {
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

fn extract_parameter_description(attrs: &[Attribute]) -> String {
    attrs.iter().find_map(|attr| {
        if attr.path.is_ident("function_call_description") {
            match attr.parse_meta().ok() {
                Some(Meta::NameValue(nv)) => {
                    if let syn::Lit::Str(lit_str) = nv.lit {
                        return Some(lit_str.value());
                    } else {
                        return None
                    }
                },
                _ => None,
            }
        } else {
            None
        }
    }).unwrap_or_default()
}