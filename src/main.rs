#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use futures_util::StreamExt;
use rfd::MessageDialog;
use tokio::{self, fs::File, io::AsyncWriteExt};
use std::{process::Command, sync::{Arc, Mutex}, time::Duration};

#[derive(Default)]
struct UpdateProgress {
    progress_value: f32,
    title: String
}

#[derive(Default)]
struct UpdateUI {
    shared_progress: Arc<Mutex<UpdateProgress>>
}

impl eframe::App for UpdateUI {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let progress_bar_size = egui::vec2(385.0, 30.0);
            let shared_progress = self.shared_progress.lock().unwrap();

            ui.add_sized(progress_bar_size, egui::ProgressBar::new(shared_progress.progress_value).show_percentage());
            frame.set_window_title(shared_progress.title.as_str());

            ctx.request_repaint();
        });
    }
}

async fn download_latest_krampui(shared_progress: Arc<Mutex<UpdateProgress>>) -> (bool, Option<String>) {
    let client = reqwest::Client::new();
    let response = match client.get("https://github.com/Pixeluted/KrampUI/releases/latest/download/KrampUI.exe")
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return (false, Some("Failed to download latest version".to_string()))    
    };

    let total_size = response.content_length().unwrap_or(0);

    let mut file = match File::create("KrampUI.exe").await {
        Ok(file) => file,
        Err(_) => return (false, Some("Failed to open file for writing!".to_string()))
    };

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    while let Some(item) = stream.next().await {
        let chunk = match item {
            Ok(chunk) => chunk,
            Err(_) => return (false, Some("Downloading failed for unknown reason!".to_string()))
        };

        match file.write_all(&chunk).await {
            Ok(_) => {},
            Err(_) => return (false, Some("Failed to write chunk!".to_string()))
        }

        downloaded += chunk.len() as u64;
        let new_progress_value = downloaded as f32 / total_size as f32;

        let mut progress_lock = shared_progress.lock().unwrap();
        progress_lock.progress_value = new_progress_value;
    }

    match file.flush().await {
        Ok(_) => {},
        Err(_) => return (false, Some("Failed to flush out the file content!".to_string()))
    }

    match file.sync_all().await {
        Ok(_) => {},
        Err(_) => return (false, Some("Failed to sync all the file content!".to_string()))
    }

    let mut progress_lock = shared_progress.lock().unwrap();
    progress_lock.title = "Update completed! Launching KrampUI in 3 seconds...".to_string();

    (true, None)
}

#[tokio::main]
async fn main() {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(400.0, 50.0)),
        resizable: false,
        always_on_top: true,
        ..Default::default()
    };

    let shared_progress = Arc::new(Mutex::new(UpdateProgress { progress_value: 0.0, title: "Downloading update...".to_string() }));
    let clone_for_gui = shared_progress.clone();
    let clone_for_progress = shared_progress.clone();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let (success, error_reason) = download_latest_krampui(clone_for_progress).await;

        if !success {
            MessageDialog::new()
                .set_title("Download failed!")
                .set_description(error_reason.unwrap())
                .show();
        } else {
            tokio::time::sleep(Duration::from_secs(3)).await;
            Command::new("./KrampUI.exe").spawn().ok();
            std::process::exit(0);
        }
    });

    eframe::run_native(
        "Downloading update...",
        options,
        Box::new(|_cc| Box::new(UpdateUI { shared_progress: clone_for_gui })),
    );
}
