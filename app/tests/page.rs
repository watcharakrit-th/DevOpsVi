//! The page in app/static/: how the app serves it, and the rules its files
//! must follow (no inline code, no markup built from strings, nothing from
//! other sites, no cookies).

mod common;

use axum::http::StatusCode;
use common::*;

const PAGE_FILES: [(&str, &str, &str); 3] = [
    ("/", "index.html", "text/html; charset=utf-8"),
    ("/app.js", "app.js", "text/javascript; charset=utf-8"),
    ("/style.css", "style.css", "text/css; charset=utf-8"),
];

fn static_file(name: &str) -> String {
    let path = format!("{}/static/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

#[tokio::test]
async fn each_file_is_served_with_its_type() {
    let app = miniarcade::app(state());
    for (uri, file, content_type) in PAGE_FILES {
        let reply = call(&app, "GET", uri, None, None).await;
        assert_eq!(reply.status, StatusCode::OK, "{uri}");
        assert_eq!(reply.headers["content-type"], content_type, "{uri}");
        assert_eq!(
            reply.body.as_str(),
            Some(static_file(file).as_str()),
            "{uri}"
        );
    }
}

#[tokio::test]
async fn an_invite_link_opens_the_same_page() {
    let app = miniarcade::app(state());
    let reply = call(&app, "GET", "/?lobby=ABC234", None, None).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        reply.body.as_str(),
        Some(static_file("index.html").as_str())
    );
}

#[tokio::test]
async fn browsers_check_for_a_new_version_on_every_load() {
    // So a release reaches an open page at its next reload.
    let app = miniarcade::app(state());
    for (uri, _, _) in PAGE_FILES {
        let reply = call(&app, "GET", uri, None, None).await;
        assert_eq!(reply.headers["cache-control"], "no-cache", "{uri}");
    }
}

#[tokio::test]
async fn page_responses_carry_the_security_headers() {
    let app = miniarcade::app(state());
    for (uri, _, _) in PAGE_FILES {
        assert_security_headers(&call(&app, "GET", uri, None, None).await.headers);
    }
}

#[tokio::test]
async fn each_page_file_has_its_own_route_label() {
    let state = state();
    let (app, ops) = (miniarcade::app(state.clone()), miniarcade::ops(state));
    for (uri, _, _) in PAGE_FILES {
        call(&app, "GET", uri, None, None).await;
    }
    call(&app, "POST", "/", None, None).await;
    let text = scrape(&ops).await;
    for route in ["/", "/app.js", "/style.css"] {
        let line = format!(r#"http_requests_total{{method="GET",route="{route}",status="200"}} 1"#);
        assert!(text.contains(&line), "missing {line}\n{text}");
    }
    let line = r#"http_requests_total{method="POST",route="/",status="405"} 1"#;
    assert!(text.contains(line), "missing {line}\n{text}");
}

#[test]
fn the_page_has_no_inline_scripts_styles_or_event_handlers() {
    let html = static_file("index.html").to_ascii_lowercase();
    let tags = html
        .split('<')
        .skip(1)
        .filter_map(|rest| rest.split('>').next());
    let mut scripts = 0;
    for tag in tags {
        if tag.starts_with("script") {
            scripts += 1;
            assert!(
                tag.contains("src=\"/"),
                "script without a same-site src: <{tag}>"
            );
        }
        assert!(!tag.starts_with("style"), "inline stylesheet: <{tag}>");
        for attribute in tag.split_whitespace().skip(1) {
            assert!(
                !attribute.starts_with("on"),
                "inline event handler: <{tag}>"
            );
            assert!(!attribute.starts_with("style="), "inline style: <{tag}>");
        }
    }
    assert_eq!(scripts, 1, "exactly one script tag");
}

#[test]
fn the_script_never_turns_strings_into_markup_or_code() {
    let js = static_file("app.js");
    let banned = [
        "innerHTML",
        "outerHTML",
        "insertAdjacentHTML",
        "document.write",
        "eval(",
        "new Function",
        "DOMParser",
    ];
    for word in banned {
        assert!(!js.contains(word), "app.js uses {word}");
    }
    assert!(js.contains("textContent"));
}

#[test]
fn tokens_live_in_session_storage_and_there_are_no_cookies() {
    let js = static_file("app.js");
    assert!(js.contains("sessionStorage"));
    assert!(
        !js.contains("localStorage"),
        "tokens must not outlive the tab"
    );
    assert!(!js.contains("document.cookie"));
}

#[test]
fn nothing_loads_from_other_sites() {
    for (_, file, _) in PAGE_FILES {
        let text = static_file(file);
        for scheme in ["http://", "https://", "//cdn", "@import"] {
            assert!(!text.contains(scheme), "{file} contains {scheme}");
        }
    }
}
