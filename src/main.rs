use std::env;
use dotenv::dotenv;
use reqwest::{
    blocking::Client,
    header,
};

const STREAM_RULE_TAG: &str = "official_account_tweets";

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
