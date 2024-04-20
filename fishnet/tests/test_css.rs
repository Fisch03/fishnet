extern crate fishnet_macros;
use fishnet_macros::css;

#[test]
fn test_css() {
    let fragment = css! {
        color: #f00000;

        img-test {
            width: 100%;
            height: calc(10rem + 1vh);
            z-index: 100;
        }

        * {
            margin: 0;
        }

        > div {
            color: #000;
        }

        :nth-child(2) {
            --var-name: 10px;
        }

        .empty-rule {}

        display: inline-block;

        @media (max-width: 600px) and (min-width: 400px) {
            .test::first-child {
                color: blue;
            }

            #test {
                color: green;
            }
        }
    };

    println!("{}", &fragment.render("component"));

    assert_eq!(
        fragment.render("component"),
        r".component {
    color: #f00000;
    display: inline-block;
}

.component img-test {
    width: 100%;
    height: calc(10rem + 1vh);
    z-index: 100;
}

.component * {
    margin: 0;
}

.component > div {
    color: #000;
}

.component:nth-child(2) {
    --var-name: 10px;
}

@media (max-width: 600px) and (min-width: 400px) {
    .component .test::first-child {
        color: blue;
    }

    .component #test {
        color: green;
    }
}
"
    );
}
