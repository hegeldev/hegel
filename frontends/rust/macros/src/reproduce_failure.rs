use proc_macro2::TokenStream;
use syn::ItemFn;

/// The `#[hegel::...]` attribute on `func` that `#[hegel::reproduce_failure]`
/// can be used with, if any.
fn host_attribute(func: &ItemFn) -> Option<&'static str> {
    func.attrs.iter().find_map(|attr| {
        let segments: Vec<_> = attr.path().segments.iter().collect();
        if segments.len() != 2 || segments[0].ident != "hegel" {
            return None;
        }
        match segments[1].ident.to_string().as_str() {
            "test" => Some("test"),
            "main" => Some("main"),
            "standalone_function" => Some("standalone_function"),
            _ => None,
        }
    })
}

/// This macro always produces a compile error when it actually runs.
///
/// In correct usage (`#[hegel::test]`, `#[hegel::main]` or
/// `#[hegel::standalone_function]` above, `#[hegel::reproduce_failure(...)]`
/// below), the attribute above processes first and consumes the
/// `reproduce_failure` attribute directly from `func.attrs`, so this macro
/// never executes.
///
/// If this macro DOES execute, it means either:
/// - Wrong order: `#[hegel::reproduce_failure(...)]` is above the attribute
/// - Bare function: none of those attributes at all
pub fn expand_reproduce_failure(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func: ItemFn = match syn::parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let message = match host_attribute(&func) {
        Some(host) => format!(
            "#[hegel::reproduce_failure(...)] must appear below #[hegel::{host}], not above it.\n\
             Write:\n  \
               #[hegel::{host}]\n  \
               #[hegel::reproduce_failure(\"<blob>\")]\n  \
               fn my_test(tc: ::hegel::TestCase) {{ ... }}"
        ),
        None => "#[hegel::reproduce_failure(...)] can only be used together with \
                 #[hegel::test], #[hegel::main] or #[hegel::standalone_function].\n\
                 Add one of them above #[hegel::reproduce_failure(...)]."
            .to_string(),
    };
    syn::Error::new_spanned(&func.sig, message).to_compile_error()
}
