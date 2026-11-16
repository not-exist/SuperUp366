//! Word-PK bucket adapter.

use serde_json::json;

use crate::adapters::{decode_items, endpoint};
use crate::client::Client;
use crate::codec;
use crate::dto::Word;
use crate::error::Result;

/// Return words from `data.contentList[].entryList[]` in server order.
pub async fn bucket_words(client: &Client, bucket_id: &str) -> Result<Vec<Word>> {
    let endpoint = endpoint("bucket_words")?;
    let data = client
        .call(
            endpoint,
            &[("source", json!(1)), ("bucketId", json!(bucket_id))],
        )
        .await?;
    let mut object = codec::into_object(data, endpoint.name)?;
    let groups = codec::take_list(&mut object, "contentList", endpoint.name)?;
    let mut words = Vec::new();
    for group in groups {
        let mut group = match group {
            serde_json::Value::Object(group) => group,
            _ => continue,
        };
        let entries = codec::take_list(&mut group, "entryList", endpoint.name)?;
        words.extend(entries);
    }
    decode_items(words, endpoint.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn bucket_words_posts_source_and_flattens_entries() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/sync/teaching/bucket/detail-info")
                .body_includes("source=1")
                .body_includes("bucketId=BUCKET-1");
            then.status(200).json_body(ok(json!({
                "contentList": [
                    { "entryList": [{ "entryId": 1 }, { "entryId": 2 }] },
                    { "entryList": [{ "entryId": 3 }] }
                ]
            })));
        });

        let client = client(&server);
        let words = bucket_words(&client, "BUCKET-1").await.unwrap();

        assert_eq!(words.len(), 3);
        assert_eq!(words[0].entry_id.as_deref(), Some("1"));
        assert_eq!(words[2].entry_id.as_deref(), Some("3"));
        mock.assert_calls(1);
    }
}
