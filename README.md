# nene-robo

Rust-based Project Sekai Twitter Bot

## How to use

- Create `.env` file and set environment variables below.
  - `APP_ACCESS_TOKEN`: Twitter bot token
  - `ACCOUNT_USERNAME`: username of Project Sekai official twitter account (pj_sekai)
  - `DISCORD_API_ENDPOINT`: Discord API endpoint
  - `DISCORD_USERNAME`: username displayed on Discord
  - `DISCORD_PROFILE_URL`: URL of profile image displayed on Discord
- Execute `cargo run`.

## Features

- Notifing the ongoing event will end today (keyword: `event_ends`)
- Notifing the event has ended (keyword: `event_ended`)
- Notifing the next Wondershow Channel broadcast schedule has been announced(keyword: `wondershow_added`)
- Notifing Wondershow Channel will start soon (keyword: `wondershow_starting`)
- Notifing birthday of each character (keyword: `birthday`)
