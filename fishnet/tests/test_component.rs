extern crate fishnet_macros;
use fishnet::component::prelude::*;
use fishnet::global_store;
use fishnet_macros::{component, css};
use std::sync::Arc;

#[cfg(test)]
use pretty_assertions::assert_eq;

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
    let render = result.built_component.render().await;

    assert_eq!(result.built_component.name(), "TestingComponent");

    assert_eq!(
        render.0,
        "<div class=\"testing-component\"><div>Hello, world! 0</div></div>"
    );

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

    let result = testing_component(42).build("/").await;
    let render = result.built_component.render().await;

    assert_eq!(
        render.0,
        "<div class=\"testing-component\"><div>Hello, world! 42</div></div>"
    );
}
