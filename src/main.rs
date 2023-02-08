use std::{env, str};
use dotenv::dotenv;
use tokio;
use serde::Deserialize;
use reqwest::{
    Client,
    header,
};
use regex::Regex;
use rusqlite::{Connection, Result};
use chrono::{Utc, Duration, NaiveDateTime, Datelike};

mod error;
use error::Error;

const STREAM_RULE_TAG: &str = "official_account_tweets";

#[derive(Deserialize)]
struct TweetData {
    text: String,
}

#[derive(Deserialize)]
struct Tweet {
    data: TweetData,
}

#[derive(Debug)]
struct Event {
    id: u32,
    name: String,
    end_date: i32, // ex: 20221229
}

#[derive(Debug)]
struct Birthday {
    character_name: String,
    date: i32, // ex: 831
    live_type: String, // "생일" | "애니버서리"
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();

    let client = Client::new();
    let twitter_stream_client = client.clone();
    let cron_client = client.clone();

    let db_path = "./sqlite.db3";
    let twitter_stream_db_connection = Connection::open(db_path)?;
    let cron_db_connection = Connection::open(db_path)?;

    init_database(&twitter_stream_db_connection);

    tokio::spawn(async move {
        twitter_stream(twitter_stream_client, twitter_stream_db_connection).await
    });
    tokio::spawn(async move {
        cron(cron_client, cron_db_connection).await
    }).await;

    Ok(())
}

async fn twitter_stream(client: Client, db_connection: Connection) -> Result<(), Error> {
    let app_access_token = env::var("APP_ACCESS_TOKEN")?;

    let get_rule_response = client
        .get("https://api.twitter.com/2/tweets/search/stream/rules")
        .header(header::AUTHORIZATION, &format!("Bearer {}", app_access_token))
        .send()
        .await?;
    let get_rule_response_body: serde_json::Value = get_rule_response.json().await?;

    match get_rule_response_body.get("data") {
        Some(rule_list) => {
            let has_rule = rule_list.as_array().unwrap().iter().any(|rule| {
                let tag = rule["tag"].as_str().unwrap();
                tag == STREAM_RULE_TAG
            });
            if has_rule {
                println!("has rule");
            } else {
                add_rule(&client).await?;
            }
        },
        None => {
            add_rule(&client).await?;
        },
    }

    let mut tweet_stream_response = client
        .get("https://api.twitter.com/2/tweets/search/stream")
        .header(header::AUTHORIZATION, &format!("Bearer {}", app_access_token))
        .send()
        .await?;

    while let Some(tweet_object_chunk) = tweet_stream_response.chunk().await? {
        let tweet_object_string = str::from_utf8(&tweet_object_chunk).unwrap().trim();
        if tweet_object_string == "" {
            continue;
        }
        let tweet_object: Tweet = serde_json::from_str(&tweet_object_string)?;
        let tweet_text = tweet_object.data.text;
        handle_database(&db_connection, &tweet_text)?;
        if let Some(discord_message) = tweet_to_discord_message(&tweet_text) {
            send_to_discord(&client, &discord_message).await?;
        }
    }

    Ok(())
}

async fn add_rule(client: &Client) -> Result<(), Error> {
    println!("no rule");

    let app_access_token = env::var("APP_ACCESS_TOKEN")?;
    let account_username = env::var("ACCOUNT_USERNAME")?;

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
        .await?;
    let add_rule_response_body: serde_json::Value = add_rule_response.json().await?;

    let added_rule_count = add_rule_response_body["meta"]["summary"]["created"].as_i64().unwrap();
    assert_eq!(added_rule_count, 1);

    let added_rule_id = add_rule_response_body["data"][0]["id"].as_str().unwrap();
    println!("rule added(id = {})", added_rule_id);

    Ok(())
}

fn handle_database(db_connection: &Connection, tweet_text: &str) -> Result<(), Error> {
    let event_ends_rule = Regex::new(r"^(?P<month>\d{1,2})月(?P<date>\d{1,2})日.*\n(?P<event_name>.*)\nアフターライブを開催").unwrap();
    if let Some(capture) = event_ends_rule.captures(tweet_text) {
        let tokyo_datetime = get_tokyo_datetime();

        let mut year = tokyo_datetime.year() as u32;
        let month: u32 = capture["month"].parse().unwrap();
        let day: u32 = capture["date"].parse().unwrap();

        if tokyo_datetime.month() == 12 && month == 1 {
            year += 1;
        }

        let date = year * 10000 + month * 100 + day;
        db_connection.execute(
            "INSERT INTO event (name, end_date) VALUES (?1, ?2);",
            (&capture["event_name"], date),
        )?;
    }

    Ok(())
}

fn tweet_to_discord_message(tweet_text: &str) -> Option<String> {
    let event_ended_rule = Regex::new(r"^本日.*\n(?P<event_name>.*)\nアフターライブを開催").ok()?;
    if let Some(event_ended_message) = event_ended_rule.captures(tweet_text).map(|capture| {
        format!("`{}` 이벤트가 종료되었습니다.\n애프터 라이브를 시청하세요.\n이벤트 스토리를 다 봤는지 확인하세요.", &capture["event_name"])
    }) {
        return Some(event_ended_message);
    }

    let wondershow_added_rule = Regex::new(r"^(?P<month>\d{1,2})月(?P<date>\d{1,2})日（(?P<day>.)）(?P<hour>\d{1,2})時より\n『ワンダショちゃんねる #(?P<episode_number>\d+)』の生配信が決定！").ok()?;
    if let Some(wondershow_added_message) = wondershow_added_rule.captures(tweet_text).map(|capture| {
        format!("{}월 {}일 ({}) {}시부터 제{}회 원더쇼 채널이 방영될 예정입니다.", &capture["month"], &capture["date"], day_kanji_to_hangul(&capture["day"]), &capture["hour"], &capture["episode_number"])
    }) {
        return Some(wondershow_added_message);
    }

    let wondershow_starting_rule = Regex::new(r"^このあと(?P<hour>\d{1,2})時より『ワンダショちゃんねる #(?P<episode_number>\d+)』を生配信").ok()?;
    if let Some(wondershow_starting_message) = wondershow_starting_rule.captures(tweet_text).map(|capture| {
        format!("잠시 후 {}시부터 제{}회 원더쇼 채널이 방영될 예정입니다.", &capture["hour"], &capture["episode_number"])
    }) {
        return Some(wondershow_starting_message);
    }

    return None;
}

async fn cron(client: Client, db_connection: Connection) -> Result<(), Error> {
    let now = chrono::Local::now();
    let mut start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().signed_duration_since(now.naive_local());
    if start < chrono::Duration::seconds(0) {
        start = start.checked_add(&chrono::Duration::days(1)).unwrap();
    }

    let period = std::time::Duration::from_secs(24 * 60 * 60);
    let mut interval = tokio::time::interval_at(tokio::time::Instant::now() + start.to_std().unwrap(), period);

    loop {
        interval.tick().await;

        let tokyo_datetime = get_tokyo_datetime();
        let year = tokyo_datetime.year() as u32;
        let month = tokyo_datetime.month();
        let day = tokyo_datetime.day();

        let event_ends_message_vec: Vec<_> = {
            let date = year * 10000 + month * 100 + day;
            let mut statement = db_connection.prepare("SELECT id, name, end_date FROM event WHERE end_date = :end_date;")?;
            let event_ends_message_vec = statement.query_map(&[(":end_date", date.to_string().as_str())], |row| {
                Ok(Event {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    end_date: row.get(2)?,
                })
            })?
            .map(|event| format!("오늘 21시에 `{}` 이벤트가 종료됩니다.", event.unwrap().name))
            .collect();
            event_ends_message_vec
        };
        for event_ends_message in event_ends_message_vec {
            send_to_discord(&client, &event_ends_message).await?;
        }

        let birthday_message_vec: Vec<_> = {
            let date = month * 100 + day;
            let mut statement = db_connection.prepare("SELECT character_name, date, live_type FROM birthday WHERE date = :date;")?;
            let birthday_message_vec = statement.query_map(&[(":date", date.to_string().as_str())], |row| {
                Ok(Birthday {
                    character_name: row.get(0)?,
                    date: row.get(1)?,
                    live_type: row.get(2)?,
                })
            })?
            .map(|event| {
                let event = event.unwrap();
                format!("오늘은 {}의 생일입니다.\n{} 라이브를 시청하세요.", event.character_name, event.live_type)
            })
            .collect();
            birthday_message_vec
        };
        for birthday_message in birthday_message_vec {
            send_to_discord(&client, &birthday_message).await?;
        }
    }
}

// common functions

async fn send_to_discord(client: &Client, message: &str) -> Result<(), Error> {
    let discord_api_endpoint = env::var("DISCORD_API_ENDPOINT")?;
    let discord_username = env::var("DISCORD_USERNAME")?;
    let discord_profile_url = env::var("DISCORD_PROFILE_URL")?;

    let body = serde_json::json!({
        "username": discord_username,
        "avatar_url": discord_profile_url,
        "content": message
    });

    let temp_response = client
        .post(discord_api_endpoint)
        .json(&body)
        .send()
        .await?;
    print!("{:?}", temp_response);

    Ok(())
}

fn get_tokyo_datetime() -> NaiveDateTime {
    return Utc::now().naive_utc() + Duration::hours(9);
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

// database initialization

fn init_database(db_connection: &Connection) -> Result<(), Error> {
    db_connection.execute(
        "CREATE TABLE if NOT EXISTS event (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            end_date    INTEGER
        );",
        (),
    )?;

    db_connection.execute("DROP TABLE IF EXISTS birthday", ())?;
    db_connection.execute(
        "CREATE TABLE birthday (
            character_name  TEXT NOT NULL,
            date            INTEGER NOT NULL,
            live_type       TEXT NOT NULL
        );",
        (),
    )?;
    db_connection.execute("CREATE INDEX birthday_date ON birthday (date);", ())?;
    db_connection.execute("
        INSERT INTO birthday (character_name, date, live_type) VALUES
        ('히노모리 시호', 108, '생일'),
        ('아사히나 마후유', 127, '생일'),
        ('메구리네 루카', 130, '애니버서리'),
        ('요이사키 카나데', 210, '생일'),
        ('KAITO', 217, '애니버서리'),
        ('아즈사와 코하네', 302, '생일'),
        ('모모이 아이리', 319, '생일'),
        ('하나사토 미노리', 414, '생일'),
        ('시노노메 에나', 430, '생일'),
        ('텐마 사키', 509, '생일'),
        ('텐마 츠카사', 517, '생일'),
        ('아오야기 토우야', 525, '생일'),
        ('카미시로 루이', 624, '생일'),
        ('쿠사나기 네네', 720, '생일'),
        ('시라이시 안', 726, '생일'),
        ('호시노 이치카', 811, '생일'),
        ('아키야마 미즈키', 827, '생일'),
        ('하츠네 미쿠', 831, '애니버서리'),
        ('오오토리 에무', 909, '생일'),
        ('키리타니 하루카', 1005, '생일'),
        ('모치즈키 호나미', 1027, '생일'),
        ('MEIKO', 1105, '애니버서리'),
        ('시노노메 아키토', 1112, '생일'),
        ('히노모리 시즈쿠', 1206, '생일'),
        ('카가미네 린', 1227, '애니버서리'),
        ('카가미네 렌', 1227, '애니버서리');
    ", ())?;

    Ok(())
}
