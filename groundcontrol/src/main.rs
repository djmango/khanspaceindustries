use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use std::io::BufRead;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const PORT_NAME: &str = "/dev/cu.usbserial-210";
const BAUD_RATE: u32 = 115_200;
const TIMEOUT_MS: u64 = 100;
const BROADCAST_INTERVAL_MS: u64 = 100;

#[derive(Default)]
struct EngineData {
    time: Vec<f64>,
    flow_rate_fuel: Vec<f64>,
    flow_rate_oxi: Vec<f64>,
    pulse_count_fuel: Vec<i32>,
    pulse_count_oxi: Vec<i32>,
    desired_pos_fuel: Vec<i32>,
    desired_pos_oxi: Vec<i32>,
    is_emergency: Vec<bool>,
    fuel_valve_open: bool,
    oxi_valve_open: bool,
}

struct FlowRateApp {
    data: Arc<Mutex<EngineData>>,
}

impl eframe::App for FlowRateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut data = self.data.lock().unwrap();

        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.toggle_value(&mut data.fuel_valve_open, "Fuel Valve");
                ui.toggle_value(&mut data.oxi_valve_open, "Oxidizer Valve");

                if ui.button("Both On").clicked() {
                    data.fuel_valve_open = true;
                    data.oxi_valve_open = true;
                }
                if ui.button("Both Off").clicked() {
                    data.fuel_valve_open = false;
                    data.oxi_valve_open = false;
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |columns| {
                columns[0].heading("Flow Rates");
                Plot::new("Flow Rates")
                    .view_aspect(2.0)
                    .legend(Legend::default())
                    .show(&mut columns[0], |plot_ui| {
                        let fuel_points: PlotPoints = data
                            .time
                            .iter()
                            .zip(&data.flow_rate_fuel)
                            .map(|(&t, &r)| [t, r])
                            .collect();

                        let oxi_points: PlotPoints = data
                            .time
                            .iter()
                            .zip(&data.flow_rate_oxi)
                            .map(|(&t, &r)| [t, r])
                            .collect();

                        plot_ui.line(
                            Line::new(fuel_points)
                                .color(egui::Color32::RED)
                                .name("Fuel Flow Rate"),
                        );

                        plot_ui.line(
                            Line::new(oxi_points)
                                .color(egui::Color32::BLUE)
                                .name("Oxidizer Flow Rate"),
                        );
                    });

                columns[1].heading("Pulse Counts");
                Plot::new("Pulse Counts")
                    .view_aspect(2.0)
                    .legend(Legend::default())
                    .show(&mut columns[1], |plot_ui| {
                        let fuel_points: PlotPoints = data
                            .time
                            .iter()
                            .zip(&data.pulse_count_fuel)
                            .map(|(&t, &p)| [t, p as f64])
                            .collect();

                        let oxi_points: PlotPoints = data
                            .time
                            .iter()
                            .zip(&data.pulse_count_oxi)
                            .map(|(&t, &p)| [t, p as f64])
                            .collect();

                        plot_ui.line(
                            Line::new(fuel_points)
                                .color(egui::Color32::RED)
                                .name("Fuel Pulse Count"),
                        );

                        plot_ui.line(
                            Line::new(oxi_points)
                                .color(egui::Color32::BLUE)
                                .name("Oxidizer Pulse Count"),
                        );
                    });
            });
        });

        ctx.request_repaint();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app_data = Arc::new(Mutex::new(EngineData::default()));

    let port = serialport::new(PORT_NAME, BAUD_RATE)
        .timeout(Duration::from_millis(TIMEOUT_MS))
        .open()
        .expect("Failed to open port");
    let app_port = Arc::new(Mutex::new(port));

    {
        let data = app_data.clone();
        let port = app_port.clone();

        thread::spawn(move || {
            let port = port.lock().unwrap().try_clone().unwrap();
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
                                    let mut data = data.lock().unwrap();
                                    data.time.push(time);
                                    data.flow_rate_fuel.push(flow_fuel);
                                    data.flow_rate_oxi.push(flow_oxi);
                                    data.pulse_count_fuel.push(pulse_fuel);
                                    data.pulse_count_oxi.push(pulse_oxi);
                                    data.desired_pos_fuel.push(pos_fuel);
                                    data.desired_pos_oxi.push(pos_oxi);
                                    data.is_emergency.push(emergency);

                                    if data.time.len() > 1000 {
                                        data.time.drain(0..1);
                                        data.flow_rate_fuel.drain(0..1);
                                        data.flow_rate_oxi.drain(0..1);
                                        data.pulse_count_fuel.drain(0..1);
                                        data.pulse_count_oxi.drain(0..1);
                                        data.desired_pos_fuel.drain(0..1);
                                        data.desired_pos_oxi.drain(0..1);
                                        data.is_emergency.drain(0..1);
                                    }
                                }
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
                    Err(e) => eprintln!("Error: {:?}", e),
                }
            }
        });
    }

    {
        let port = app_port.clone();
        let app_data = app_data.clone();

        thread::spawn(move || {
            let mut port = port.lock().unwrap().try_clone().unwrap();
            loop {
                let data = app_data.lock().unwrap();
                let msg = format!(
                    "{},{}\n",
                    if data.fuel_valve_open { 1 } else { 0 },
                    if data.oxi_valve_open { 1 } else { 0 }
                );

                if let Err(e) = port.write_all(msg.as_bytes()) {
                    eprintln!("Failed to write to serial port: {:?}", e);
                } else {
                    print!("Sent: {}", msg);
                }
                thread::sleep(Duration::from_millis(BROADCAST_INTERVAL_MS));
            }
        });
    }

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Khan Space Industries | Ground Control System",
        native_options,
        Box::new(move |_cc| Ok(Box::new(FlowRateApp { data: app_data }))),
    )
    .expect("Failed to run the app");

    Ok(())
}
