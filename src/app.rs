use itertools::izip;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    objects: Vec<(f64, f64, egui::Color32, bool, Vec<f64>)>,

    carrier_frequency: f64,
    t: Vec<f64>,
    chirps: Vec<f64>,
    f: Vec<f64>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            carrier_frequency: 77E9,
            objects: vec![
                (10.0, 0.0, egui::Color32::GREEN, true, vec![]),
                (30.0, 20.0, egui::Color32::BLUE, false, vec![]),
                (40.0, -10.0, egui::Color32::RED, false, vec![]),
            ],
            t: vec![],
            chirps: vec![],
            f: vec![],
        }
    }
}

const SPEED_OF_LIGHT: f64 = 299999000.0;

fn saw(t_: &[f64], tc: &[f64]) -> Vec<f64> {
    // period of the sawtooth
    let period = tc;
    t_.iter()
        .map(|&t| {
            // Find which Tc interval we're in, wrapping around if needed
            let total_duration: f64 = period.iter().sum();
            let t_wrapped = t % total_duration;
            let mut total_period = 0.0;
            let mut current_period = period[0];
            for &p in period.iter() {
                if t_wrapped < total_period + p {
                    current_period = p;
                    break;
                }
                total_period += p;
            }
            // normalized time within current period
            let t_mod = t_wrapped - total_period;
            // sawtooth value from 0.0 to 1.0
            t_mod / current_period
        })
        .collect()
}

fn beat_frequencies(
    t: &[f64],
    f: &[f64],
    range: f64,
    velocity: f64,
    carrier_frequency: f64,
    bandwidth: f64,
    chirps: &[f64],
) -> Vec<f64> {
    // Time shift due to range
    let timeshift_due_to_range = 2.0 * range / SPEED_OF_LIGHT;
    let time_at_range = &t
        .iter()
        .map(|ti| ti - timeshift_due_to_range)
        .collect::<Vec<f64>>();
    let saw_values_at_range = saw(&time_at_range, chirps);
    let range_frequencies: Vec<f64> = saw_values_at_range
        .iter()
        .map(|&s| s * bandwidth + carrier_frequency)
        .collect();

    // Calculate beat frequency at each time sample
    let mut beat_freqs = vec![0.0; t.len()];
    for i in 0..t.len() {
        let doppler_shift =
            f[i] * ((SPEED_OF_LIGHT - velocity) / (SPEED_OF_LIGHT + velocity) - 1.0);
        let range_shift = range_frequencies[i] - f[i];
        beat_freqs[i] = doppler_shift + range_shift;
    }

    // Return both the time vector, frequencies sent and the beat frequencies
    beat_freqs
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }
}

fn update_data(app: &mut App) {
    let samples = 10000;
    let duration = 100E-6;
    let bandwidth = 1.6E9;
    app.chirps = vec![40E-6];
    app.t = (0..samples)
        .map(|i| i as f64 * duration / samples as f64)
        .collect();
    // Calculate frequencies across the time vector
    let saw_values = saw(&app.t, &app.chirps);
    app.f = saw_values
        .iter()
        .map(|&s| s * bandwidth + app.carrier_frequency)
        .collect();

    for obj in app.objects.iter_mut() {
        obj.4 = beat_frequencies(
            &app.t,
            &app.f,
            obj.0,
            obj.1,
            app.carrier_frequency,
            bandwidth,
            &app.chirps,
        );
    }
}

impl eframe::App for App {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        update_data(self);
        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("FMCW Radar demo 0");

            for (i, obj) in self.objects.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("Object {}", i + 1));
                    ui.add(egui::Checkbox::new(&mut obj.3, ""));
                    ui.add(egui::Slider::new(&mut obj.0, 0.0..=100.0).text("Range"));
                    ui.add(egui::Slider::new(&mut obj.1, -20.0..=20.0).text("Velocity"));
                });
            }

            ui.separator();

            egui_plot::Plot::new("my_plot")
                .height(100.0)
                .auto_bounds(false)
                .default_x_bounds(0.0, 100.0)
                .default_y_bounds(-20.0, 20.0)
                .show(ui, |plot_ui| {
                    for (i, obj) in self.objects.iter().enumerate() {
                        if !obj.3 {
                            continue;
                        }
                        // Draw a sphere for each object as a circle on the plot
                        let sphere =
                            egui_plot::Points::new(format!("sphere_{i}"), vec![[obj.0, obj.1]])
                                .radius(8.0)
                                .color(obj.2);
                        plot_ui.points(sphere);
                    }
                });

            egui_plot::Plot::new("my_plot2")
                .height(100.0)
                .show(ui, |plot_ui| {
                    let line = egui_plot::Line::new(
                        "f",
                        egui_plot::PlotPoints::from_iter(
                            self.t.iter().zip(self.f.iter()).map(|(&x, &y)| [x, y]),
                        ),
                    )
                    .color(egui::Color32::LIGHT_BLUE)
                    .name("Carrier Frequency");
                    plot_ui.line(line);
                    for (i, obj) in self.objects.iter().enumerate() {
                        if !obj.3 {
                            continue;
                        }
                        let line = egui_plot::Line::new(
                            format!("bf_{i}"),
                            egui_plot::PlotPoints::from_iter(
                                izip!(self.t.iter(), obj.4.iter(), self.f.iter())
                                    .map(|(&x, &y, &f)| [x, y + f]),
                            ),
                        )
                        .color(obj.2)
                        .name(format!("Beat Frequency of Object {i}"));
                        plot_ui.line(line);
                    }
                });

            egui_plot::Plot::new("my_plot3")
                .height(100.0)
                .show(ui, |plot_ui| {
                    // Create a linspace from 0.0 to 1E-6 with 1024 points
                    let t: Vec<f64> = (0..1024).map(|i| i as f64 * 1E-6 / 1023.0).collect();
                    // Sum the sinus functions of all enabled objects' beat frequencies
                    let mut summed_signal: Vec<f64> = vec![0.0; t.len()];
                    for obj in self.objects.iter() {
                        if !obj.3 {
                            continue;
                        }
                        // Use the beat frequency at index 40 for all t
                        if obj.4.len() > 40 {
                            let bf = obj.4[40];
                            for (i, &t_val) in t.iter().enumerate() {
                                summed_signal[i] += (2.0 * std::f64::consts::PI * bf * t_val).sin();
                            }
                        }
                    }
                    // Plot the summed signal
                    let line = egui_plot::Line::new(
                        "Summed Beat Sine",
                        egui_plot::PlotPoints::from_iter(
                            self.t
                                .iter()
                                .zip(summed_signal.iter())
                                .map(|(&x, &y)| [x, y]),
                        ),
                    )
                    .color(egui::Color32::YELLOW)
                    .name("Sum of sin(2π·beat_freq·t) for all objects");
                    plot_ui.line(line);
                });

            ui.add(egui::github_link_file!(
                "https://github.com/GRASBOCK/fmcw-radar_demo-0/blob/main/",
                "Source code."
            ));

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}
