use std::fs::File;
use std::io::{self, BufRead, Write, BufReader};
use std::path::Path;

pub fn read_file(file_txt: String) -> io::Result<Vec<String>>{
    read_lines(file_txt)
}

pub fn write_to_file(file_txt: &str, content: &str) -> io::Result<()> {
    let mut file = File::create(file_txt)?;
    file.write_all(content.as_bytes())
}

fn read_lines<P>(filename: P) -> io::Result<Vec<String>>
    where
        P: AsRef<Path>,
{
    let file = File::open(filename)?;
    let reader = io::BufReader::new(file);
    reader.lines().collect()
}

pub async fn download_txt_file(url: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Fetch the content from the given URL
    let response = reqwest::get(url).await?;

    // Check if the request was successful
    if response.status().is_success() {
        // Read the text content
        let text = response.text().await?;

        // Create a new file and write the content into it
        let mut file = File::create(output_path)?;
        file.write_all(text.as_bytes())?;
        println!("Downloaded content saved to: {}", output_path);
    } else {
        println!("Failed to download the content. HTTP Status: {}", response.status());
    }

    Ok(())
}