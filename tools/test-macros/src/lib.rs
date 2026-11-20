use proc_macro::{TokenStream, TokenTree};

#[proc_macro_attribute]
pub fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    let attributes = if args.is_empty() {
        r#"#[cfg_attr(target_os = "none", test_case)]
           #[cfg_attr(not(target_os = "none"), ::core::prelude::v1::test)]"#
    } else {
        let mut args = args.into_iter();
        let is_qemu = matches!(args.next(), Some(TokenTree::Ident(key)) if key.to_string() == "target")
            && matches!(args.next(), Some(TokenTree::Punct(eq)) if eq.as_char() == '=')
            && matches!(args.next(), Some(TokenTree::Literal(value)) if value.to_string() == r#""qemu""#)
            && args.next().is_none();
        if !is_qemu {
            return r#"compile_error!("expected #[test] or #[test(target = \"qemu\")]");"#
                .parse()
                .unwrap();
        }
        r#"#[cfg(target_os = "none")] #[test_case]"#
    };

    let mut output: TokenStream = attributes.parse().unwrap();
    output.extend(item);
    output
}
