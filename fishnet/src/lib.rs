//! fishnet is a opinionated, performant web framework for small projects and personal websites where [htmx](https://htmx.org/) is a first class citizen.
//!
//! # overview
//! fishnet aims to provide abstractions for splitting your page into components,
//! while trying to stay as close as possible to the served content. no virtual dom, giant javascript bundles or complex build steps.
//! in doing so, fishnet makes a few core assumptions about your project:
//! - the initially served content is *(mostly)* static. you can serve dynamically rendered content, but it comes with a few drawbacks (see [the section on dynamic components](#dynamic-components))
//! - you want to split your page into components, but don't want to deal with the complexity of a full blown frontend framework.
//! - you want full control over the html, css and javascript that is served to the client.
//! - and most importantly: you are fine with using a library that probably isn't going to get a lot of maintenance.
//!
//! # getting started
//! ## creating a page
//! to get started with fishnet, you need to create a new [`Website`] and add a [`Page`] to it.
//! you can then use the [`html!`](html) (see the [maud documentation](https://maud.lambda.xyz/) for more info) macro to create the content of the page.
//! ```rust,no_run
//! use futures::future::FutureExt;
//! use fishnet::{Website, Page, html};
//!
//! #[tokio::main] // feel free to use any async runtime you like
//! async fn main() {
//!     // create a page
//!     let home_page = Page::new("home").with_body(|| async {
//!         html! {
//!             h1 { "Hello, World!" }
//!         }
//!     }.boxed());
//!
//!     // create a new website
//!     let website = Website::new()
//!         // add a new 'home' page to the website, accessible at '/'
//!         .add_page("/", home_page)
//!         .await;
//!
//!    // serve the website on port 8080
//!     website.serve(8080).await;
//! }
//! ```
//!
//! ### using components
//! lets say you've added a button to your page:
//! ```rust
//! use futures::future::FutureExt;
//! use fishnet::{Page, html};
//!
//! Page::new("example").with_body(|| async {
//!    html! {
//!       button onclick="alert('hello!')" { "click me!" }
//!   }
//! }.boxed());
//! ```
//!
//! you have now decided to reuse this button multiple times.
//! you can achieve this by creating a new component:
//! ```rust
//! use fishnet::{
//!   Page,
//!   // the component prelude contains a lot of useful imports for creating components
//!   component::prelude::*,
//! };
//!
//! fn my_awesome_button(label: &str, alert: &str) -> impl BuildableComponent {
//!     // convert the str references to owned strings
//!     let state = (
//!         label.to_string(),
//!         format!("alert('{}')", alert)
//!     );
//!
//!     // create a new 'button' component
//!     component!(Button)
//!     //add our parameters as state to the component
//!     .with_state(Arc::new(state))
//!     // render the button
//!     .render(|state| async move {
//!         html! {
//!            button onclick=(state.1) { (state.0) }
//!        }
//!     }.boxed())
//! }
//!
//! Page::new("example").with_body(|| async {
//!    // use the component
//!   html! {
//!     (c!(my_awesome_button("click me!", "hello!")))
//!     (c!(my_awesome_button("click me too!", "goodbye!")))
//!   }
//! }.boxed());
//! ```
//! let's break down what's happening here:
//! the `my_awesome_button` function is a function that constructs a new component with the contents of the button.
//! since the render function may run multiple times, we need to convert the passed references to owned strings and `move` them into the closure.
//!
//! on the page itself, we use the `c!` macro to add the component to the page. this handles all the behind the scenes work of building and rendering the component, and caching it for future use.
//! in this scenario, the `my_awesome_button` function and the components render function are both run exactly once over the lifetime of the whole page, even if the page is visited multiple times.
//! (this may not always be the case, see [the section on dynamic components](#dynamic-components) for more info.)
//!
//! ### components vs html! in functions (or: why do i need to use a component at all?)
//! you might be wondering why using components is better than just something like this:
//! ```rust
//! use futures::future::FutureExt;
//! use fishnet::{Page, html, Markup};
//!
//! fn my_awesome_button(label: &str, alert: &str) -> Markup {
//!     html! {
//!        button onclick={"alert("(alert)")"} { (label) }
//!     }
//! }
//!
//! Page::new("example").with_body(|| async {
//!    html! {
//!       (my_awesome_button("click me!", "hello!"))
//!       (my_awesome_button("click me too!", "goodbye!"))
//!   }
//! }.boxed());
//! ```
//! while this is indeed much simpler, and might even be desirable for such a simple case,
//! you lose out to three things you get from using a component:
//! - [api routes](#htmx) - each component gets its own api route, which can be used for serving dynamic content or handling form submissions.
//! - cached rendering - (static) components are only rendered once and then cached. using a
//! function, you leave the caching up to the parent component. if you use a function at the top level it gets rerendered on every page visit.
//! - [state](#dynamic-components) - components can have state, which can be used to store data over the lifetime of the component.
//!  
//! this means, that in the end the tradeoff is up to the use-case. using a simple function as a component is usually fine (and even recommended!) for the smaller parts of your page.
//! because of this, fishnet also provides the [`style!`] and [`script!`] macros for adding component independent css and javascript to your functions.
//! these work mostly identical to their [component counterparts](#styling)
//!
//! ## dynamic components
//! the components you used until now were all statically rendered. this means, that their
//! contents are fixed after the initial render. sometimes you want your components content to
//! change each time the user visits the page however. this is where dynamic components come into
//! play. imagine you want to add a (crude) page visit counter to your site. you can force it
//! to be rerendered each page render using [`render_dynamic`](crate::component::Component::render_dynamic):
//! ```rust
//! use fishnet::component::prelude::*;
//!
//! fn visit_counter() -> impl BuildableComponent {
//!     let count = Arc::new(Mutex::new(0));
//!    
//!     component!(VisitCounter)
//!         .with_state(count)
//!         .render_dynamic(|count| async move {
//!             let mut count = count.lock().await;
//!             *count += 1;
//!             html! {
//!                 "you are visitor no. " (count) "!"
//!             }
//!         }.boxed())
//! }
//! ```
//! there is one important performance consideration to using dynamic components: **using a dynamic component
//! will force all its parents to also be rendered dynamically**. this is usually not a big issue,
//! but it should be taken into consideration nonetheless. this is also why **you should never rely
//! on your static components render function being called only once**.
//!
//! ## htmx
//! fishnet is built around supporting [htmx](https://htmx.org/). each component automagically gets
//! assigned its very own api endpoint. you can add routes to it using the [`route`](crate::component::Component::route) function
//!
//! the root url of the endpoint is then provided to you via the
//! [`endpoint`](crate::component::ComponentState::endpoint) function on the components state
//!
//! implementing the htmx quick start example is as simple as
//! ```rust
//! use fishnet::component::prelude::*;
//!
//! fn awesome_htmx_btn() -> impl BuildableComponent {
//!     async fn click_endpoint(state: Extension<ComponentState<()>>) -> Markup {
//!         html! { "hiiii!!" }
//!     }
//!
//!     component!(HtmxButton)
//!         .route("/", routing::post(click_endpoint))
//!         .render(|state| async move {
//!             html! {
//!                 button hx-post=(state.endpoint()) hx-swap="outerHTML" {
//!                     "click me"
//!                 }
//!             }
//!         }.boxed())
//! }
//! ```
//! as you can see, the components state also gets passed to the components routes as an axum
//! [`Extension`](https://docs.rs/axum/latest/axum/struct.Extension.html)
//!
//! of course these api endpoints are not restricted to being used with htmx. you can serve
//! anything that can be made into an axum response!
//!
//! ## styling
//! fishnet provides its own [`css!`](crate::css!) macro that you can use to style your components.
//! it supports a slightly modified version of the normal css syntax that applies styles relative
//! to your component:
//!
//! ```rust
//! use fishnet::component::prelude::*;
//!
//! fn styled_component() -> impl BuildableComponent {
//!     component!(StyledComponent)
//!         .style(css! {
//!             background-color: red;
//!
//!             > div {
//!                 padding: 10px;
//!             }
//!             
//!             @media (max-width: 600px) {
//!                 background-color: blue;
//!
//!                 .some-child {
//!                     display: none;
//!                 }
//!             }
//!         })
//!         .render(|_| async {
//!             html!{
//!                 //whatever...
//!             }
//!         }.boxed())
//! }
//! ```
//!
//! there are two special things you can do in the css macro:
//! - **top level declarations** - if a css declaration does not appear within a block, it is
//! applied to the component itself (e.g. in the above example, the components `background-color`
//! would be red). this also works within media queries
//! - **relative selectors** - since all css is applied relative to the component, a selector like
//! `> div` is perfectly valid. it selects all the `div`s that are direct children of the
//! component. this also means that a selector like `*` will only affect the components children.
//!
//! if you want to style a specific child component, its class name will always be derived from the
//! type you put into the [`component!`](crate::component!) macro (e.g. "SomeChild" becomes
//! "some-child"). this also means that conflicts can occur if you use the same name multiple
//! times, choose your names wisely...
//!
//! ## javascript
//! lastly, you can attach custom javascript to your components using
//! [`add_script`](crate::component::Component::add_script). this can be both written
//! [`Inline`](crate::js::ScriptType::Inline) or loaded from an
//! [`External`](`crate::js::ScriptType::External`) file. All loaded scripts will be bundled into
//! one and minified if you use the optional `minify-js` crate feature (note that this will use
//! [esbuild](https://esbuild.github.io/) internally, so you need to have [go](https://go.dev/) installed on your system.)
//!

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod component;
mod routes;

mod page;
#[doc(hidden)]
pub use page::render_context::{global_store, render_component, GlobalStoreEntry};
pub use page::Page;

mod website;
pub use website::Website;

pub mod css;
pub mod js;

/// macro for generating a [`StyleFragment`](crate::css::StyleFragment) from css.
///
/// the syntax is very similar to the css syntax, with some extensions:
///
/// #### top level declarations
/// ```css
/// css! {
///    color: red
///    background-color: #00ff00;
/// };
/// ```
/// will automatically be scoped to select the root class used while rendering the resulting [`StyleFragment`](crate::css::StyleFragment)
///
/// #### relative selectors
/// ```css
/// css! {
///     > div {
///         /* ... */
///     }
///     
///     * {
///         /* ... */
///     }
/// }
/// ```
/// all selectors are relative to the root class used while rendering the resulting [`StyleFragment`](crate::css::StyleFragment). therefore `> div` will only select its direct children that are `div`s
/// and `*` will select all children of the root class
///
/// ### the issue with `em`, `ex` and color hex values
/// due to the way rust syntax works, everytime you have a number followed by the letter `e`
/// (e.g. `#2effff`, `1em`, ...), it will be interpreted as an exponential number and result in a
/// compile error. in these special cases you can use a string literal and it will be automatically
/// unescaped. this currently works for strings that start with a `#` and strings that end with `em` and `ex`, everything else will stay a string.
/// if you want a string that matches these conditions to stay as a string, you can triple quote it:
/// ```css
/// css! {
///     some-color: #2effff   /* this will not compile... */
///     some-color: "#2effff" /* ...but this will! */
///
///     some-length: 1em;     /* this will not compile... */
///     some-length: "1em";   /* ...but this will! */
///
///     some-length: 1ex;     /* this will not compile... */
///     some-length: "1ex";   /* ...but this will! */
///
///     some-other-string: "hello world!"; /* this will stay a string */
///     string-with-em: """1em""";         /* this will become a normal string */
/// }
///```
///
/// ### double class selectors and other special cases
/// in special cases like `.root-class.other-class` (e.g. both classes on the same element), you
/// can refer to the root class using `&`:
/// ```css
/// css! {
///     &.other-class {
///         /* ... */
///     }
/// }
/// ```
/// this will only work if the `&` is the first character of the selector!
pub use fishnet_macros::css;

/// TODO: document this
pub use fishnet_macros::component;

#[doc(hidden)]
pub use fishnet_macros::const_nanoid;

/// macro for generating [`Markup`] from html.
///
/// this is just a reexport of the `html!` macro from the [maud](https://maud.lambda.xyz/) crate.
///
/// **note:** due to the way this macro works, you will still need to add maud as a dependency to your project.
/// it's just here for convenience.
pub use maud::html;
pub use maud::Markup;
