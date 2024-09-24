use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, FnArg, Ident, ItemFn, Pat, Type};

#[proc_macro_attribute]
pub fn opcode(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut function_item = parse_macro_input!(input as ItemFn);

    let mut signature = function_item.sig.to_owned();

    let mut register_names = vec![];
    let mut variable_names = vec![];
    let mut variable_refs = vec![];
    let mut variable_muts = vec![];

    let mut member_names = vec![];
    let mut member_types = vec![];

    for argument in &mut signature.inputs {
        if let FnArg::Typed(ref mut argument) = argument {
            if let Pat::Ident(variable_name) = argument.pat.as_ref() {
                if let Some(attribute) = argument.attrs.first() {
                    if let Some(register_name) = attribute.path().get_ident() {
                        register_names.push(register_name.clone());
                        variable_names.push(variable_name.ident.clone());
                        if let Type::Reference(reference) = argument.ty.as_ref() {
                            variable_refs.push(Some(reference.and_token));
                            variable_muts.push(reference.mutability);
                        } else {
                            variable_refs.push(None);
                            variable_muts.push(None);
                        }
                    }
                } else {
                    member_names.push(variable_name.ident.clone());
                    member_types.push(*argument.ty.clone());
                }
            }
        }
    }

    function_item.sig = signature;

    let body = function_item.block;
    let function_name = function_item.sig.ident;

    let struct_name: String = function_name
        .to_string()
        .split("_")
        .map(|x| {
            let mut part = x.to_owned();
            part.get_mut(0..1).unwrap().make_ascii_uppercase();
            part
        })
        .fold(String::new(), |a, b| a + &b);
    let struct_name = Ident::new(&struct_name, Span::call_site());

    quote! {
        pub struct #struct_name {
            #(#member_names: #member_types),*
        }

        impl #struct_name {
            pub fn new(#(#member_names: #member_types),*) -> Self {
                Self {#(
                    #member_names
                ),*}
            }
        }

        impl crab8_core::OpCode for #struct_name {
            #[inline(always)]
            fn apply(&self, state: &mut crab8_core::State) {
                #(let #variable_names = #variable_refs #variable_muts state.#register_names;)*
                #(let #member_names = self.#member_names;)*

                #body;
            }
        }
    }
    .into()
}
