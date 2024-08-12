extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_macro_input, Attribute, AttributeArgs, Data, DataStruct, DeriveInput, Fields, FieldsNamed, FnArg, Ident, ItemFn, Meta, NestedMeta, Pat, Type};

#[proc_macro_derive(FunctionCallable)]
pub fn turn_type_to_function_call(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let out = match input.data {
        Data::Struct(s) => {
            let fields = s.fields.into_iter().map(|field| field.ident.unwrap());
            quote! {
                impl open_ai_rust::FunctionCallable for #name {
                    fn to_json(&self) -> String {
                        let mut json = "{ ".to_string();
                        #(
                            json.push_str(&format!("\"{}\": {}, ", stringify!(#fields), open_ai_rust::FunctionCallable::to_fn_call(&self.#fields)));
                        )*
                        json.remove(json.len() - 2); // remove trailling comma
                        json.push('}');
                        json
                    }
                }
            }
        },
        Data::Enum(e) => {
            let variants = e.variants.into_iter().map(|variant| variant.ident);
            quote! {
                impl open_ai_rust::FunctionCallable for #name {
                    fn to_json(&self) -> String {
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

// fn generate_struct_representation(ast: &DeriveInput) -> TokenStream2 {
//     let name = &ast.ident;
//     let fields = match &ast.data {
//         Data::Struct(DataStruct { fields: Fields::Named(FieldsNamed { named, .. }), .. }) => named,
//         _ => panic!("Only works for structs"),
//     };

//     let expanded_fields = fields.iter().map(|f| {
//         let field_name = &f.ident;
//         let ty = &f.ty;
//         let field_repr = match ty {
//             Type::Path(path) => {
//                 if let Some(ident) = path.path.get_ident() {
//                     // Recursively process nested structs
//                     let nested_name = format_ident(&ident.to_string().to_uppercase());
//                     quote! { #field_name: { #nested_name } }
//                 } else {
//                     quote! { #field_name: #ty }
//                 }
//             },
//             _ => quote! { #field_name: #ty },
//         };
//         field_repr
//     });

//     let uppercased_name = syn::Ident::new(&name.to_string().to_uppercase(), name.span());

//     quote! {
//         pub const #uppercased_name: &'static str = concat!(stringify!(#name { #(#expanded_fields),* }));
//     }
// }

fn format_ident(s: &str) -> syn::Ident {
    syn::Ident::new(s, proc_macro2::Span::call_site())
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