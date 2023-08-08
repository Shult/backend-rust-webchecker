use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;
use tokio::time::{timeout, Duration};

// Excel
use rust_xlsxwriter::*;

// Import constants
use crate::constants::*;

// Vérifier le timeout demain.

pub async fn check_sites(sites: Vec<String>) -> Result< (Vec<String>, Vec<String>, Vec<String>, (i32, i32, i32)), Box<dyn std::error::Error>> {

    // Create the excel file
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    // Init of the counter for statistics
    let up_count = Arc::new(Mutex::new(0));
    let down_count = Arc::new(Mutex::new(0));
    let timeout_count = Arc::new(Mutex::new(0));

    let handles: Vec<_> = sites
        .into_iter()
        .map(|mut url| {

            // Verify that every url had http:// or https:// and add https:// in case
            if !url.starts_with("http://") && !url.starts_with("https://") {
                url = format!("https://{}", url);
            }
            let url_clone = url.clone();

            // Stats: Count the number of up, down and timeout websites. Because of the thread they are not simple iterator
            let up_count = Arc::clone(&up_count);
            let down_count = Arc::clone(&down_count);
            let timeout_count = Arc::clone(&timeout_count);

            task::spawn(async move {
                let result = timeout(Duration::from_secs(TIMOUT_DELAY), reqwest::get(&url_clone)).await;

                match result {
                    Ok(response) => {
                        match response {
                            // Response: Ok, the website is UP
                            Ok(resp) => {
                                println!("{} is up: Status {}", url_clone, resp.status());
                                // let mut up = up_count.lock().unwrap();
                                let mut up = up_count.lock().await; // Use await here
                                *up += 1;
                                (url_clone, "UP") // Return a tuple with the site and its status
                            },

                            // Response: Down, the website is DOWN
                            Err(err) => {
                                println!("{} is down: {}", url_clone, err);
                                // let mut down = down_count.lock().unwrap();
                                let mut down = down_count.lock().await; // And here
                                *down += 1;
                                (url_clone, "DOWN")
                            },
                        }
                    },

                    // Response: Timeout
                    Err(_) => {
                        println!("{} timeout", url_clone);
                        // let mut timeout = timeout_count.lock().unwrap();
                        let mut timeout = timeout_count.lock().await; // And here
                        *timeout += 1;
                        (url_clone, "TIMEOUT")
                    },
                }
            })
        })
        .collect();

    let mut up_sites = Vec::new();
    let mut down_sites = Vec::new();
    let mut timeout_sites = Vec::new();

    for handle in handles {
        // handle.await?;

        let (site, status) = handle.await?;
        if status=="UP"  {
            //let json_like = "{url:'".to_owned()+ &site +"', status: 'DOWN'},";
            up_sites.push(site);
        } else if status=="DOWN"  {
            down_sites.push(site);
        } else {
            timeout_sites.push(site);
        }
    }

    // Write in the excel file
    worksheet.write(0, 0, "test").unwrap();

    // Save the excel file
    workbook.save("demo.xlsx").unwrap();

    let count = (*up_count.lock().await, *down_count.lock().await, *timeout_count.lock().await);

    let reponse = (up_sites, down_sites, timeout_sites, count);
    Ok(reponse)
}
