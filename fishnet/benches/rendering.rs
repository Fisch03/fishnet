use fishnet::component::prelude::*;
use fishnet::page::{BuiltPage, Page};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

macro_rules! make_page {
    ($component: expr, $amt: literal) => {{
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        let page = Page::new("bench").with_body(|| {
            async {
                for _ in 0..$amt {
                    let _ = fishnet::render_component(&nanoid::nanoid!(10), || $component);
                }

                html! {}
            }
            .boxed()
        });

        let (built, _) = runtime.block_on(async { BuiltPage::new(page, "/").await });

        built
    }};
}

fn bench_render(cr: &mut Criterion) {
    #[component]
    fn basic_component() {
        html! {
            "Hello world!"
        }
    }

    let page = make_page!(basic_component(), 1000);

    cr.bench_with_input(
        BenchmarkId::new("render_basic", 1000),
        &Extension(page),
        |b, page| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();

            b.to_async(runtime)
                .iter(|| async { BuiltPage::render(page.clone()).await })
        },
    );

    #[component]
    fn styled_component() {
        style!(css! {
            color: red;
        });

        html! {
            "Hello world!"
        }
    }

    let page = make_page!(basic_component(), 1000);

    cr.bench_with_input(
        BenchmarkId::new("render_styled", 1000),
        &Extension(page),
        |b, page| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();

            b.to_async(runtime)
                .iter(|| async { BuiltPage::render(page.clone()).await })
        },
    );
}

criterion_group!(benches, bench_render);
criterion_main!(benches);
