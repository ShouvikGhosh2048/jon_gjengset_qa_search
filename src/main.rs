use askama::Template;
use axum::{
    extract::{Query, State},
    response::Html,
    routing::get,
    Router,
};
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;

// Youtube types for deserializing from API.
// https://stackoverflow.com/a/25877389
#[allow(non_snake_case)]
#[derive(Deserialize, Debug)]
struct VideoId {
    videoId: String,
}

#[allow(non_snake_case)]
#[derive(Deserialize, Debug)]
struct Snippet {
    title: String,
    description: String,
    resourceId: VideoId,
}

#[derive(Deserialize, Debug)]
struct Item {
    snippet: Snippet,
}

#[derive(Deserialize, Debug)]
struct YoutubeItems {
    items: Vec<Item>,
}

// Types for home template
struct Question {
    text: String,
    url: String,
}

struct Video<'a> {
    title: &'a str,
    questions: Vec<Question>,
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate<'a> {
    search: &'a str,
    videos: Vec<Video<'a>>,
}

// Type for get_videos's query
#[derive(Deserialize)]
struct GetVideosQuery {
    search: Option<String>,
}

// Taken from once_cell docs
macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

#[shuttle_runtime::main]
pub async fn axum(
    #[shuttle_secrets::Secrets] secrets: shuttle_secrets::SecretStore,
) -> shuttle_axum::ShuttleAxum {
    let shared_state = Arc::new(secrets.get("YOUTUBE_API_URL").unwrap());
    let app = Router::new()
        .route("/", get(get_videos))
        .with_state(shared_state);
    Ok(app.into())
}

async fn get_videos(
    State(youtube_api_url): State<Arc<String>>,
    Query(params): Query<GetVideosQuery>,
) -> Html<String> {
    let ctx = Client::new().get(&*youtube_api_url);
    let items = match ctx.send().await {
        Err(_) => return Html("Couldn't fetch the videos from the youtube API1".to_string()),
        Ok(response) => match response.json::<YoutubeItems>().await {
            Err(_) => return Html("Couldn't fetch the videos from the youtube API2".to_string()),
            Ok(items) => items,
        },
    };

    let timestamp_regex = regex!(r"(\d*):(\d{2}):(\d{2}) (.*)");

    let search = if let Some(search) = &params.search {
        search
    } else {
        ""
    };

    let mut home_template = HomeTemplate {
        videos: vec![],
        search,
    };

    for item in &items.items {
        let mut video = Video {
            title: &item.snippet.title,
            questions: vec![],
        };
        for cap in timestamp_regex.captures_iter(&item.snippet.description) {
            if let Some(search) = &params.search {
                if !cap[4].contains(search) {
                    continue;
                }
            }

            // Since cap[1], caps[2] and cap[3] are just digits, we can unwrap.
            let mut time: u32 = cap[3].parse().unwrap();
            time += cap[2].parse::<u32>().unwrap() * 60;
            time += cap[1].parse::<u32>().unwrap() * 60 * 60;

            let question = Question {
                text: cap[4].to_string(),
                url: format!(
                    "https://www.youtube.com/watch?v={}&t={}",
                    item.snippet.resourceId.videoId, time
                ),
            };
            video.questions.push(question);
        }
        home_template.videos.push(video);
    }
    Html(home_template.render().unwrap())
}
