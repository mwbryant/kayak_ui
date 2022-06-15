extern crate proc_macro;

mod function_component;
mod tags;

mod attribute;
mod child;
mod children;
mod partial_eq;
mod use_effect;
mod widget;
mod widget_attributes;
mod widget_builder;
mod widget_props;

use function_component::WidgetArguments;
use partial_eq::impl_dyn_partial_eq;
use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use quote::quote;
use syn::{parse_macro_input, parse_quote};
use use_effect::UseEffect;
use widget::ConstructedWidget;

use crate::widget::Widget;
use crate::widget_props::impl_widget_props;

/// A top level macro that works the same as [`rsx`] but provides some additional
/// context for building the root widget.
#[proc_macro]
#[proc_macro_error]
pub fn render(input: TokenStream) -> TokenStream {
    let widget = parse_macro_input!(input as Widget);

    let kayak_core = get_core_crate();

    let result = quote! {
        let mut context = #kayak_core::KayakContextRef::new(context, None);
        let parent_id: Option<#kayak_core::Index> = None;
        let children: Option<#kayak_core::Children> = None;
        #widget
        context.commit();
    };

    TokenStream::from(result)
}

/// A proc macro that turns RSX syntax into structure constructors and calls the
/// context to create the widgets.
#[proc_macro]
#[proc_macro_error]
pub fn rsx(input: TokenStream) -> TokenStream {
    let widget = parse_macro_input!(input as Widget);
    let result = quote! { #widget };
    TokenStream::from(result)
}

/// A proc macro that turns RSX syntax into structure constructors only.
#[proc_macro]
#[proc_macro_error]
pub fn constructor(input: TokenStream) -> TokenStream {
    let el = parse_macro_input!(input as ConstructedWidget);
    let widget = el.widget;
    let result = quote! { #widget };
    TokenStream::from(result)
}

/// This attribute macro is what allows Rust functions to be generated into
/// valid widgets structs.
///
/// # Examples
///
/// ```
/// #[widget]
/// fn MyWidget() { /* ... */ }
/// ```
#[proc_macro_attribute]
#[proc_macro_error]
pub fn widget(args: TokenStream, item: TokenStream) -> TokenStream {
    let mut widget_args = WidgetArguments::default();
    if !args.is_empty() {
        // Parse stuff..
        let parsed = args.to_string();
        widget_args.focusable = parsed.contains("focusable");
    }

    let f = parse_macro_input!(item as syn::ItemFn);
    function_component::create_function_widget(f, widget_args)
}

/// A derive macro for the `WidgetProps` trait
#[proc_macro_derive(WidgetProps, attributes(prop_field))]
#[proc_macro_error]
pub fn derive_widget_props(item: TokenStream) -> TokenStream {
    impl_widget_props(item)
}

#[proc_macro_derive(DynPartialEq)]
pub fn dyn_partial_eq_macro_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_dyn_partial_eq(&ast)
}

#[proc_macro_attribute]
pub fn dyn_partial_eq(_: TokenStream, input: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(input as syn::ItemTrait);

    let name = &input.ident;

    let bound: syn::TypeParamBound = parse_quote! {
      DynPartialEq
    };

    input.supertraits.push(bound);

    (quote! {
      #input

      impl core::cmp::PartialEq for Box<dyn #name> {
        fn eq(&self, other: &Self) -> bool {
          self.box_eq(other.as_any())
        }
      }
    })
    .into()
}

/// Register some state data with an initial value.
///
/// Once the state is created, this macro returns the current value, a closure for updating the current value, and
/// the raw Binding in a tuple.
///
/// For more details, check out [React's documentation](https://reactjs.org/docs/hooks-state.html),
/// upon which this macro is based.
///
/// # Arguments
///
/// * `initial_state`: The initial value for the state
///
/// returns: (state, set_state, state_binding)
///
/// # Examples
///
/// ```
/// # use kayak_core::{EventType, OnEvent};
/// # use kayak_render_macros::use_state;
///
/// let (count, set_count, ..) = use_state!(0);
///
/// let on_event = OnEvent::new(move |_, event| match event.event_type {
///         EventType::Click(..) => {
///             set_count(foo + 1);
///         }
///         _ => {}
/// });
///
/// rsx! {
///         <>
///             <Button on_event={Some(on_event)}>
///                 <Text size={16.0} content={format!("Count: {}", count)}>{}</Text>
///             </Button>
///         </>
///     }
/// ```
#[proc_macro]
pub fn use_state(initial_state: TokenStream) -> TokenStream {
    let initial_state = parse_macro_input!(initial_state as syn::Expr);
    let kayak_core = get_core_crate();

    let result = quote! {{
        use #kayak_core::{Bound, MutableBound};
        let state = context.create_state(#initial_state).unwrap();
        let cloned_state = state.clone();
        let set_state = move |value| {
            cloned_state.set(value);
        };

        let state_value = state.get();

        (state.get(), set_state, state)
    }};
    TokenStream::from(result)
}

/// Registers a side-effect callback for a given set of dependencies.
///
/// This macro takes on the form: `use_effect!(callback, dependencies)`. The callback is
/// the closure that's ran whenever one of the Bindings in the dependencies array is changed.
///
/// Dependencies are automatically cloned when added to the dependency array. This allows the
/// original bindings to be used within the callback without having to clone them manually first.
/// This can be seen in the example below where `count_state` is used within the callback and in
/// the dependency array.
///
/// For more details, check out [React's documentation](https://reactjs.org/docs/hooks-effect.html),
/// upon which this macro is based.
///
/// # Arguments
///
/// * `callback`: The side-effect closure
/// * `dependencies`: The dependency array (in the form `[dep_1, dep_2, ...]`)
///
/// returns: ()
///
/// # Examples
///
/// ```
/// # use kayak_core::{EventType, OnEvent};
/// # use kayak_render_macros::{use_effect, use_state};
///
/// let (count, set_count, count_state) = use_state!(0);
///
/// use_effect!(move || {
///     println!("Count: {}", count_state.get());
/// }, [count_state]);
///
/// let on_event = OnEvent::new(move |_, event| match event.event_type {
///         EventType::Click(..) => {
///             set_count(foo + 1);
///         }
///         _ => {}
/// });
///
/// rsx! {
///         <>
///             <Button on_event={Some(on_event)}>
///                 <Text size={16.0} content={format!("Count: {}", count)} />
///             </Button>
///         </>
///     }
/// ```
#[proc_macro]
pub fn use_effect(input: TokenStream) -> TokenStream {
    let effect = parse_macro_input!(input as UseEffect);
    effect.build()
}

/// Helper method for getting the core crate
///
/// Depending on the usage of the macro, this will become `crate`, `kayak_core`,
/// or `kayak_ui::core`.
///
/// # Examples
///
/// ```
/// fn my_macro() -> proc_macro2::TokenStream {
///   let kayak_core = get_core_crate();
///   quote! {
///     let foo = #kayak_core::Foo;
///   }
/// }
/// ```
fn get_core_crate() -> proc_macro2::TokenStream {
    let found_crate = proc_macro_crate::crate_name("kayak_core");
    if let Ok(found_crate) = found_crate {
        match found_crate {
            proc_macro_crate::FoundCrate::Itself => quote! { crate },
            proc_macro_crate::FoundCrate::Name(name) => {
                let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
                quote!(#ident)
            }
        }
    } else {
        quote!(kayak_ui::core)
    }
}
