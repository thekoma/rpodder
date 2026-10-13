//! OPML export from the web UI: titled outlines and a download file name.

use axum::http::{StatusCode, header};
use serde_json::json;

use rpodder_core::repo::PodcastRepo;

use super::TestApp;

const FEED: &str = "http://example.com/feed.xml";
const UNFETCHED: &str = "http://example.com/new.xml";
const BLANK: &str = "http://example.com/blank.xml";

#[tokio::test]
async fn opml_export_names_feeds_and_the_download() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let (status, body) = app
        .post_json(
            &alice.cookie,
            "/api/2/subscriptions/alice/phone.json",
            json!({"add": [FEED, UNFETCHED, BLANK], "remove": []}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let repo = app.repo();
    let (mut podcast, _) = PodcastRepo::get_or_create_for_url(&repo, FEED)
        .await
        .unwrap();
    podcast.title = "Fresh Air & Friends".to_string();
    PodcastRepo::update(&repo, &podcast).await.unwrap();
    // A title of only whitespace falls back to the URL like a missing one.
    let (mut blank, _) = PodcastRepo::get_or_create_for_url(&repo, BLANK)
        .await
        .unwrap();
    blank.title = " \n\t ".to_string();
    PodcastRepo::update(&repo, &blank).await.unwrap();

    for (uri, file) in [
        ("/subscriptions/alice.opml", "rpodder-alice.opml"),
        (
            "/subscriptions/alice/phone.opml",
            "rpodder-alice-phone.opml",
        ),
    ] {
        let (status, headers, opml) = app.get_text(&alice.cookie, uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {opml}");
        assert_eq!(
            headers[header::CONTENT_DISPOSITION],
            format!("attachment; filename=\"{file}\""),
            "{uri}"
        );
        assert!(
            opml.contains(&format!(
                r#"text="Fresh Air &amp; Friends" title="Fresh Air &amp; Friends" xmlUrl="{FEED}""#
            )),
            "{uri}: {opml}"
        );
        for url in [UNFETCHED, BLANK] {
            assert!(
                opml.contains(&format!(r#"text="{url}" title="{url}" xmlUrl="{url}""#)),
                "{uri}: {opml}"
            );
        }
    }

    // JSON stays a plain URL list, without a download header.
    let (status, headers, body) = app
        .get_text(&alice.cookie, "/subscriptions/alice.json")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get(header::CONTENT_DISPOSITION).is_none());
    let urls: Vec<String> = serde_json::from_str(&body).unwrap();
    assert_eq!(urls.len(), 3, "{body}");
}
