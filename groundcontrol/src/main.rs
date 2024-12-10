use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PORT_NAME: &str = "/dev/cu.usbserial-10";
const BAUD_RATE: u32 = 115_200;
const TIMEOUT_MS: u64 = 100;
const BROADCAST_INTERVAL_MS: u64 = 100;
const MAX_DATA_POINTS: usize = 1000;

#[derive(Debug, Clone)]
struct EngineDataPoint {
    timestamp: u64, // Real date timestamp in Unix time
    time: f64,      // Time from the data
    flow_rate_fuel: f64,
    flow_rate_oxi: f64,
    pulse_count_fuel: i32,
    pulse_count_oxi: i32,
    desired_pos_fuel: i32,
    desired_pos_oxi: i32,
    fuel_valve_open: bool, // Valve states at the time of data point
    oxi_valve_open: bool,
    raw_values: String, // Raw decoded values as a string
}

#[derive(Default)]
struct EngineData {
    data_points: VecDeque<EngineDataPoint>,
    // Valve states
    fuel_valve_open: bool,
    oxi_valve_open: bool,
}

struct FlowRateApp {
    // Receiver for data points
    data_receiver: Receiver<EngineDataPoint>,
    // Shared valve states
    valve_state_sender: Sender<(bool, bool)>,
    // Local data storage
    engine_data: EngineData,
    // Latest raw decoded values
    latest_raw_values: String,
    // Log directory path
    log_dir: PathBuf,
}

impl FlowRateApp {
    /// Creates a new FlowRateApp instance.
    fn new(
        data_receiver: Receiver<EngineDataPoint>,
        valve_state_sender: Sender<(bool, bool)>,
        log_dir: PathBuf,
    ) -> Self {
        Self {
            data_receiver,
            valve_state_sender,
            engine_data: EngineData::default(),
            latest_raw_values: String::new(),
            log_dir,
        }
    }
}

impl eframe::App for FlowRateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Receive new data points
        while let Ok(data_point) = self.data_receiver.try_recv() {
            self.latest_raw_values = data_point.raw_values.clone(); // Update latest raw values
            self.engine_data.data_points.push_back(data_point);
            if self.engine_data.data_points.len() > MAX_DATA_POINTS {
                self.engine_data.data_points.pop_front();
            }
        }

        // Update the UI controls
        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            // Display current system time
            let current_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            ui.label(format!("Current Time: {}", current_time));

            ui.horizontal(|ui| {
                let mut fuel_valve_open = self.engine_data.fuel_valve_open;
                let mut oxi_valve_open = self.engine_data.oxi_valve_open;

                if ui
                    .toggle_value(&mut fuel_valve_open, "Fuel Valve")
                    .changed()
                    || ui
                        .toggle_value(&mut oxi_valve_open, "Oxidizer Valve")
                        .changed()
                {
                    self.engine_data.fuel_valve_open = fuel_valve_open;
                    self.engine_data.oxi_valve_open = oxi_valve_open;
                    // Send updated valve states
                    let _ = self
                        .valve_state_sender
                        .send((fuel_valve_open, oxi_valve_open));
                }

                if ui.button("Both On").clicked() {
                    self.engine_data.fuel_valve_open = true;
                    self.engine_data.oxi_valve_open = true;
                    // Send updated valve states
                    let _ = self.valve_state_sender.send((true, true));
                }
                if ui.button("Both Off").clicked() {
                    self.engine_data.fuel_valve_open = false;
                    self.engine_data.oxi_valve_open = false;
                    // Send updated valve states
                    let _ = self.valve_state_sender.send((false, false));
                }
            });
        });

        // Always build and render the plots
        let data_points = &self.engine_data.data_points;

        // Build PlotPoints from the data
        let (fuel_flow_points, oxi_flow_points): (Vec<_>, Vec<_>) = data_points
            .iter()
            .map(|dp| ([dp.time, dp.flow_rate_fuel], [dp.time, dp.flow_rate_oxi]))
            .unzip();

        let (fuel_pulse_points, oxi_pulse_points): (Vec<_>, Vec<_>) = data_points
            .iter()
            .map(|dp| {
                (
                    [dp.time, dp.pulse_count_fuel as f64],
                    [dp.time, dp.pulse_count_oxi as f64],
                )
            })
            .unzip();

        let (desired_pos_fuel_points, desired_pos_oxi_points): (Vec<_>, Vec<_>) = data_points
            .iter()
            .map(|dp| {
                (
                    [dp.time, dp.desired_pos_fuel as f64],
                    [dp.time, dp.desired_pos_oxi as f64],
                )
            })
            .unzip();

        // Valve states over time
        let fuel_valve_points: Vec<_> = data_points
            .iter()
            .map(|dp| ([dp.time, if dp.fuel_valve_open { 1.0 } else { 0.0 }]))
            .collect();

        let oxi_valve_points: Vec<_> = data_points
            .iter()
            .map(|dp| ([dp.time, if dp.oxi_valve_open { 1.0 } else { 0.0 }]))
            .collect();

        // Render the plots without ScrollArea
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Engine Data");

            // First Row: Flow Rates and Pulse Counts
            ui.columns(2, |columns| {
                // Flow Rates Plot
                columns[0].heading("Flow Rates");
                Plot::new("Flow Rates")
                    .view_aspect(2.0)
                    .legend(Legend::default())
                    .allow_double_click_reset(true)
                    .show(&mut columns[0], |plot_ui| {
                        plot_ui.line(
                            Line::new(PlotPoints::from(fuel_flow_points.clone()))
                                .color(egui::Color32::RED)
                                .name("Fuel Flow Rate"),
                        );

                        plot_ui.line(
                            Line::new(PlotPoints::from(oxi_flow_points.clone()))
                                .color(egui::Color32::BLUE)
                                .name("Oxidizer Flow Rate"),
                        );
                    });

                // Pulse Counts Plot
                columns[1].heading("Pulse Counts");
                Plot::new("Pulse Counts")
                    .view_aspect(2.0)
                    .legend(Legend::default())
                    .allow_double_click_reset(true)
                    .show(&mut columns[1], |plot_ui| {
                        plot_ui.line(
                            Line::new(PlotPoints::from(fuel_pulse_points.clone()))
                                .color(egui::Color32::RED)
                                .name("Fuel Pulse Count"),
                        );

                        plot_ui.line(
                            Line::new(PlotPoints::from(oxi_pulse_points.clone()))
                                .color(egui::Color32::BLUE)
                                .name("Oxidizer Pulse Count"),
                        );
                    });
            });

            // Second Row: Valve States and Desired Positions
            ui.columns(2, |columns| {
                // Valve States Plot
                columns[0].heading("Valve States");
                Plot::new("Valve States")
                    .view_aspect(2.0)
                    .allow_double_click_reset(true)
                    .show(&mut columns[0], |plot_ui| {
                        plot_ui.line(
                            Line::new(PlotPoints::from(fuel_valve_points.clone()))
                                .color(egui::Color32::RED)
                                .name("Fuel Valve Open"),
                        );

                        plot_ui.line(
                            Line::new(PlotPoints::from(oxi_valve_points.clone()))
                                .color(egui::Color32::BLUE)
                                .name("Oxidizer Valve Open"),
                        );
                    });

                // Desired Positions Plot
                columns[1].heading("Desired Positions");
                Plot::new("Desired Positions")
                    .view_aspect(2.0)
                    .legend(Legend::default())
                    .allow_double_click_reset(true)
                    .show(&mut columns[1], |plot_ui| {
                        plot_ui.line(
                            Line::new(PlotPoints::from(desired_pos_fuel_points.clone()))
                                .color(egui::Color32::RED)
                                .name("Desired Position Fuel"),
                        );

                        plot_ui.line(
                            Line::new(PlotPoints::from(desired_pos_oxi_points.clone()))
                                .color(egui::Color32::BLUE)
                                .name("Desired Position Oxidizer"),
                        );
                    });
            });
        });

        // Display latest raw decoded values at the bottom
        egui::TopBottomPanel::bottom("raw_values").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Latest Raw Values: {}", self.latest_raw_values));

                // Allocate remaining space with right-to-left layout
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), ui.available_height()),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if ui.button("Open Data Folder").clicked() {
                            if let Err(e) = open::that(&self.log_dir) {
                                eprintln!("Failed to open folder: {}", e);
                            }
                        }

                        ui.label(self.log_dir.display().to_string());
                    },
                );
            });
        });

        // Request repaint unconditionally
        ctx.request_repaint();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Channels for communication
    let (data_sender, data_receiver) = mpsc::channel::<EngineDataPoint>();
    let (valve_state_sender, valve_state_receiver) = mpsc::channel::<(bool, bool)>();

    // Shared valve states between GUI and serial read thread
    let shared_valve_states = Arc::new(Mutex::new((false, false)));

    // Initialize serial port
    let port = serialport::new(PORT_NAME, BAUD_RATE)
        .timeout(Duration::from_millis(TIMEOUT_MS))
        .open()
        .expect("Failed to open port");
    let port_clone = port.try_clone().expect("Failed to clone port");

    // Create logging directory and file
    let log_dir = create_log_directory()?;
    let log_file_path = log_dir.join("data_log.csv");
    let log_file = Arc::new(Mutex::new(File::create(&log_file_path)?));

    // Serial read thread
    {
        let data_sender = data_sender.clone();
        let shared_valve_states = shared_valve_states.clone();
        let log_file = log_file.clone();

        thread::spawn(move || {
            let mut reader = std::io::BufReader::new(port);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(bytes_read) => {
                        if bytes_read > 0 {
                            let raw_values = line.trim().to_string();
                            let values: Vec<&str> = line.trim().split(',').collect();
                            if values.len() == 8 {
                                match parse_engine_data_point(&values) {
                                    Ok(mut data_point) => {
                                        // Get the current timestamp
                                        let timestamp = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs();
                                        data_point.timestamp = timestamp;

                                        // Get current valve states
                                        let valve_states = shared_valve_states.lock().unwrap();
                                        data_point.fuel_valve_open = valve_states.0;
                                        data_point.oxi_valve_open = valve_states.1;

                                        // Store raw values
                                        data_point.raw_values = raw_values.clone();

                                        // Send data point to GUI
                                        let _ = data_sender.send(data_point.clone());

                                        // Log data point
                                        let mut log_file = log_file.lock().unwrap();
                                        let log_line = format!(
                                            "{},{},{},{},{},{},{},{},{},{}\n",
                                            timestamp,
                                            data_point.time,
                                            data_point.flow_rate_fuel,
                                            data_point.flow_rate_oxi,
                                            data_point.pulse_count_fuel,
                                            data_point.pulse_count_oxi,
                                            data_point.desired_pos_fuel,
                                            data_point.desired_pos_oxi,
                                            data_point.fuel_valve_open,
                                            data_point.oxi_valve_open,
                                        );
                                        let _ = log_file.write_all(log_line.as_bytes());
                                    }
                                    Err(e) => {
                                        eprintln!("Error parsing data: {}", e);
                                    }
                                }
                            } else {
                                eprintln!("Received unexpected number of values: {}", values.len());
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
        let shared_valve_states = shared_valve_states.clone();
        thread::spawn(move || {
            let mut port = port_clone;
            let mut last_sent_state = (false, false);

            loop {
                // Check for updated valve states
                match valve_state_receiver.try_recv() {
                    Ok(valve_states) => {
                        last_sent_state = valve_states;
                        // Update shared valve states
                        let mut shared_states = shared_valve_states.lock().unwrap();
                        *shared_states = last_sent_state;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(e) => {
                        eprintln!("Error receiving valve state: {:?}", e);
                    }
                }

                let msg = format!(
                    "{},{}\n",
                    if last_sent_state.0 { 1 } else { 0 },
                    if last_sent_state.1 { 1 } else { 0 }
                );

                if let Err(e) = port.write_all(msg.as_bytes()) {
                    eprintln!("Failed to write to serial port: {:?}", e);
                }
                // else {
                //     println!("Sent: {}", msg.trim());
                // }
                thread::sleep(Duration::from_millis(BROADCAST_INTERVAL_MS));
            }
        });
    }

    // Run the GUI application
    let native_options = eframe::NativeOptions::default();
    let app = FlowRateApp::new(data_receiver, valve_state_sender, log_dir.clone());
    eframe::run_native(
        "Khan Space Industries | Ground Control System",
        native_options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )
    .expect("Failed to run the app");

    Ok(())
}

/// Parses a slice of string values into an EngineDataPoint.
fn parse_engine_data_point(values: &[&str]) -> Result<EngineDataPoint, String> {
    if values.len() != 8 {
        return Err("Invalid number of values".to_string());
    }

    let time = values[0]
        .parse::<f64>()
        .map_err(|e| format!("Time parse error: {}", e))?;
    let flow_fuel = values[1]
        .parse::<f64>()
        .map_err(|e| format!("Flow fuel parse error: {}", e))?;
    let flow_oxi = values[2]
        .parse::<f64>()
        .map_err(|e| format!("Flow oxi parse error: {}", e))?;
    let pulse_fuel = values[3]
        .parse::<i32>()
        .map_err(|e| format!("Pulse fuel parse error: {}", e))?;
    let pulse_oxi = values[4]
        .parse::<i32>()
        .map_err(|e| format!("Pulse oxi parse error: {}", e))?;
    let pos_fuel = values[5]
        .parse::<i32>()
        .map_err(|e| format!("Pos fuel parse error: {}", e))?;
    let pos_oxi = values[6]
        .parse::<i32>()
        .map_err(|e| format!("Pos oxi parse error: {}", e))?;
    let _emergency = match values[7].parse::<i32>() {
        Ok(1) => true,
        Ok(0) => false,
        Ok(_) => return Err("Emergency value must be 0 or 1".to_string()),
        Err(e) => return Err(format!("Emergency parse error: {}", e)),
    };

    Ok(EngineDataPoint {
        timestamp: 0, // Will be set later
        time,
        flow_rate_fuel: flow_fuel,
        flow_rate_oxi: flow_oxi,
        pulse_count_fuel: pulse_fuel,
        pulse_count_oxi: pulse_oxi,
        desired_pos_fuel: pos_fuel,
        desired_pos_oxi: pos_oxi,
        fuel_valve_open: false, // Will be set later
        oxi_valve_open: false,  // Will be set later
        raw_values: String::new(),
    })
}

/// Creates a logging directory inside 'logs/' with a date-timestamped name.
fn create_log_directory() -> std::io::Result<PathBuf> {
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let dir_name = format!("KSI_Ground_Control_{}", timestamp);
    let dir_path = std::path::Path::new("logs").join(dir_name);
    fs::create_dir_all(&dir_path)?;
    Ok(dir_path)
}
