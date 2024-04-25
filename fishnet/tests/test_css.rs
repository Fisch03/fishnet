extern crate fishnet_macros;
use fishnet_macros::css;

#[cfg(test)]
use pretty_assertions::assert_eq;
#[cfg(test)]
use unindent::unindent;

#[test]
fn test_ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/css/*.rs");
}

fn test_match(style: fishnet::css::StyleFragment, expected: &str) {
    let mut stylesheet = fishnet::css::Stylesheet::new();
    stylesheet.add(&style.render("component"));

    let render = stylesheet.render();
    let render = render.trim();

    let expected = expected.trim();
    let expected = unindent(expected);

    assert_eq!(expected, render);
}

#[test]
fn test_toplevel() {
    test_match(
        css! {
            color: #f00000;

            .some-rule {
                width: 100%;
            }

            display: inline-block;
        },
        r"
        .component {
            color: #f00000;
            display: inline-block;
        }

        .component .some-rule {
            width: 100%;
        }
        ",
    )
}

#[test]
fn test_relative() {
    test_match(
        css! {
            .img-test {
                rule: 1;
            }

            * {
                rule: 2;
            }

            > div {
                rule: 3;
            }

            :nth-child(2) {
                rule: 4;
            }

            ::pseudo-class {
                rule: 5;
            }
        },
        r"
        .component .img-test {
            rule: 1;
        }

        .component * {
            rule: 2;
        }

        .component > div {
            rule: 3;
        }

        .component:nth-child(2) {
            rule: 4;
        }

        .component::pseudo-class {
            rule: 5;
        }
        ",
    )
}

#[test]
fn test_relative_explicit() {
    test_match(
        css! {
            &.double-class {
                rule: 1;
            }
        },
        r"
        .component.double-class {
            rule: 1;
        }
        ",
    )
}

#[test]
fn test_nested_pseudo() {
    test_match(
        css! {
            .some-other::pseudo-class {
                rule: 1;
            }
        },
        r"
        .component .some-other::pseudo-class {
            rule: 1;
        }
        ",
    )
}

#[test]
fn test_empty_rule() {
    test_match(
        css! {
            .empty-rule {}
        },
        r"
        ",
    )
}

#[test]
fn test_at_rule() {
    test_match(
        css! {
            @import url("whatever.css");

            @media screen and (max-width: 600px) and (min-width: 400px) {
                .test::first-child {
                    color: blue;
                }

                #test {
                    color: green;
                }
            }
        },
        r#"
        @import url("whatever.css");

        @media screen and (max-width: 600px) and (min-width: 400px) {
            .component .test::first-child {
                color: blue;
            }

            .component #test {
                color: green;
            }
        }
        "#,
    )
}

#[test]
fn test_media_merge() {
    let style_one = css! {
        @media screen and (min-width: 400px) {
            color: blue;
        }
    };

    let style_two = css! {
        @media screen and (min-width: 400px) {
            color: green;
        }
    };

    let mut stylesheet = fishnet::css::Stylesheet::new();
    stylesheet.add(&style_one.render("component-a"));
    stylesheet.add(&style_two.render("component-b"));

    assert_eq!(
        stylesheet.render().trim(),
        unindent(
            r"
        @media screen and (min-width: 400px) {
            .component-a {
                color: blue;
            }

            .component-b {
                color: green;
            }
        }
        "
        )
        .trim()
    );
}

#[test]
fn test_units_abs_length() {
    test_match(
        css! {
            length: 1cm;
            length: 1mm;
            length: 1in;
            length: 1px;
            length: 1pt;
            length: 1pc;
            length: 1Q;
        },
        r"
        .component {
            length: 1cm;
            length: 1mm;
            length: 1in;
            length: 1px;
            length: 1pt;
            length: 1pc;
            length: 1Q;
        }
        ",
    )
}

#[test]
fn test_units_rel_length() {
    test_match(
        css! {
            length: "1em";
            length: "1ex";
            length: 1ch;
            length: 1rem;
            length: 1vw;
            length: 1vh;
            length: 1vmin;
            length: 1vmax;
            length: 1%;
            length: 1lh;
            length: 1rlh;
        },
        r#"
        .component {
            length: 1em;
            length: 1ex;
            length: 1ch;
            length: 1rem;
            length: 1vw;
            length: 1vh;
            length: 1vmin;
            length: 1vmax;
            length: 1%;
            length: 1lh;
            length: 1rlh;
        }
        "#,
    )
}

#[test]
fn test_units_colors() {
    test_match(
        css! {
            color: red;
            color: antiquewhite;
            color: "#22eb41";
            color: #a0c;
            color: rgb(104 284 596);
            color: rgb(104 284 596 / .6);
            color: rgba(104 284 596 0.6);
            color: hsl(104 284 596);
            color: hsl(104 284 596 / .6);
            color: hwb(104 284 596);
            color: lch(104 284 596);
            color: lab(104 284 596);
            color: hwb(104 284 596);
        },
        r"
        .component {
            color: red;
            color: antiquewhite;
            color: #22eb41;
            color: #a0c;
            color: rgb(104 284 596);
            color: rgb(104 284 596 / .6);
            color: rgba(104 284 596 0.6);
            color: hsl(104 284 596);
            color: hsl(104 284 596 / .6);
            color: hwb(104 284 596);
            color: lch(104 284 596);
            color: lab(104 284 596);
            color: hwb(104 284 596);
        }
        ",
    )
}

#[test]
fn test_units_img() {
    test_match(
        css! {
            image: url("cool_cat.png");
            image: linear-gradient(90deg, rgb(119 0 255 / 39%), rgb(0 212 255 / 100%));
        },
        r#"
        .component {
            image: url("cool_cat.png");
            image: linear-gradient(90deg, rgb(119 0 255 / 39%), rgb(0 212 255 / 100%));
        }
        "#,
    )
}

#[test]
fn test_units_pos() {
    test_match(
        css! {
            position: top;
            position: left;
            position: bottom;
            position: right;
            position: center;
        },
        r"
        .component {
            position: top;
            position: left;
            position: bottom;
            position: right;
            position: center;
        }
        ",
    )
}

#[test]
fn test_units_string() {
    test_match(
        css! {
            random-string: "hello world!";
            color-string: """#2effff""";
            em-string: """1em""";
            ex-string: """1ex""";
            random-string: """goodbye world!""";
        },
        r##"
        .component {
            random-string: "hello world!";
            color-string: "#2effff";
            em-string: "1em";
            ex-string: "1ex";
            random-string: "goodbye world!";
        }
        "##,
    )
}

#[test]
fn test_units_math() {
    test_match(
        css! {
            math: calc(1cm + 1mm);
            math: min(1cm, 1mm);
            math: calc(100% - 1cm / 2 * 3);
        },
        r"
        .component {
            math: calc(1cm + 1mm);
            math: min(1cm, 1mm);
            math: calc(100% - 1cm / 2 * 3);
        }
        ",
    )
}

#[test]
fn test_border() {
    test_match(
        css! {
            border-radius: 3rem 0 0 3rem;
            border-right: 1px solid var(--fg-color);
        },
        r"
        .component {
            border-radius: 3rem 0 0 3rem;
            border-right: 1px solid var(--fg-color);
        }
        ",
    )
}

#[test]
fn test_selector_next_sibling() {
    test_match(
        css! {
            +.next-sibling {
                color: red;
            }
        },
        r"
        .component +.next-sibling {
            color: red;
        }
        ",
    )
}
