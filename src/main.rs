use std::{env, thread, time, collections::HashMap};
use log::{info, debug, error};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use fasthash::{murmur3::Hash32, FastHash};

const API: &str = "https://discord.com/api/v10";

#[derive(Serialize, Deserialize, Debug)]
struct APIReponse {
    id: String
}

#[derive(Serialize, Deserialize, Debug)]
struct ChannelResponse {
    id: String,
    r#type: i32
}

#[derive(Serialize, Deserialize, Debug)]
struct InviteResponse {
    code: String
}

fn create_guild(client: &Client, experiment: &String) {
    let delay_secs: u64 = 60;

    let tmprange_min: u32 = 0;
    let tmprange_max: u32 = 100;

    info!("{} target ranges: {}-{}", experiment, tmprange_min, tmprange_max);

    let mut tries: u32 = 0;
    loop {
        let req = client.post(format!("{}/guilds", API))
        .json(&HashMap::from([("name", experiment)]))
        .send();

        let res = match req {
            Ok(res) => res,
            Err(error) => panic!("Error trying to create a new guild: {:?}", error),
        };

        let guild_id: String = match res.status() {
            reqwest::StatusCode::CREATED => {
                match res.json::<APIReponse>() {
                    Ok(guild) => {
                        guild.id
                    },
                    Err(err) => {
                        panic!("Could not parse guilds API response: {:?}", err);
                    },
                }
            }
            reqwest::StatusCode::UNAUTHORIZED => {
                error!("Invalid bot token");
                std::process::exit(1);
            }
            reqwest::StatusCode::TOO_MANY_REQUESTS => {
                error!("Bot got ratelimited! {:?}", res);
                std::process::exit(1);
            }
            other => {
                panic!("Unknown error while creating a guild: {:?}", other);
            }
        };

        // mmh3 hash of b"experiment:id" modulo 10,000
        let bucket: u32 = Hash32::hash(format!("{}:{}", experiment, guild_id).as_bytes()) % 10000;
        if tmprange_max > bucket && bucket > tmprange_min {
            info!("Server with {} found! ID: {}", experiment, guild_id);
            info!("Server invite: https://discord.gg/{}", create_guild_invite(client, &guild_id));
            break;
        } else {
            tries += 1;
            debug!("Attempt {} [{}; {}] failed, trying again in {} seconds.", tries, guild_id, bucket, delay_secs);

            let req = client.delete(format!("{}/guilds/{}", API, guild_id))
            .send();

            match req {
                Ok(res) => if res.status() != reqwest::StatusCode::NO_CONTENT {
                    panic!("Unknown error while deleting a guild: {:?}", res)
                },
                Err(error) => panic!("Error trying to delete a guild: {:?}", error),
            };
        }

        thread::sleep(time::Duration::from_secs(delay_secs));
    }
}

fn create_guild_invite(client: &Client, guild_id: &String) -> String {
    let req = client.get(format!("{}/guilds/{}/channels", API, guild_id))
    .send();

    let res = match req {
        Ok(res) => res,
        Err(error) => panic!("Error trying to fetch guild channels: {:?}", error),
    };

    let channel_id: String = match res.status() {
        reqwest::StatusCode::OK => {
            match res.json::<Vec<ChannelResponse>>() {
                Ok(channels) => {
                    channels.iter().find(|c| c.r#type == 0).unwrap().id.to_owned()
                },
                Err(err) => {
                    panic!("Could not parse channels API response: {:?}", err);
                },
            }
        },
        other => {
            panic!("Unknown error while fetching guild channels: {:?}", other);
        }
    };

    let req = client.post(format!("{}/channels/{}/invites", API, channel_id))
    .json(&HashMap::from([("unique", true)]))
    .send();

    let res = match req {
        Ok(res) => res,
        Err(error) => panic!("Error trying to create an invite: {:?}", error),
    };

    match res.status() {
        reqwest::StatusCode::OK => {
            match res.json::<InviteResponse>() {
                Ok(invite) => {
                    invite.code
                },
                Err(err) => {
                    panic!("Could not parse invite response: {:?}", err);
                },
            }
        },
        other => {
            panic!("Unknown error while creating an invite: {:?}", other);
        }
    }
}

fn transfer_ownership(client: &Client, guild_id: &String, user_id: &String) {
    let req = client.patch(format!("{}/guilds/{}", API, guild_id))
    .json(&HashMap::from([("owner_id", user_id)]))
    .send();

    match req {
        Ok(res) => if res.status() != reqwest::StatusCode::OK { panic!("Unknown error while transferring guild ownership: {:?}", res); },
        Err(error) => panic!("Error trying to transfer guild ownership: {:?}", error),
    };

    info!("Ownership of guild {} was transferred to {}", guild_id, user_id);
}

fn print_usage(args: &Vec<String>) {
    error!("Usage:");
    error!("{} [bot token] create [experiment id (text)]", args[0]);
    error!("{} [bot token] ownership [guild id] [user id]", args[0]);
    // TODO: list command to list all servers bot is in
    std::process::exit(1);
}

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() == 1 { print_usage(&args) };

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Authorization", format!("Bot {}", &args[1]).parse().unwrap());

    let clientbuilder = reqwest::blocking::Client::builder()
    .default_headers(headers)
    .build();

    let client = match clientbuilder {
        Ok(res) => res,
        Err(error) => panic!("Error trying to build a HTTP client: {:?}", error),
    };

    match args[2].as_str() {
        "create" => {
            if args.len() != 4 { print_usage(&args) }
            create_guild(&client, &args[3]);
        }
        "ownership" => {
            if args.len() != 5 { print_usage(&args) }
            transfer_ownership(&client, &args[3], &args[4]);
        }
        _ => print_usage(&args)
    };
}
