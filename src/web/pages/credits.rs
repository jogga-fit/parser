use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
struct Dep {
    name: &'static str,
    description: &'static str,
    license: &'static str,
}

impl Dep {
    const fn new(name: &'static str, description: &'static str, license: &'static str) -> Self {
        Self { name, description, license }
    }
}

const ACTIVITYPUB: &[Dep] = &[
    Dep::new("activitypub_federation", "ActivityPub federation protocol implementation", "AGPL-3.0"),
];

const WEB: &[Dep] = &[
    Dep::new("Dioxus", "Full-stack Rust UI framework (web + SSR + WASM)", "MIT"),
    Dep::new("Axum", "Ergonomic async web framework for Rust", "MIT"),
    Dep::new("Tower HTTP", "HTTP middleware and utilities", "MIT"),
];

const ASYNC: &[Dep] = &[
    Dep::new("Tokio", "Async runtime for Rust", "MIT"),
];

const DATA: &[Dep] = &[
    Dep::new("SQLx", "Async SQL toolkit with compile-time query checking", "MIT OR Apache-2.0"),
    Dep::new("Serde", "Serialization and deserialization framework", "MIT OR Apache-2.0"),
    Dep::new("uuid", "UUID generation and parsing", "MIT OR Apache-2.0"),
    Dep::new("chrono", "Date and time library", "MIT OR Apache-2.0"),
    Dep::new("url", "URL parsing per the WHATWG standard", "MIT OR Apache-2.0"),
];

const CRYPTO: &[Dep] = &[
    Dep::new("argon2", "Password hashing with Argon2", "MIT OR Apache-2.0"),
    Dep::new("rsa", "RSA cryptography for HTTP signatures", "MIT"),
    Dep::new("sha2", "SHA-2 hash functions", "MIT OR Apache-2.0"),
    Dep::new("base64", "Base64 encoding and decoding", "MIT OR Apache-2.0"),
    Dep::new("rand", "Random number generation", "MIT OR Apache-2.0"),
];

const EMAIL: &[Dep] = &[
    Dep::new("lettre", "Email client for Rust", "MIT"),
];

const FITNESS: &[Dep] = &[
    Dep::new("fitparser", "Garmin FIT file parser", "MIT"),
    Dep::new("gpx", "GPX file parser and writer", "MIT"),
];

const UTILS: &[Dep] = &[
    Dep::new("tracing", "Application-level tracing and diagnostics", "MIT"),
    Dep::new("thiserror", "Ergonomic error type derivation", "MIT OR Apache-2.0"),
    Dep::new("reqwest", "High-level async HTTP client", "MIT OR Apache-2.0"),
    Dep::new("clap", "Command-line argument parser", "MIT OR Apache-2.0"),
    Dep::new("config", "Layered configuration management", "MIT"),
    Dep::new("mimalloc", "High-performance memory allocator", "MIT"),
];

const TESTING: &[Dep] = &[
    Dep::new("Playwright", "End-to-end browser testing framework", "Apache-2.0"),
    Dep::new("TypeScript", "Typed superset of JavaScript", "Apache-2.0"),
];

fn license_class(license: &str) -> &'static str {
    if license.contains("AGPL") {
        "license-badge license-agpl"
    } else if license.contains("Apache") {
        "license-badge license-apache"
    } else {
        "license-badge license-mit"
    }
}

#[component]
fn DepGroup(title: &'static str, deps: Vec<Dep>) -> Element {
    rsx! {
        div { class: "credits-group",
            h2 { class: "credits-group-title", "{title}" }
            div { class: "credits-dep-list",
                for dep in deps {
                    div { class: "credits-dep",
                        div { class: "credits-dep-info",
                            span { class: "credits-dep-name", "{dep.name}" }
                            span { class: "credits-dep-desc", "{dep.description}" }
                        }
                        span { class: license_class(dep.license), "{dep.license}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn CreditsPage() -> Element {
    rsx! {
        div { class: "credits-page",
            div { class: "credits-header",
                h1 { class: "credits-title", "Open Source Credits" }
                p { class: "credits-subtitle",
                    "Jogga is built on the shoulders of giants. Thank you to every contributor \
                     in the open source ecosystem that made this possible."
                }
            }

            div { class: "credits-license-note",
                div { class: "credits-license-icon", i { class: "ph ph-scales" } }
                div {
                    p { class: "credits-license-heading", "GNU Affero General Public License v3.0" }
                    p { class: "credits-license-body",
                        "This software is licensed under the AGPL-3.0, in part because it depends \
                         on activitypub_federation which carries the same license. Any deployment \
                         that serves users over a network must make its complete source code \
                         available to those users."
                    }
                    a {
                        class: "credits-license-link",
                        href: "https://github.com/jogga-fit/core/blob/main/LICENSE",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "View LICENSE on GitHub"
                        i { class: "ph ph-arrow-square-out" }
                    }
                }
            }

            DepGroup { title: "ActivityPub & Federation", deps: ACTIVITYPUB.to_vec() }
            DepGroup { title: "Web Framework", deps: WEB.to_vec() }
            DepGroup { title: "Async Runtime", deps: ASYNC.to_vec() }
            DepGroup { title: "Data & Serialization", deps: DATA.to_vec() }
            DepGroup { title: "Cryptography & Auth", deps: CRYPTO.to_vec() }
            DepGroup { title: "Email", deps: EMAIL.to_vec() }
            DepGroup { title: "Fitness Data", deps: FITNESS.to_vec() }
            DepGroup { title: "Utilities", deps: UTILS.to_vec() }
            DepGroup { title: "Testing", deps: TESTING.to_vec() }

            div { class: "credits-footer",
                a {
                    href: "https://github.com/jogga-fit/core",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    class: "btn btn-secondary",
                    i { class: "ph ph-github-logo" }
                    "View source on GitHub"
                }
            }
        }
    }
}
