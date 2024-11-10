use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use std::collections::VecDeque;
use std::io::BufRead;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

const PORT_NAME: &str = "/dev/cu.usbserial-210";
const BAUD_RATE: u32 = 115_200;
const TIMEOUT_MS: u64 = 100;
const BROADCAST_INTERVAL_MS: u64 = 100;
const MAX_DATA_POINTS: usize = 1000;

#[derive(Debug)]
struct EngineDataPoint {
    time: f64,
    flow_rate_fuel: f64,
    flow_rate_oxi: f64,
    pulse_count_fuel: i32,
    pulse_count_oxi: i32,
    desired_pos_fuel: i32,
    desired_pos_oxi: i32,
    is_emergency: bool,
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
}

impl FlowRateApp {
    /// Creates a new FlowRateApp instance.
    fn new(
        data_receiver: Receiver<EngineDataPoint>,
        valve_state_sender: Sender<(bool, bool)>,
    ) -> Self {
        Self {
            data_receiver,
            valve_state_sender,
            engine_data: EngineData::default(),
        }
    }
}

impl eframe::App for FlowRateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut need_repaint = false;

        // Receive new data points
        while let Ok(data_point) = self.data_receiver.try_recv() {
            self.engine_data.data_points.push_back(data_point);
            if self.engine_data.data_points.len() > MAX_DATA_POINTS {
                self.engine_data.data_points.pop_front();
            }
            need_repaint = true;
        }

        // Update the UI controls
        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
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

        // Build PlotPoints from the data slices
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

        let is_emergency_points: Vec<_> = data_points
            .iter()
            .map(|dp| ([dp.time, if dp.is_emergency { 1.0 } else { 0.0 }]))
            .collect();

        // Render the plots
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Engine Data");
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.columns(2, |columns| {
                    // Flow Rates Plot
                    columns[0].heading("Flow Rates");
                    Plot::new("Flow Rates")
                        .view_aspect(2.0)
                        .legend(Legend::default())
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

                ui.columns(2, |columns| {
                    // Desired Positions Plot
                    columns[0].heading("Desired Positions");
                    Plot::new("Desired Positions")
                        .view_aspect(2.0)
                        .legend(Legend::default())
                        .show(&mut columns[0], |plot_ui| {
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

                    // Emergency Status Plot
                    columns[1].heading("Emergency Status");
                    Plot::new("Emergency Status")
                        .view_aspect(2.0)
                        .legend(Legend::default())
                        .show(&mut columns[1], |plot_ui| {
                            plot_ui.line(
                                Line::new(PlotPoints::from(is_emergency_points.clone()))
                                    .color(egui::Color32::RED) // Changed to RED
                                    .name("Is Emergency"),
                            );
                        });
                });
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

    // Initialize serial port
    let port = serialport::new(PORT_NAME, BAUD_RATE)
        .timeout(Duration::from_millis(TIMEOUT_MS))
        .open()
        .expect("Failed to open port");
    let port_clone = port.try_clone().expect("Failed to clone port");

    // Serial read thread
    thread::spawn(move || {
        let mut reader = std::io::BufReader::new(port);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(bytes_read) => {
                    if bytes_read > 0 {
                        let values: Vec<&str> = line.trim().split(',').collect();
                        if values.len() == 8 {
                            match parse_engine_data_point(&values) {
                                Ok(data_point) => {
                                    let _ = data_sender.send(data_point);
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

    // Serial write thread
    thread::spawn(move || {
        let mut port = port_clone;
        let mut last_sent_state = (false, false);

        loop {
            // Check for updated valve states
            match valve_state_receiver.try_recv() {
                Ok(valve_states) => {
                    last_sent_state = valve_states;
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
            } else {
                println!("Sent: {}", msg.trim());
            }
            thread::sleep(Duration::from_millis(BROADCAST_INTERVAL_MS));
        }
    });

    // Run the GUI application
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Khan Space Industries | Ground Control System",
        native_options,
        Box::new(move |_cc| {
            Ok(Box::new(FlowRateApp::new(
                data_receiver,
                valve_state_sender,
            )))
        }),
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
    let emergency = match values[7].parse::<i32>() {
        Ok(1) => true,
        Ok(0) => false,
        Ok(_) => return Err("Emergency value must be 0 or 1".to_string()),
        Err(e) => return Err(format!("Emergency parse error: {}", e)),
    };

    Ok(EngineDataPoint {
        time,
        flow_rate_fuel: flow_fuel,
        flow_rate_oxi: flow_oxi,
        pulse_count_fuel: pulse_fuel,
        pulse_count_oxi: pulse_oxi,
        desired_pos_fuel: pos_fuel,
        desired_pos_oxi: pos_oxi,
        is_emergency: emergency,
    })
}
