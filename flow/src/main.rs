use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use std::io::BufRead;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

struct FlowRateApp {
    data: Arc<Mutex<Vec<f64>>>,
}

impl eframe::App for FlowRateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let data = self.data.lock().unwrap();

        // Prepare data for plotting
        let points: PlotPoints = data
            .iter()
            .enumerate()
            .map(|(i, &rate)| [i as f64 * 0.1, rate]) // Assuming 0.1s interval
            .collect();

        egui::CentralPanel::default().show(ctx, |ui| {
            Plot::new("Flow Rate")
                .view_aspect(2.0)
                .legend(Legend::default())
                .show(ui, |plot_ui| {
                    plot_ui.line(
                        Line::new(points)
                            .color(egui::Color32::RED)
                            .name("Flow Rate (L/min)"),
                    );
                });
        });

        ctx.request_repaint(); // Continuously update the UI
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Shared data storage between threads
    let data = Arc::new(Mutex::new(Vec::new()));
    let data_clone = Arc::clone(&data);

    // Serial port reading in a separate thread
    thread::spawn(move || {
        let port_name = "/dev/cu.usbserial-210"; // Replace with your actual port
        let baud_rate = 115200;

        let port = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
            .expect("Failed to open port");

        let mut reader = std::io::BufReader::new(port);

        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(bytes_read) => {
                    if bytes_read > 0 {
                        if let Ok(flow_rate) = line.trim().parse::<f64>() {
                            let mut data = data_clone.lock().unwrap();
                            data.push(flow_rate);

                            // Keep only the last 100 points for real-time display
                            if data.len() > 1000 {
                                data.drain(0..1);
                            }
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
                Err(e) => eprintln!("Error: {:?}", e),
            }
        }
    });

    // Use a closure to create the app with a Result type
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Flow Rate Monitor",
        native_options,
        Box::new(move |_cc| {
            Ok(Box::new(FlowRateApp {
                data: Arc::clone(&data),
            }))
        }),
    )
    .expect("Failed to run the app"); // Handle the Result returned by `run_native`

    Ok(())
}
