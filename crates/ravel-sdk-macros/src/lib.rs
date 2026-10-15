#![forbid(unsafe_code)]

//! Mod identity attributes, re-exported by `ravel-sdk`.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::quote;
use ravel_core::ModId;
use syn::spanned::Spanned as _;
use syn::{Error, Expr, ExprLit, ImplItem, ItemImpl, Lit, LitStr, MetaNameValue, Result, parse2};

const MISSING_ID: &str = "`declare_mod` needs the mod's declared identity, as `id = \"example\"`";

const UNKNOWN_OPTION: &str = "`declare_mod` takes one option, `id = \"example\"`";

const ID_NOT_A_STRING: &str = "`declare_mod`'s `id` is a string literal, as `id = \"example\"`";

const ILLEGAL_ID: &str = "`declare_mod`'s `id` is not a legal mod id";

const NOT_AN_IMPL: &str =
    "`declare_mod` goes on the `impl Mod for` block that carries the mod's behaviour";

const INHERENT_IMPL: &str = "`declare_mod` goes on a trait `impl`, as `impl Mod for Example`, because what it generates is \
     that trait's `id` method";

const NOT_MOD: &str = "`declare_mod` goes on the `impl Mod for` block, because the `id` method it generates belongs \
     to that trait and to no other. This reads the trait path as written, and a macro cannot \
     resolve an import alias, so spell it `Mod` here even where it is in scope under another name";

const GENERATED_ID: &str = "`declare_mod` generates `id` from the identity it was given, so the block cannot also define \
     it";

const GENERIC_IMPL: &str = "`declare_mod` binds one identity to one concrete type, so the `impl` block takes no generic \
     parameters";

/// Binds an identity to an `impl Mod` block and generates its identity accessors.
///
/// ```ignore
/// #[ravel_sdk::declare_mod(id = "example")]
/// impl ravel_sdk::Mod for Example {
///     fn enter(&mut self, state: ravel_sdk::LifecycleState)
///         -> Result<(), ravel_sdk::ModError>
///     {
///         Ok(())
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn declare_mod(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand(attribute.into(), item.into())
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

// proc_macro2 keeps expansion testable outside a compiler invocation.
fn expand(attribute: Tokens, item: Tokens) -> Result<Tokens> {
    let id = parse_id(attribute)?;
    let mut block: ItemImpl =
        parse2(item).map_err(|_| Error::new(Span::call_site(), NOT_AN_IMPL))?;

    let Some((implemented, _)) = block.trait_.as_ref() else {
        return Err(Error::new(block.self_ty.span(), INHERENT_IMPL));
    };
    // Reject unrelated traits before generating an invalid `id` method.
    if implemented
        .segments
        .last()
        .is_none_or(|segment| segment.ident != "Mod")
    {
        return Err(Error::new(implemented.span(), NOT_MOD));
    }
    if !block.generics.params.is_empty() || block.generics.where_clause.is_some() {
        return Err(Error::new(block.generics.span(), GENERIC_IMPL));
    }
    if let Some(existing) = block.items.iter().find(|item| declares_id(item)) {
        return Err(Error::new(existing.span(), GENERATED_ID));
    }

    let self_ty = block.self_ty.clone();
    block.items.insert(
        0,
        parse2(quote! {
            fn id(&self) -> &::ravel_sdk::ModId {
                <Self as ::ravel_sdk::Declared>::declared_id()
            }
        })?,
    );

    Ok(quote! {
        impl ::ravel_sdk::Declared for #self_ty {
            /// # Panics
            ///
            /// Nothing an author can write reaches this panic: `declare_mod`
            /// parses the literal it was given at expansion time and refuses an
            /// illegal one at the call site, so this parses a string already
            /// known to be legal. It stays a panic because a mod cannot be
            /// validated, loaded, or attributed under an illegal identity, so
            /// there is nothing weaker for it to do.
            fn declared_id() -> &'static ::ravel_sdk::ModId {
                static ID: ::std::sync::LazyLock<::ravel_sdk::ModId> =
                    ::std::sync::LazyLock::new(|| {
                        #id.parse().expect("`declare_mod` was given a legal mod id")
                    });
                &ID
            }
        }

        #block
    })
}

fn parse_id(attribute: Tokens) -> Result<LitStr> {
    if attribute.is_empty() {
        return Err(Error::new(Span::call_site(), MISSING_ID));
    }

    let option: MetaNameValue =
        parse2(attribute).map_err(|error| Error::new(error.span(), UNKNOWN_OPTION))?;
    if !option.path.is_ident("id") {
        return Err(Error::new(option.path.span(), UNKNOWN_OPTION));
    }

    match option.value {
        Expr::Lit(ExprLit {
            lit: Lit::Str(id), ..
        }) => match id.value().parse::<ModId>() {
            Ok(_) => Ok(id),
            Err(error) => Err(Error::new(id.span(), format!("{ILLEGAL_ID}: {error}"))),
        },
        other => Err(Error::new(other.span(), ID_NOT_A_STRING)),
    }
}

fn declares_id(item: &ImplItem) -> bool {
    matches!(item, ImplItem::Fn(function) if function.sig.ident == "id")
}

#[cfg(test)]
mod tests {
    use proc_macro2::TokenStream as Tokens;
    use quote::quote;

    use super::{
        GENERATED_ID, GENERIC_IMPL, ID_NOT_A_STRING, ILLEGAL_ID, INHERENT_IMPL, MISSING_ID, ModId,
        NOT_AN_IMPL, NOT_MOD, UNKNOWN_OPTION, expand,
    };

    // Compare token text without depending on whitespace.
    fn expanded(attribute: Tokens, item: Tokens) -> String {
        expand(attribute, item)
            .expect("the expansion should succeed")
            .to_string()
    }

    fn refusal(attribute: Tokens, item: Tokens) -> String {
        expand(attribute, item)
            .expect_err("the expansion should be refused")
            .to_string()
    }

    #[test]
    fn the_generated_method_reads_the_identity_the_attribute_was_given() {
        let expansion = expanded(
            quote! { id = "example" },
            quote! {
                impl Mod for Example {}
            },
        );

        assert!(
            expansion.contains("impl :: ravel_sdk :: Declared for Example"),
            "{expansion}"
        );
        assert!(expansion.contains("\"example\" . parse ()"), "{expansion}");
        assert!(
            expansion.contains("fn id (& self) -> & :: ravel_sdk :: ModId"),
            "{expansion}"
        );
        assert!(
            expansion.contains("< Self as :: ravel_sdk :: Declared > :: declared_id ()"),
            "{expansion}"
        );
    }

    #[test]
    fn the_block_the_author_wrote_survives_the_expansion() {
        let expansion = expanded(
            quote! { id = "example" },
            quote! {
                impl Mod for Example {
                    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
                        self.seen.push(event);
                        Ok(())
                    }
                }
            },
        );

        assert!(
            expansion.contains("self . seen . push (event)"),
            "{expansion}"
        );
        assert!(expansion.contains("impl Mod for Example"), "{expansion}");
    }

    #[test]
    fn a_qualified_trait_path_and_a_pathed_type_both_expand() {
        let expansion = expanded(
            quote! { id = "example" },
            quote! {
                impl ravel_sdk::Mod for crate::mods::Example {}
            },
        );

        assert!(
            expansion.contains("impl :: ravel_sdk :: Declared for crate :: mods :: Example"),
            "{expansion}"
        );
    }

    #[test]
    fn an_attribute_with_nothing_in_it_names_what_it_needs() {
        assert_eq!(
            refusal(Tokens::new(), quote! { impl Mod for Example {} }),
            MISSING_ID
        );
    }

    #[test]
    fn an_option_that_is_not_the_identity_is_refused() {
        assert_eq!(
            refusal(
                quote! { name = "example" },
                quote! { impl Mod for Example {} }
            ),
            UNKNOWN_OPTION
        );
        assert_eq!(
            refusal(quote! { id }, quote! { impl Mod for Example {} }),
            UNKNOWN_OPTION
        );
    }

    #[test]
    fn an_identity_that_is_not_a_string_literal_is_refused() {
        assert_eq!(
            refusal(quote! { id = 9 }, quote! { impl Mod for Example {} }),
            ID_NOT_A_STRING
        );
        assert_eq!(
            refusal(quote! { id = EXAMPLE }, quote! { impl Mod for Example {} }),
            ID_NOT_A_STRING
        );
    }

    #[test]
    fn an_identity_the_mod_id_grammar_refuses_is_refused_at_expansion() {
        // The grammar is `ravel-core`'s, called rather than restated, so these
        // are cases `ModId`'s parser rejects and not a second list.
        for illegal in ["Not A Legal Id", "", "1st", "example__mod", "example-"] {
            let refusal = refusal(quote! { id = #illegal }, quote! { impl Mod for Example {} });
            assert!(refusal.starts_with(ILLEGAL_ID), "{illegal:?}: {refusal}");
        }
    }

    #[test]
    fn the_refusal_of_an_illegal_identity_quotes_the_grammar_that_refused_it() {
        let refusal = refusal(
            quote! { id = "Not A Legal Id" },
            quote! { impl Mod for Example {} },
        );
        let reported = "Not A Legal Id"
            .parse::<ModId>()
            .expect_err("the grammar refuses a capital letter")
            .to_string();

        assert_eq!(refusal, format!("{ILLEGAL_ID}: {reported}"));
    }

    #[test]
    fn the_attribute_refuses_anything_that_is_not_an_impl_block() {
        assert_eq!(
            refusal(quote! { id = "example" }, quote! { struct Example; }),
            NOT_AN_IMPL
        );
        assert_eq!(
            refusal(quote! { id = "example" }, quote! { fn example() {} }),
            NOT_AN_IMPL
        );
    }

    #[test]
    fn an_inherent_impl_is_refused_because_it_would_generate_a_method_no_trait_wants() {
        assert_eq!(
            refusal(quote! { id = "example" }, quote! { impl Example {} }),
            INHERENT_IMPL
        );
    }

    #[test]
    fn an_impl_of_some_other_trait_is_refused_by_name_rather_than_by_the_method_it_would_insert() {
        assert_eq!(
            refusal(
                quote! { id = "example" },
                quote! { impl fmt::Display for Example {} }
            ),
            NOT_MOD
        );
        assert_eq!(
            refusal(
                quote! { id = "example" },
                quote! { impl Default for Example {} }
            ),
            NOT_MOD
        );
    }

    #[test]
    fn the_trait_named_under_an_import_alias_is_refused_and_the_message_says_why() {
        // `use ravel_sdk::Mod as M; impl M for Example` does implement `Mod`, and
        // this refuses it anyway because expansion happens before name
        // resolution. The message has to admit that rather than tell the author
        // to implement the trait they already implemented.
        assert_eq!(
            refusal(quote! { id = "example" }, quote! { impl M for Example {} }),
            NOT_MOD
        );
        assert!(NOT_MOD.contains("import alias"), "{NOT_MOD}");
    }

    #[test]
    fn a_generic_impl_is_refused_because_one_identity_cannot_name_every_instantiation() {
        assert_eq!(
            refusal(
                quote! { id = "example" },
                quote! { impl<T> Mod for Example<T> {} }
            ),
            GENERIC_IMPL
        );
        assert_eq!(
            refusal(
                quote! { id = "example" },
                quote! { impl Mod for Example where Example: Send {} }
            ),
            GENERIC_IMPL
        );
    }

    #[test]
    fn a_block_that_writes_its_own_identity_method_is_refused_rather_than_overridden() {
        assert_eq!(
            refusal(
                quote! { id = "example" },
                quote! {
                    impl Mod for Example {
                        fn id(&self) -> &ModId {
                            &self.id
                        }
                    }
                }
            ),
            GENERATED_ID
        );
    }
}
