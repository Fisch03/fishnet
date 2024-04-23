extern crate fishnet_macros;
use fishnet::component::prelude::*;
use fishnet::global_store;
use fishnet_macros::{component, css};
use std::sync::Arc;

#[cfg(test)]
use pretty_assertions::assert_eq;

#[test]
fn test_ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/component/*.rs");
}

#[derive(Default)]
struct TestComponentState {
    some_val: usize,
}

#[tokio::test]
async fn test_component() {
    #[component]
    async fn testing_component() {
        let state = state!(Arc<Mutex<TestComponentState>>);

        style!(css! {
            color: #f00000;
        });

        script!("console.log('hello world');");
        script!(r#"console.log("goodbye world");"#);

        let state = state.lock().await;

        html! {
            div {
                "Hello, world! " (state.some_val)
            }
        }
    }

    let result = testing_component().build("/").await;

    assert_eq!(result.built_component.name(), "TestingComponent");

    let render = result.built_component.render().await;
    assert_eq!(
        render.0,
        "<div class=\"testing-component\"><div>Hello, world! 0</div></div>"
    );

    assert!(result.runner.is_none());
    assert!(result.router.is_none());

    let component_entry = global_store()
        .get(result.built_component.id())
        .await
        .unwrap();

    assert_eq!(component_entry.scripts.len(), 1);
    match &component_entry.scripts[0] {
        fishnet::js::ScriptType::Inline(script) => {
            assert_eq!(
                script,
                &"console.log('hello world');console.log(\"goodbye world\");"
            );
        }
        _ => panic!("expected static script"),
    }

    assert_eq!(component_entry.style.is_some(), true);
}

#[tokio::test]
async fn test_component_args() {
    #[component]
    async fn testing_component(some_val: usize) {
        let state = state_init!(Arc::new(TestComponentState { some_val }));

        style!(css! {
            color: #f00000;
        });

        script!("console.log('hello world');");
        script!(r#"console.log("goodbye world");"#);

        html! {
            div {
                "Hello, world! " (state.some_val)
            }
        }
    }

    let result = testing_component(42).build("/").await;
    let render = result.built_component.render().await;

    assert_eq!(
        render.0,
        "<div class=\"testing-component\"><div>Hello, world! 42</div></div>"
    );
}

#[tokio::test]
async fn test_component_state_ident() {
    #[component]
    async fn testing_component() {
        let count = state!(Arc<Mutex<TestComponentState>>);

        let mut count = count.lock().await;
        count.some_val += 1;

        html! {
            (count.some_val)
        }
    }

    let result = testing_component().build("/").await;
    let render = result.built_component.render().await;

    assert_eq!(render.0, "<div class=\"testing-component\">1</div>");
}

#[tokio::test]
async fn test_component_staticity() {
    #[component]
    async fn testing_component() {
        let count = state!(Arc<Mutex<TestComponentState>>);

        let mut count = count.lock().await;
        count.some_val += 1;

        html! {
            (count.some_val)
        }
    }

    let result = testing_component().build("/").await;

    let render = result.built_component.render().await;
    assert_eq!(render.0, "<div class=\"testing-component\">1</div>");

    let render = result.built_component.render().await;
    assert_eq!(render.0, "<div class=\"testing-component\">1</div>");
}

#[tokio::test]
async fn test_component_dynamic() {
    #[dyn_component]
    async fn testing_component() {
        let count = state!(Arc<Mutex<TestComponentState>>);

        let mut count = count.lock().await;
        count.some_val += 1;

        html! {
            (count.some_val)
        }
    }

    let result = testing_component().build("/").await;
    dbg!(&result.built_component);

    let render = result.built_component.render().await;
    assert_eq!(render.0, "<div class=\"testing-component\">1</div>");

    let render = result.built_component.render().await;
    assert_eq!(render.0, "<div class=\"testing-component\">2</div>");
}

#[tokio::test]
async fn test_component_route_post() {
    #[component]
    async fn testing_component() {
        let state = state!(Arc<TestComponentState>);

        #[route("/", POST)]
        async fn root(state: Extension<ComponentState<Arc<TestComponentState>>>) -> Markup {
            html! {
                (state.some_val)
            }
        }
    }

    let result = testing_component().build("/").await;

    assert!(result.router.is_some());
}
