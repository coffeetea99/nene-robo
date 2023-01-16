use std::{env, str};
use dotenv::dotenv;
use tokio;
use serde::Deserialize;
use reqwest::{
    Client,
    header,
};
use regex::Regex;

const STREAM_RULE_TAG: &str = "official_account_tweets";

#[derive(Deserialize)]
struct TweetData {
    text: String,
}

#[derive(Deserialize)]
struct Tweet {
    data: TweetData,
}

fn day_kanji_to_hangul(kanji: &str) -> &'static str {
    match kanji {
        "月" => "월",
        "火" => "화",
        "水" => "수",
        "木" => "목",
        "金" => "금",
        "土" => "토",
        "日" => "일",
        _ => "",
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let client = Client::new();
    let app_access_token = env::var("APP_ACCESS_TOKEN").unwrap();

    let get_rule_response = client
        .get("https://api.twitter.com/2/tweets/search/stream/rules")
        .header(header::AUTHORIZATION, &format!("Bearer {}", app_access_token))
        .send()
        .await
        .unwrap();
    let get_rule_response_body: serde_json::Value = get_rule_response.json().await.unwrap();

    match get_rule_response_body.get("data") {
        Some(rule_list) => {
            let has_rule = rule_list.as_array().unwrap().iter().any(|rule| {
                let tag = rule["tag"].as_str().unwrap();
                tag == STREAM_RULE_TAG
            });
            if has_rule {
                println!("has rule");
            } else {
                add_rule(&client).await;
            }
        },
        None => {
            add_rule(&client).await;
        },
    }

    let mut tweet_stream_response = client
        .get("https://api.twitter.com/2/tweets/search/stream")
        .header(header::AUTHORIZATION, &format!("Bearer {}", app_access_token))
        .send()
        .await
        .unwrap();

    while let Some(tweet_object_chunk) = tweet_stream_response.chunk().await.unwrap() {
        let tweet_object_string = str::from_utf8(&tweet_object_chunk).unwrap().trim();
        if tweet_object_string == "" {
            continue;
        }
        let tweet_object: Tweet = serde_json::from_str(&tweet_object_string).unwrap();
        let tweet_text = tweet_object.data.text;
        handle_tweet(&tweet_text);
    }
}

async fn add_rule(client: &Client) {
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
        .await
        .unwrap();
    let add_rule_response_body: serde_json::Value = add_rule_response.json().await.unwrap();

    let added_rule_count = add_rule_response_body["meta"]["summary"]["created"].as_i64().unwrap();
    assert_eq!(added_rule_count, 1);

    let added_rule_id = add_rule_response_body["data"][0]["id"].as_str().unwrap();
    println!("rule added(id = {})", added_rule_id);
}

fn handle_tweet(tweet_text: &str) {
    let event_ended_rule = Regex::new(r"^(?P<month>\d{1,2})月(?P<date>\d{1,2})日（(?P<day>.)）.*\n.*\nアフターライブを開催").unwrap();
    event_ended_rule.captures(tweet_text).map(|capture| {
        println!("{}월 {}일 ({}) 이벤트 종료", &capture["month"], &capture["date"], day_kanji_to_hangul(&capture["day"]));  // TODO: store in database & notice on the day
    });

    let event_ended_rule = Regex::new(r"^本日.*\n(?P<event_name>.*)\nアフターライブを開催").unwrap();
    event_ended_rule.captures(tweet_text).map(|capture| {
        println!("`{}` 이벤트가 종료되었습니다.\n애프터 라이브를 시청하세요.\n이벤트 스토리를 다 봤는지 확인하세요.", &capture["event_name"]);
    });

    let wondershow_added_rule = Regex::new(r"^(?P<month>\d{1,2})月(?P<date>\d{1,2})日（(?P<day>.)）(?P<hour>\d{1,2})時より\n『ワンダショちゃんねる #(?P<episode_number>\d+)』の生配信が決定！").unwrap();
    wondershow_added_rule.captures(tweet_text).map(|capture| {
        println!("{}월 {}일 ({}) {}시부터 제{}회 원더쇼 채널이 방영될 예정입니다.", &capture["month"], &capture["date"], day_kanji_to_hangul(&capture["day"]), &capture["hour"], &capture["episode_number"]);
    });

    let wondershow_starting_rule = Regex::new(r"^このあと(?P<hour>\d{1,2})時より『ワンダショちゃんねる #(?P<episode_number>\d+)』を生配信").unwrap();
    wondershow_starting_rule.captures(tweet_text).map(|capture| {
        println!("잠시 후 {}시부터 제{}회 원더쇼 채널이 방영될 예정입니다.", &capture["hour"], &capture["episode_number"]);
    });
}
