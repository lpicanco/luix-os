//! Audited boot entry macro. Expansion emits the entry symbol and a safe function call.
#![forbid(unsafe_code)]

use proc_macro::{TokenStream, TokenTree};

/// Export a diverging function with no arguments as the boot entry point.
/// The Rust type check enforces the signature. The caller still initializes the core.
#[proc_macro_attribute]
pub fn entry(args: TokenStream, item: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return "compile_error!(\"entry takes no arguments\");"
            .parse()
            .unwrap();
    }
    let mut tokens = item.clone().into_iter();
    let name = loop {
        match tokens.next() {
            Some(TokenTree::Ident(i)) if i.to_string() == "fn" => {
                break match tokens.next() {
                    Some(TokenTree::Ident(name)) => name.to_string(),
                    _ => return "compile_error!(\"expected a function\");".parse().unwrap(),
                };
            }
            Some(_) => {}
            None => return "compile_error!(\"expected a function\");".parse().unwrap(),
        }
    };
    let mut output = item;
    output.extend(
        format!(
            "#[doc(hidden)] #[unsafe(no_mangle)] pub extern \"C\" fn _start() -> ! {{
            let start: fn() -> ! = {name}; start()
        }}"
        )
        .parse::<TokenStream>()
        .unwrap(),
    );
    output
}
