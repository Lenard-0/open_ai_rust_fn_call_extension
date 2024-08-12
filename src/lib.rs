extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_macro_input, Attribute, AttributeArgs, Data, DataStruct, DeriveInput, Fields, FieldsNamed, FnArg, Ident, ItemFn, Meta, NestedMeta, Pat, Type};

#[proc_macro_derive(FunctionCall)]
pub fn turn_type_to_function_call(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let out = match input.data {
        Data::Struct(s) => {
            let fields = s.fields.clone().into_iter().map(|field| field.ident.unwrap());
            let fields2 = s.fields.into_iter().map(|field| field.ident.unwrap());
            quote! {
                impl open_ai_rust::logoi::input::tool::raw_macro::FunctionCallable for #name {
                    fn to_fn_call(&self) -> FunctionCall {
                        FunctionCall {
                            name: String::from(stringify!(#name)),
                            description: None,
                            parameters: vec![
                                #(
                                    FunctionParameter {
                                        name: stringify!(#fields),
                                        _type: open_ai_rust::logoi::input::tool::raw_macro::FunctionCallable::to_fn_type(&self.#fields),
                                        description: None
                                    },
                                )*
                            ]
                        }
                    }

                    fn to_fn_type(&self) -> FunctionType {
                        FunctionType::Object(vec![
                            #(
                                FunctionParameter {
                                    name: stringify!(#fields2),
                                    _type: open_ai_rust::logoi::input::tool::raw_macro::FunctionCallable::to_fn_type(&self.#fields2),
                                    description: None
                                },
                            )*
                        ])
                    }
                }
            }
        },
        Data::Enum(e) => {
            let variants = e.variants.into_iter().map(|variant| variant.ident);
            quote! {
                impl open_ai_rust::logoi::input::tool::raw_macro::FunctionCallable for #name {
                    fn to_fn_call(&self) -> FunctionCall {
                        match &self {
                            #(Self::#variants => format!("\"{}\"", stringify!(#variants)) ),*
                        }
                    }
                }
            }
        },
        _ => todo!(),
    };

    out.into()
}



// FUNCTION ATTR BELOW

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