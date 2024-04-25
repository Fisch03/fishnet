use fishnet::component::prelude::*;
use fishnet::page::render_context;
use fishnet::page::{BuiltPage, Page};

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(target_os = "linux")]
use pprof::criterion::{Output, PProfProfiler};

macro_rules! make_page {
    ($runtime: ident, $body: tt) => {{
        let page = Page::new("bench").with_body(move || async move { $body }.boxed());

        let (built, _) = $runtime.block_on(async {
            render_context::global_store().clear().await;

            let built = BuiltPage::new(page, "/").await;

            BuiltPage::render(Extension(built.0.clone())).await;

            built
        });

        built
    }};
}

macro_rules! make_page_side_by_side {
    ($component: expr, $amt: ident, $runtime: ident) => {{
        make_page!($runtime, {
            let mut render = String::new();

            for i in 0..$amt {
                let id = format!("12345678{}", i);

                render.push_str(&render_context::render_component(&id, || $component).await.0);
            }

            maud::PreEscaped(render)
        })
    }};
}
macro_rules! make_page_nested {
    ($component: expr, $runtime: ident) => {{
        make_page!($runtime, {
            let nested = $component(|| {
                async move {
                    let nested = $component(|| {
                        async move {
                            let nested = $component(|| {
                                async move {
                                    let nested = $component(|| {
                                        async {
                                            html! {
                                                "Hello world!"
                                            }
                                        }
                                        .boxed()
                                    });

                                    render_context::render_component(&"1234567893", || nested).await
                                }
                                .boxed()
                            });

                            render_context::render_component(&"1234567892", || nested).await
                        }
                        .boxed()
                    });

                    render_context::render_component(&"1234567891", || nested).await
                }
                .boxed()
            });

            render_context::render_component(&"1234567890", || nested).await
        })
    }};
}
fn bench_render(cr: &mut Criterion) {
    #[component]
    fn basic_component() {
        let a = black_box(0);

        html! {
            "Hello world!" (a)
        }
    }

    #[component]
    fn styled_component() {
        style!(css! {
            color: red;
        });

        let a = black_box(0);

        html! {
            "Hello world!" (a)
        }
    }

    #[component]
    fn stateful_component() {
        let state = state!(usize);

        html! {
            "Hello world!" (*state)
        }
    }

    #[dyn_component]
    fn dyn_component() {
        let a = black_box(0);

        html! {
            "Hello world!" (a)
        }
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .build()
        .unwrap();

    let mut group = cr.benchmark_group("render side-by-side");

    for i in [1, 250] {
        let page_basic = make_page_side_by_side!(basic_component(), i, runtime);
        group.bench_with_input(
            BenchmarkId::new("basic", i),
            &Extension(page_basic),
            |b, page| {
                b.to_async(&runtime)
                    .iter(|| BuiltPage::render(page.clone()));
            },
        );

        let page_styled = make_page_side_by_side!(styled_component(), i, runtime);
        group.bench_with_input(
            BenchmarkId::new("styled", i),
            &Extension(page_styled),
            |b, page| {
                b.to_async(&runtime)
                    .iter(|| BuiltPage::render(page.clone()));
            },
        );

        let page_stateful = make_page_side_by_side!(stateful_component(), i, runtime);
        group.bench_with_input(
            BenchmarkId::new("stateful", i),
            &Extension(page_stateful),
            |b, page| {
                b.to_async(&runtime)
                    .iter(|| BuiltPage::render(page.clone()));
            },
        );

        let page_dyn = make_page_side_by_side!(dyn_component(), i, runtime);
        group.bench_with_input(
            BenchmarkId::new("dynamic", i),
            &Extension(page_dyn),
            |b, page| {
                b.to_async(&runtime)
                    .iter(|| BuiltPage::render(page.clone()));
            },
        );
    }
    group.finish();

    #[component]
    fn basic_nested_component(
        inner: impl Fn() -> BoxFuture<'static, Markup> + Send + Sync + 'static,
    ) {
        let inner = state_init!(Arc::new(inner));

        html! {
            (inner().await)
        }
    }

    #[component]
    fn styled_nested_component(
        inner: impl Fn() -> BoxFuture<'static, Markup> + Send + Sync + 'static,
    ) {
        let inner = state_init!(Arc::new(inner));

        style!(css! {
            color: red;
        });

        html! {
            (inner().await)
        }
    }

    #[dyn_component]
    fn dyn_nested_component(
        inner: impl Fn() -> BoxFuture<'static, Markup> + Send + Sync + 'static,
    ) {
        let inner = state_init!(Arc::new(inner));

        html! {
            (inner().await)
        }
    }
    let mut group = cr.benchmark_group("render nested");

    let page_basic = make_page_nested!(basic_nested_component, runtime);
    group.bench_with_input(
        BenchmarkId::new("basic", 3),
        &Extension(page_basic),
        |b, page| {
            b.to_async(&runtime)
                .iter(|| BuiltPage::render(page.clone()));
        },
    );

    let page_styled = make_page_nested!(styled_nested_component, runtime);
    group.bench_with_input(
        BenchmarkId::new("styled", 3),
        &Extension(page_styled),
        |b, page| {
            b.to_async(&runtime)
                .iter(|| BuiltPage::render(page.clone()));
        },
    );

    let page_dyn = make_page_nested!(dyn_nested_component, runtime);
    group.bench_with_input(
        BenchmarkId::new("dynamic", 3),
        &Extension(page_dyn),
        |b, page| {
            b.to_async(&runtime)
                .iter(|| BuiltPage::render(page.clone()));
        },
    );
    group.finish();
}

#[cfg(target_os = "linux")]
criterion_group!(
    name = benches;

    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = bench_render
);
#[cfg(not(target_os = "linux"))]
criterion_group!(benches, bench_render);

criterion_main!(benches);
