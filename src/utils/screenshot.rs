use headless_chrome::Browser;
use std::fs::File;
use std::io::Write;
use anyhow::Context;

pub fn browser_take_screenshot(browser: &Browser, url: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {

    // Create a new tab and navigate to the URL.
    let tab = browser.new_tab().context("Failed to create new tab")?;
    tab.navigate_to(url).context("Failed to navigate to URL")?;

    // Wait for the page to finish loading.
    tab.wait_until_navigated()?;

    // Capture a screenshot of the page.
    let screenshot_data = tab.capture_screenshot(
        headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
        None,
        None,
        true,
    ).context("Failed to capture screenshot")?;

    let name = url.trim_start_matches("http://").trim_start_matches("https://");

    // This will replace all non-alphanumeric characters with an underscore.
    let name = name.replace(|c: char| !c.is_alphanumeric(), "_");

    // Save the screenshot to a file.
    let filename = format!("{}/{}.png", path, name);
    let mut file = File::create(filename)?;
    file.write_all(&screenshot_data).context("Failed to write to file")?;

    tab.close(true).expect("TODO: panic message");
    Ok(())
}
