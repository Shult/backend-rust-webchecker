#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use] extern crate rocket;
// UP FOR API
use rocket_cors::{
    AllowedHeaders, AllowedOrigins,
    CorsOptions,
};

mod utils {
    pub mod status_checker;
    pub mod read_file;
    pub mod screenshot;
    pub mod api_route;
}

mod constants;
pub use constants::*;
use crate::utils::screenshot::browser_take_screenshot;
use headless_chrome::{Browser,LaunchOptionsBuilder};

use tokio::sync::{Semaphore, Mutex};
use std::sync::{Arc};


// Date and Time
use chrono::{DateTime, Utc};

// Manage directory
use std::fs;

// Timeout screenshot
use std::time::Duration;

// Object thread-safe for the pool
use crossbeam::queue::SegQueue;
use rocket::http::Method;


// DOWN FOR API
extern crate serde;
extern crate serde_json;

use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize)]
struct RequestBody {
    urls: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct ResponseBody {
    id: usize,
    url: String,
    status: String,
    response_time: f32,
    addssl: bool,
}
// UP FOR API

use std::path::{Path, PathBuf};
use rocket::fs::NamedFile;
use crate::utils::read_file;

use rocket::response::{self, Responder};
use rocket::http::Status;

#[get("/screenshot/<date>/<file..>")]
async fn screenshot(date:String, file: PathBuf) -> Option<NamedFile> {
    NamedFile::open( Path::new("screenshot/").join(date).join(file) ).await.ok()
}

#[get("/log/<file..>")]
async fn log(file: PathBuf) -> Result<NamedFile, response::status::Custom<&'static str>> {
    let filename = format!("{}.txt", file.display());
    match NamedFile::open(Path::new("logs/").join(filename)).await {
        Ok(named_file) => {
            let url = "logs/mon_fichier.txt";
            let output_path = "downloaded_file.txt";

            if let Err(e) = read_file::download_txt_file(url, output_path).await {
                eprintln!("An error occurred: {}", e);
            }
            Ok(named_file)
        },
        Err(_) => Err(response::status::Custom(Status::InternalServerError, "Error downloading logs")),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rocket::build()
        .mount("/", routes![check_status, screenshot, log]).attach(make_cors().to_cors().unwrap()).launch().await.unwrap();
    Ok(())
}

fn make_cors() -> CorsOptions {
    let allowed_origins = AllowedOrigins::all();

    CorsOptions {
        allowed_origins,
        allowed_methods: vec![Method::Get, Method::Post, Method::Delete, Method::Options, Method::Put, Method::Patch]
            .into_iter().map(From::from).collect(),
        allowed_headers: AllowedHeaders::all(),
        allow_credentials: true,
        ..Default::default()
    }
}

#[post("/checkStatus", data = "<data>")]
async fn check_status(data: String) -> String {
    let (result_checker, formatted) = status_checker(data).await.unwrap();

    let mut result_items: Vec<String> = vec![];
    while let Some(item) = Arc::clone(&result_checker).pop() {
        result_items.push(item);
    }
    let result_api = format!("{}", result_items.join("\n"));

    println!("API =\n {}",result_api);

    let path = format!("logs/{}.txt", formatted);
    read_file::write_to_file(&path, &result_api);

    result_api
}

pub async fn status_checker(data : String) -> Result<(Arc<SegQueue<String>>, String), Box<dyn std::error::Error>> {

    // Get the number of core in the PC to optimize the number of thread
    let mut cpu_cores = num_cpus::get();
    println!("CPU core = {}", cpu_cores);

    let now: DateTime<Utc> = Utc::now();
    let formatted = now.format("%Y-%m-%d-%H-%M-%S").to_string();
    println!("{}", formatted);

    // Create the directory to store the screenshot, the name with the date will ensure that the folder is unique.
    let path_directory = format!("screenshot/{}",formatted);
    fs::create_dir_all(path_directory.clone())?;

    // Init the pool
    let sites_pool: Arc<SegQueue<String>> = Arc::new(SegQueue::new());
    let sites_pool_result: Arc<SegQueue<String>> = Arc::new(SegQueue::new());




    // Read the txt file that contain the websites to check
    let vec_of_data: Vec<String> = data.split_whitespace().map(|s| s.to_string()).collect();

    println!("daaaaaata{:?}", vec_of_data);

    // Check the status of the websites and also return the list the websites that are up
    let result = utils::status_checker::check_sites(vec_of_data).await?;

    let (up_sites, down_sites, timeout_sites, (up_count, down_count, timeout_count)) = result;

    let size = up_count;
    // Add the up websites to the pool
    for site in up_sites {
        sites_pool.push(site);
    }

    // Push down site
    for site in down_sites {
        let json_down = json!({
            "urls": &site,
            "status": "DOWN"
        });

        sites_pool_result.push(json_down.to_string());
    }

    // Push timeout site
    for site in timeout_sites {
        let json_down = json!({
            "urls": &site,
            "status": "TIMEOUT"
        });

        sites_pool_result.push(json_down.to_string());
    }

    // Timeout screenshot
    let five_seconds = Duration::new(TIMEOUT_SCREENSHOT, 0);

    if size <= cpu_cores as i32 {
        cpu_cores = 2;
    }

    // Create 4 browsers
    let browsers: Vec<Arc<Mutex<Browser>>> = (0..cpu_cores).map(|_| {
        // let browser = Browser::default().expect("Failed to create browser");
        let options = LaunchOptionsBuilder::default()
            .headless(true) // specify headless mode
            .idle_browser_timeout(five_seconds)
            .build()
            .unwrap();
        let browser = Browser::new(options).expect("Failed to create browser");
        Arc::new(Mutex::new(browser))
    }).collect();

    // Init Semaphore
    let semaphore = Arc::new(Semaphore::new(4)); // Semaphore with 4 permits
    let mut handles = vec![];

    for i in 0..cpu_cores {
        let semaphore = Arc::clone(&semaphore);
        let sites_pool = Arc::clone(&sites_pool);
        let sites_pool_result = Arc::clone(&sites_pool_result);
        let browser = Arc::clone(&browsers[i]);

        let path_directory = path_directory.clone(); // clone the directory path here

        let handle = tokio::spawn(async move {
            loop {
                // Take a permit in the semaphore
                let _permit = semaphore.acquire().await.expect("acquire failed");

                // Pick a website
                let site = match sites_pool.pop() {
                    Some(site) => site,
                    None => break, // No more sites, exit the task
                };
                println!("Site= {}", site);

                // Take a screenshot of the websites that are up
                println!("Taking a screenshot of {} ...", site);
                let browser = browser.lock().await;

                match browser_take_screenshot(&*browser, &site, &path_directory) {
                    Ok(_) => println!("Screenshot successful for site: {}\n", site),
                    Err(e) => println!("Screenshot failed for site: {}. Error: {}\n", site, e),
                }

                let name_screenshot = site.trim_start_matches("http://").trim_start_matches("https://");
                let name_screenshot = name_screenshot.replace(|c: char| !c.is_alphanumeric(), "_");

                let json_up = json!({
                    "urls": &site,
                    "status": "UP",
                    "screenshot": name_screenshot+".png"
                });

                sites_pool_result.push(json_up.to_string());
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }



    let data = json!({
        "date" : formatted,
        "up_count" : up_count,
        "down_count" : down_count,
        "timeout_count" : timeout_count
    });

    sites_pool_result.push(data.to_string());

    println!("Total up sites: {}", up_count);
    println!("Total down sites: {}", down_count);
    println!("Total timeout sites: {}", timeout_count);

    Ok((sites_pool_result, formatted))
}

struct WebsiteData {
    formatted: String,
    up_count: i32,
    down_count: i32,
    timeout_count: i32,
}