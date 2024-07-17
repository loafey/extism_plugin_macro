use proc_macro::TokenStream as TS;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_macro_input,
    punctuated::Punctuated,
    token::{Colon, Comma},
    FnArg, Ident, ItemTrait, Pat, PatIdent, PatType, ReturnType, Type,
};

fn get_export_functions(item: TS) -> TS {
    let input = parse_macro_input!(item as ItemTrait);
    let mut res = quote!();

    for item in input.items.iter().filter_map(|f| match f {
        syn::TraitItem::Fn(f) => Some(f),
        _ => None,
    }) {
        let sig = &item.sig;
        let mut args: Punctuated<_, Comma> = Punctuated::new();
        let macro_sig = {
            let mut sig = sig.clone();
            sig.ident = Ident::new(&format!("plugin_{}", sig.ident), Span::call_site());
            let ReturnType::Type(rl, mut ty) = sig.output else {
                panic!("expected return type!")
            };
            let Type::Path(p) = &mut *ty else {
                panic!("expected PluginResult type")
            };
            let p = &mut p.path.segments[0];
            p.ident = Ident::new("FnResult", Span::call_site());
            sig.output = ReturnType::Type(rl, ty);

            let params = sig
                .inputs
                .into_iter()
                .enumerate()
                .map(|(i, a)| {
                    let FnArg::Typed(mut pat) = a else {
                        panic!("no self types in argument")
                    };
                    let name = Ident::new(&format!("arg{i}"), Span::call_site());
                    pat.pat = Box::new(Pat::Ident(PatIdent {
                        attrs: Vec::new(),
                        by_ref: None,
                        mutability: None,
                        ident: name.clone(),
                        subpat: None,
                    }));
                    args.push(name);
                    FnArg::Typed(pat)
                })
                .collect::<Punctuated<_, Comma>>();
            sig.inputs = params;
            sig
        };
        let call = &sig.ident;
        res = quote! {
            #res

            #[plugin_fn]
            pub #macro_sig {
                match $name::#call(#args) {
                    Ok(o) => Ok(o),
                    Err(e) => Err(WithReturnCode(Error::msg(format!("{e}")), 1))
                }
            }
        };
    }

    quote! {
        #[macro_export]
        macro_rules! plugin_gen {
            ($name:ident) => {
                #res
            }
        }
    }
    .into()
}

fn get_plugin_calls(item: TS) -> TS {
    let input = parse_macro_input!(item as ItemTrait);
    let mut res = quote!();

    for item in input.items.iter().filter_map(|f| match f {
        syn::TraitItem::Fn(f) => Some(f),
        _ => None,
    }) {
        let sig = &item.sig;
        let mut args: Punctuated<_, Comma> = Punctuated::new();
        let macro_sig = {
            let mut sig = sig.clone();
            sig.ident = Ident::new(&format!("{}", sig.ident), Span::call_site());
            let ReturnType::Type(rl, mut ty) = sig.output else {
                panic!("expected return type!")
            };
            let Type::Path(p) = &mut *ty else {
                panic!("expected PluginResult type")
            };
            let p = &mut p.path.segments[0];
            p.ident = Ident::new("Result", Span::call_site());
            sig.output = ReturnType::Type(rl, ty);

            let mut params = sig
                .inputs
                .into_iter()
                .enumerate()
                .map(|(i, a)| {
                    let FnArg::Typed(mut pat) = a else {
                        panic!("no self types in argument")
                    };
                    let name = Ident::new(&format!("arg{i}"), Span::call_site());
                    pat.pat = Box::new(Pat::Ident(PatIdent {
                        attrs: Vec::new(),
                        by_ref: None,
                        mutability: None,
                        ident: name.clone(),
                        subpat: None,
                    }));
                    args.push(name);
                    FnArg::Typed(pat)
                })
                .collect::<Punctuated<_, Comma>>();
            params.insert(
                0,
                FnArg::Typed(PatType {
                    attrs: Vec::new(),
                    ty: Box::new(
                        syn::parse_str("&std::sync::Arc<tokio::sync::Mutex<extism::Plugin>>")
                            .unwrap(),
                    ),
                    pat: Box::new(Pat::Ident(PatIdent {
                        attrs: Vec::new(),
                        by_ref: None,
                        mutability: None,
                        ident: Ident::new("__plugin__", Span::call_site()),
                        subpat: None,
                    })),
                    colon_token: Colon {
                        spans: [Span::call_site()],
                    },
                }),
            );
            sig.inputs = params;
            sig
        };
        let args = if args.len() == 1 {
            quote! {#args}
        } else {
            quote! { () }
        };
        let call = format!("plugin_{}", sig.ident);
        res = quote! {
            #res

            pub async #macro_sig {
                let __plugin__ = __plugin__.clone();
                let res = tokio::task::spawn_blocking(
                    move || __plugin__.blocking_lock().call(#call, #args)
                ).await??;

                Ok(res)
            }
        };
    }

    quote! {
        #[macro_export]
        macro_rules! plugin_calls {
            () => {
                mod calls {
                    use extism::{*, convert::*};
                    use anyhow::Result;
                    use std::sync::Arc;
                    use tokio::sync::Mutex;
                    use super::*;
                    #res
                }
            }
        }
    }
    .into()
}

#[proc_macro_attribute]
pub fn plugin(_attr: TS, item: TS) -> TS {
    let res = TokenStream::from(get_export_functions(item.clone()));
    let calls = TokenStream::from(get_plugin_calls(item.clone()));
    let item = TokenStream::from(item);
    let res = quote! {
        #item
        #res
        #calls
    };

    res.into()
}
