use itertools::izip;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    objects: Vec<(f64, f64, egui::Color32, bool, Vec<f64>)>,

    carrier_frequency: f64,
    t: Vec<f64>,
    chirps: Vec<f64>,
    ffts: Vec<Vec<f64>>,
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
            ffts: vec![],
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

fn sample_signal(t: &[f64], frequencies: &[f64]) -> Vec<f64> {
    // For each timestamp in t, sum sin(2π f t) for all frequencies and return a Vec
    t.iter()
        .map(|&t_val| {
            let mut sum = 0.0;
            for &f in frequencies {
                sum += (2.0 * std::f64::consts::PI * f * t_val).sin();
            }
            sum
        })
        .collect()
}

fn fftspectrum(signal: &[f64], sampling_rate: f64) -> Vec<(f64, f64)> {
    let n = signal.len();
    // Compute FFT using rustfft
    // Import rustfft types
    use rustfft::{FftPlanner, num_complex::Complex};
    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(n);

    // Prepare input: convert real signal to complex
    let mut buffer: Vec<Complex<f64>> =
        signal.iter().map(|&x| Complex { re: x, im: 0.0 }).collect();
    fft.process(&mut buffer);

    // Compute magnitude spectrum (normalize)
    let norm = n as f64;
    buffer
        .iter()
        .take(n / 2)
        .enumerate()
        .map(|(i, c)| {
            let freq = i as f64 * sampling_rate / n as f64;
            let mag = (c.norm() / norm) * 2.0; // scale for single-sided spectrum
            (freq, mag)
        })
        .collect()
}

fn idx_at_t(v: &[f64], t: f64) -> usize {
    // Collect the beat frequencies at the found index for all enabled objects
    v.iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - t).abs().partial_cmp(&(*b - t).abs()).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0)
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }

    pub fn update(&mut self) {
        let samples = 10000;
        let duration = 100E-6;
        let bandwidth = 1.6E9;
        self.chirps = vec![40E-6, 20E-6];
        self.t = (0..samples)
            .map(|i| i as f64 * duration / samples as f64)
            .collect();
        // Calculate frequencies across the time vector
        let saw_values = saw(&self.t, &self.chirps);
        self.f = saw_values
            .iter()
            .map(|&s| s * bandwidth + self.carrier_frequency)
            .collect();

        for obj in self.objects.iter_mut() {
            obj.4 = beat_frequencies(
                &self.t,
                &self.f,
                obj.0,
                obj.1,
                self.carrier_frequency,
                bandwidth,
                &self.chirps,
            );
        }

        // FFT of the sampled signal (from my_plot3)
        // Use the same sampled signal as in my_plot3 overlay
        // Create FFT spectra for multiple different start times
        // For each chirp, compute the start time as the sum of previous chirp durations plus 98% of the current chirp duration
        let mut start_times = Vec::new();
        // sum of chirps (except the last one)
        for (i, &chirp) in self.chirps.iter().enumerate() {
            let sum = {
                if i > 0 {
                    self.chirps.iter().take(i).sum()
                } else {
                    0.0
                }
            };
            dbg!(&sum);
            let sum = sum + chirp * 0.98;
            start_times.push(sum);
        }
        dbg!(&start_times);

        let duration = 40E-6;
        let sampling_rate = 50E6f64;
        let n = (duration * sampling_rate).round() as usize;

        self.ffts = start_times
            .iter()
            .map(|&start| {
                let t: Vec<f64> = (0..n)
                    .map(|i| start + i as f64 * duration / (n - 1) as f64)
                    .collect();

                // Collect the beat frequencies at the found index for all enabled objects
                let idx = idx_at_t(&self.t, start);

                let mut frequencies: Vec<f64> = Vec::new();
                for obj in self.objects.iter().take(3) {
                    if obj.3 && obj.4.len() > idx {
                        frequencies.push(obj.4[idx]);
                    }
                }
                let signal = sample_signal(&t, &frequencies);

                // Only keep the magnitude part of the spectrum for plotting
                let spectrum = fftspectrum(&signal, sampling_rate);
                spectrum.iter().map(|&(_f, mag)| mag).collect::<Vec<f64>>()
            })
            .collect();
    }
}

impl eframe::App for App {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update();
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
                    let start = 5E-6;
                    let duration = 1E-6;

                    // Find the index in self.t that is closest to 'start'
                    let idx = idx_at_t(&self.t, start);

                    // Collect the beat frequencies at the found index for all enabled objects
                    let mut frequencies: Vec<f64> = Vec::new();
                    for obj in self.objects.iter().take(3) {
                        if obj.3 && obj.4.len() > idx {
                            frequencies.push(obj.4[idx]);
                        }
                    }
                    let t: Vec<f64> = (0..512)
                        .map(|i| start + i as f64 * duration / 511.0)
                        .collect();
                    let high_res_signal = sample_signal(&t, &frequencies);
                    // Plot the summed signal
                    let line = egui_plot::Line::new(
                        "Summed Beat Sine",
                        egui_plot::PlotPoints::from_iter(
                            t.iter().zip(high_res_signal.iter()).map(|(&x, &y)| [x, y]),
                        ),
                    )
                    .color(egui::Color32::YELLOW)
                    .name("Sum of sin(2π·beat_freq·t) for all objects");
                    plot_ui.line(line);

                    // Overlay sampling points
                    let sampling_rate = 50E6;
                    let n = (duration * sampling_rate).round() as usize;
                    let t: Vec<f64> = (0..n)
                        .map(|i| start + i as f64 * duration / (n - 1) as f64)
                        .collect();
                    let low_res_signal = sample_signal(&t, &frequencies);
                    // Convert t and magnitude_sample to points for plotting
                    let overlay_points: Vec<[f64; 2]> = t
                        .iter()
                        .zip(low_res_signal.iter())
                        .map(|(&tx, &my)| [tx, my])
                        .collect();
                    let points = egui_plot::Points::new("Overlay Samples", overlay_points)
                        .color(egui::Color32::RED)
                        .radius(4.0);
                    plot_ui.points(points);
                });

            egui_plot::Plot::new("fft_plot")
                .height(120.0)
                .show(ui, |plot_ui| {
                    let colors = [
                        egui::Color32::LIGHT_GREEN,
                        egui::Color32::LIGHT_BLUE,
                        egui::Color32::YELLOW,
                        egui::Color32::RED,
                        egui::Color32::WHITE,
                        egui::Color32::LIGHT_RED,
                        egui::Color32::LIGHT_YELLOW,
                        egui::Color32::LIGHT_GRAY,
                        egui::Color32::GRAY,
                        egui::Color32::BLUE,
                    ];
                    for (i, fft) in self.ffts.iter().enumerate() {
                        let line = egui_plot::Line::new(
                            format!("FFT_{i}"),
                            egui_plot::PlotPoints::from_iter(fft.iter().enumerate().map(
                                |(j, &mag)| {
                                    // Frequency axis: up to Nyquist, evenly spaced
                                    let n = fft.len();
                                    let freq = j as f64 * 50.0 / n as f64; // MHz, since sampling_rate=50E6
                                    [freq, mag]
                                },
                            )),
                        )
                        .color(colors[i % colors.len()])
                        .name(format!("FFT {i}"));
                        plot_ui.line(line);
                    }

                    // For compatibility with the code below, set spectrum to the first fft (or empty if none)
                    let spectrum: Vec<(f64, f64)> = if let Some(fft) = self.ffts.get(0) {
                        fft.iter()
                            .enumerate()
                            .map(|(j, &mag)| {
                                let n = fft.len();
                                let freq = j as f64 * 50.0 / n as f64; // MHz
                                (freq, mag)
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    // Plot the FFT magnitude
                    let line = egui_plot::Line::new(
                        "FFT Magnitude",
                        egui_plot::PlotPoints::from_iter(
                            spectrum.iter().map(|&(f, mag)| [f * 1e-6, mag]), // MHz
                        ),
                    )
                    .color(egui::Color32::LIGHT_GREEN)
                    .name("FFT |Magnitude| (MHz)");
                    plot_ui.line(line);

                    //plot_ui.set_x_axis_formatter(|x, _| format!("{:.1}", x));
                    //plot_ui.set_x_axis_label("Frequency (MHz)");
                    //plot_ui.set_y_axis_label("Magnitude");
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
