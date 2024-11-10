use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use std::io::BufRead;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

const PORT_NAME: &str = "/dev/cu.usbserial-210";
const BAUD_RATE: u32 = 115_200;
const TIMEOUT_MS: u64 = 100;
const BROADCAST_INTERVAL_MS: u64 = 100;
const MAX_DATA_POINTS: usize = 1000;

#[derive(Default)]
struct EngineData {
    // Sensor data
    time: Vec<f64>,
    flow_rate_fuel: Vec<f64>,
    flow_rate_oxi: Vec<f64>,
    pulse_count_fuel: Vec<i32>,
    pulse_count_oxi: Vec<i32>,
    desired_pos_fuel: Vec<i32>,
    desired_pos_oxi: Vec<i32>,
    is_emergency: Vec<bool>,
    // Valve states
    fuel_valve_open: bool,
    oxi_valve_open: bool,
}

struct FlowRateApp {
    data: Arc<RwLock<EngineData>>,
    last_data_length: usize,
    last_fuel_valve_state: bool,
    last_oxi_valve_state: bool,
}

impl FlowRateApp {
    /// Creates a new FlowRateApp instance.
    fn new(data: Arc<RwLock<EngineData>>) -> Self {
        Self {
            data,
            last_data_length: 0,
            last_fuel_valve_state: false,
            last_oxi_valve_state: false,
        }
    }
}

impl eframe::App for FlowRateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut need_repaint = false;

        // Read the valve states and data length
        let data_read = self.data.read().unwrap();
        let data_length = data_read.time.len();

        // Check if new data has arrived
        if data_length != self.last_data_length {
            self.last_data_length = data_length;
            need_repaint = true;
        }

        // Clone valve states to avoid holding the lock during UI interaction
        let mut fuel_valve_open = data_read.fuel_valve_open;
        let mut oxi_valve_open = data_read.oxi_valve_open;
        drop(data_read); // Release the read lock as soon as possible

        // Update the UI controls
        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .toggle_value(&mut fuel_valve_open, "Fuel Valve")
                    .changed()
                {
                    need_repaint = true;
                }
                if ui
                    .toggle_value(&mut oxi_valve_open, "Oxidizer Valve")
                    .changed()
                {
                    need_repaint = true;
                }

                if ui.button("Both On").clicked() {
                    fuel_valve_open = true;
                    oxi_valve_open = true;
                    need_repaint = true;
                }
                if ui.button("Both Off").clicked() {
                    fuel_valve_open = false;
                    oxi_valve_open = false;
                    need_repaint = true;
                }
            });
        });

        // Update the shared data if valve states have changed
        if fuel_valve_open != self.last_fuel_valve_state
            || oxi_valve_open != self.last_oxi_valve_state
        {
            let mut data_write = self.data.write().unwrap();
            data_write.fuel_valve_open = fuel_valve_open;
            data_write.oxi_valve_open = oxi_valve_open;
            self.last_fuel_valve_state = fuel_valve_open;
            self.last_oxi_valve_state = oxi_valve_open;
            need_repaint = true;
        }

        // Always build and render the plots
        let data_read = self.data.read().unwrap();

        // Build PlotPoints from the data slices
        let fuel_flow_points: PlotPoints = PlotPoints::from_iter(
            data_read
                .time
                .iter()
                .zip(&data_read.flow_rate_fuel)
                .map(|(&t, &r)| [t, r]),
        );

        let oxi_flow_points: PlotPoints = PlotPoints::from_iter(
            data_read
                .time
                .iter()
                .zip(&data_read.flow_rate_oxi)
                .map(|(&t, &r)| [t, r]),
        );

        let fuel_pulse_points: PlotPoints = PlotPoints::from_iter(
            data_read
                .time
                .iter()
                .zip(&data_read.pulse_count_fuel)
                .map(|(&t, &p)| [t, p as f64]),
        );

        let oxi_pulse_points: PlotPoints = PlotPoints::from_iter(
            data_read
                .time
                .iter()
                .zip(&data_read.pulse_count_oxi)
                .map(|(&t, &p)| [t, p as f64]),
        );

        drop(data_read); // Release the read lock before rendering

        // Render the plots
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |columns| {
                columns[0].heading("Flow Rates");
                Plot::new("Flow Rates")
                    .view_aspect(2.0)
                    .legend(Legend::default())
                    .show(&mut columns[0], |plot_ui| {
                        plot_ui.line(
                            Line::new(fuel_flow_points)
                                .color(egui::Color32::RED)
                                .name("Fuel Flow Rate"),
                        );

                        plot_ui.line(
                            Line::new(oxi_flow_points)
                                .color(egui::Color32::BLUE)
                                .name("Oxidizer Flow Rate"),
                        );
                    });

                columns[1].heading("Pulse Counts");
                Plot::new("Pulse Counts")
                    .view_aspect(2.0)
                    .legend(Legend::default())
                    .show(&mut columns[1], |plot_ui| {
                        plot_ui.line(
                            Line::new(fuel_pulse_points)
                                .color(egui::Color32::RED)
                                .name("Fuel Pulse Count"),
                        );

                        plot_ui.line(
                            Line::new(oxi_pulse_points)
                                .color(egui::Color32::BLUE)
                                .name("Oxidizer Pulse Count"),
                        );
                    });
            });
        });

        // Request repaint if needed
        if need_repaint {
            ctx.request_repaint();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Shared data protected by RwLock
    let app_data = Arc::new(RwLock::new(EngineData::default()));

    // Initialize serial port
    let port = serialport::new(PORT_NAME, BAUD_RATE)
        .timeout(Duration::from_millis(TIMEOUT_MS))
        .open()
        .expect("Failed to open port");
    let app_port = Arc::new(port);

    // Serial read thread
    {
        let data = Arc::clone(&app_data);
        let port = app_port.try_clone().expect("Failed to clone port");
        thread::spawn(move || {
            let mut reader = std::io::BufReader::new(port);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(bytes_read) => {
                        if bytes_read > 0 {
                            let values: Vec<&str> = line.trim().split(',').collect();
                            if values.len() == 8 {
                                if let (
                                    Ok(time),
                                    Ok(flow_fuel),
                                    Ok(flow_oxi),
                                    Ok(pulse_fuel),
                                    Ok(pulse_oxi),
                                    Ok(pos_fuel),
                                    Ok(pos_oxi),
                                    Ok(emergency),
                                ) = (
                                    values[0].parse::<f64>(),
                                    values[1].parse::<f64>(),
                                    values[2].parse::<f64>(),
                                    values[3].parse::<i32>(),
                                    values[4].parse::<i32>(),
                                    values[5].parse::<i32>(),
                                    values[6].parse::<i32>(),
                                    values[7].parse::<bool>(),
                                ) {
                                    let mut data_write = data.write().unwrap();

                                    // Append new data
                                    data_write.time.push(time);
                                    data_write.flow_rate_fuel.push(flow_fuel);
                                    data_write.flow_rate_oxi.push(flow_oxi);
                                    data_write.pulse_count_fuel.push(pulse_fuel);
                                    data_write.pulse_count_oxi.push(pulse_oxi);
                                    data_write.desired_pos_fuel.push(pos_fuel);
                                    data_write.desired_pos_oxi.push(pos_oxi);
                                    data_write.is_emergency.push(emergency);

                                    // Limit data size
                                    if data_write.time.len() > MAX_DATA_POINTS {
                                        data_write.time.remove(0);
                                        data_write.flow_rate_fuel.remove(0);
                                        data_write.flow_rate_oxi.remove(0);
                                        data_write.pulse_count_fuel.remove(0);
                                        data_write.pulse_count_oxi.remove(0);
                                        data_write.desired_pos_fuel.remove(0);
                                        data_write.desired_pos_oxi.remove(0);
                                        data_write.is_emergency.remove(0);
                                    }
                                }
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
                    Err(e) => eprintln!("Error reading from serial port: {:?}", e),
                }
            }
        });
    }

    // Serial write thread
    {
        let data = Arc::clone(&app_data);
        let mut port = app_port.try_clone().expect("Failed to clone port");
        thread::spawn(move || {
            loop {
                // Read the valve states
                let (fuel_valve_open, oxi_valve_open) = {
                    let data_read = data.read().unwrap();
                    (data_read.fuel_valve_open, data_read.oxi_valve_open)
                };

                let msg = format!(
                    "{},{}\n",
                    if fuel_valve_open { 1 } else { 0 },
                    if oxi_valve_open { 1 } else { 0 }
                );

                if let Err(e) = port.write_all(msg.as_bytes()) {
                    eprintln!("Failed to write to serial port: {:?}", e);
                } else {
                    println!("Sent: {}", msg.trim());
                }
                thread::sleep(Duration::from_millis(BROADCAST_INTERVAL_MS));
            }
        });
    }

    // Run the GUI application
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Khan Space Industries | Ground Control System",
        native_options,
        Box::new(move |_cc| Ok(Box::new(FlowRateApp::new(app_data)))),
    )
    .expect("Failed to run the app");

    Ok(())
}
