use std::{env, thread, io::{BufReader, BufRead}, sync::mpsc};
use dotenv::dotenv;
use serde::Deserialize;
use reqwest::{
    blocking::Client,
    header,
};

const STREAM_RULE_TAG: &str = "official_account_tweets";

#[derive(Deserialize)]
struct TweetData {
    text: String,
}

#[derive(Deserialize)]
struct Tweet {
    data: TweetData,
}

fn main() {
    dotenv().ok();

    let client = Client::new();
    let app_access_token = env::var("APP_ACCESS_TOKEN").unwrap();

    let get_rule_response = client
        .get("https://api.twitter.com/2/tweets/search/stream/rules")
        .header(header::AUTHORIZATION, &format!("Bearer {}", app_access_token))
        .send()
        .unwrap();
    let get_rule_response_body: serde_json::Value = get_rule_response.json().unwrap();

    match get_rule_response_body.get("data") {
        Some(rule_list) => {
            let has_rule = rule_list.as_array().unwrap().iter().any(|rule| {
                let tag = rule["tag"].as_str().unwrap();
                tag == STREAM_RULE_TAG
            });
            if has_rule {
                println!("has rule");
            } else {
                add_rule(&client);
            }
        },
        None => {
            add_rule(&client);
        },
    }

    let (tweet_tx, tweet_rx) = mpsc::channel();

    thread::spawn(move || {
        let tweet_stream_response = client
            .get("https://api.twitter.com/2/tweets/search/stream")
            .header(header::AUTHORIZATION, &format!("Bearer {}", app_access_token))
            .send()
            .unwrap();

        let tweet_stream_reader = BufReader::new(tweet_stream_response);
        for tweet_object_result in tweet_stream_reader.lines() {
            let tweet_object_string = tweet_object_result.unwrap();
            if tweet_object_string == "" {
                continue;
            }
            let tweet_object: Tweet = serde_json::from_str(&tweet_object_string).unwrap();
            tweet_tx.send(tweet_object.data.text).unwrap();
        }
    });

    for tweet_text in tweet_rx {
        println!("{}", tweet_text);
    }
}

fn add_rule(client: &Client) {
    println!("no rule");

    let app_access_token = env::var("APP_ACCESS_TOKEN").unwrap();
    let account_username = env::var("ACCOUNT_USERNAME").unwrap();

    let add_rule_request_body = serde_json::json!({
        "add": [
            {"value": format!("from:{}", account_username), "tag": STREAM_RULE_TAG}
        ]
    });

    let add_rule_response = client
        .post("https://api.twitter.com/2/tweets/search/stream/rules")
        .header(header::AUTHORIZATION, &format!("Bearer {}", app_access_token))
        .json(&add_rule_request_body)
        .send()
        .unwrap();
    let add_rule_response_body: serde_json::Value = add_rule_response.json().unwrap();

    let added_rule_count = add_rule_response_body["meta"]["summary"]["created"].as_i64().unwrap();
    assert_eq!(added_rule_count, 1);

    let added_rule_id = add_rule_response_body["data"][0]["id"].as_str().unwrap();
    println!("rule added(id = {})", added_rule_id);
}
