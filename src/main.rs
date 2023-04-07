use axum::{
    extract::{DefaultBodyLimit, Multipart, Query},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use serde::{de::IntoDeserializer, Deserialize};
use std::net::SocketAddr;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufWriter},
    process::Command,
};
use tower_http::limit::RequestBodyLimitLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Clone)]
enum Error {
    SaveFile,
    Other(String),
}

#[derive(Deserialize)]
struct UploadQuery {
    bitrate: String,
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let body = match self {
            Self::SaveFile => String::from("Could not save incoming file"),
            Self::Other(s) => s,
        };
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response() // ! no way to discover this method, except look at docs example
    }
}

const PREFIX: &'static str = "1M.";

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from(
            "example_multipart_form=debug,tower_http=debug",
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // build our application with some routes
    let app = Router::new()
        .route("/", get(file_upload_page))
        .route("/upload", post(upload))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(
            512 * 1024 * 1024, /* 512 MB */
        ))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    // run it with hyper
    let port: u16 = dotenvy::var("PORT")
        .expect("PORT provided")
        .parse()
        .unwrap();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn file_upload_page() -> Html<String> {
    let mut index_page = File::open("src/index.html").await.unwrap();
    let mut out_buffer = String::new();
    let _ = index_page.read_to_string(&mut out_buffer).await.unwrap();
    Html(out_buffer.clone())
}

async fn upload(query: Query<UploadQuery>, mut multipart: Multipart) -> impl IntoResponse {
    match query.bitrate.as_ref() {
        "200K" | "1M" => {
            println!("Got bitrate: {}", query.bitrate)
        }
        _ => {
            println!("Got invalid bitrate: {}", query.bitrate);
            return (StatusCode::BAD_REQUEST).into_response();
        }
    };

    let mut outer_file_name = String::new();
    let mut outer_file_size: usize = 0;
    let mut file_counter = 0;
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        let file_name = match field.file_name() {
            Some(fname) => {
                file_counter += 1;
                fname.to_string()
            }
            None => {
                println!("{} {:?}", name, field.bytes().await);
                continue;
            }
        };
        if file_counter == 2 {
            return (StatusCode::BAD_REQUEST, "Only single-file uploads").into_response();
        }

        let content_type = field.content_type().unwrap().to_string();
        let data = field.bytes().await.unwrap();
        println!(
            "Length of `{}` (`{}`: `{}`) is {} bytes",
            name,
            file_name,
            content_type,
            data.len()
        );

        // write data to a temp file
        let local_input = match File::create(format!("uploads/{}", file_name)).await {
            Ok(file) => file,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
        };
        let mut writer = BufWriter::with_capacity(1024 * 64, local_input);
        match writer.write_all(&data).await {
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
            _ => {
                outer_file_name = file_name;
                outer_file_size = data.len();
            }
        };
    }

    // pass data to ffmpeg
    {
        let command_output = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "ffmpeg -y -threads 0 -i \"uploads/{}\" -b:v {} -b:a 44K \"uploads/{}.{}\"",
                outer_file_name, query.bitrate, PREFIX, outer_file_name
            ))
            .spawn()
            .unwrap()
            .wait_with_output()
            .await;
        match command_output {
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Error in `ffmpeg`: {}", e),
                )
                    .into_response()
            }
            Ok(cmd_out) => {
                if !cmd_out.status.success() {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!(
                            "Error in `ffmpeg`: {}",
                            String::from_utf8_lossy(&cmd_out.stderr)
                        ),
                    )
                        .into_response();
                }
            }
        };
    }
    // return bytes stream hmm
    let mut response_file = File::open(format!("uploads/{}.{}", PREFIX, outer_file_name))
        .await
        .unwrap();
    let mut response_buf: Vec<u8> = Vec::with_capacity(outer_file_size);
    response_file.read_to_end(&mut response_buf).await.unwrap();
    // I am sure you can do faster
    // 1. establish a connection with an early response
    // 2. keep polling on client until EOS
    // can read ffmpeg result at the same time as streaming chunks of it to the client!
    // but I won't bother

    let _ = Command::new("rm")
        .arg(format!("uploads/{}", outer_file_name))
        .spawn(); // async?

    (StatusCode::OK, response_buf).into_response()
}
