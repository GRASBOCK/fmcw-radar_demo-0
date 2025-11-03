/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    // Example stuff:
    label: String,

    #[serde(skip)] // This how you opt-out of serialization of a field
    value: f32,

    objects: Vec<(f64, f64, egui::Color32, bool)>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
            objects: vec![
                (10.0, 0.0, egui::Color32::GREEN, true),
                (30.0, 20.0, egui::Color32::BLUE, false),
                (40.0, -10.0, egui::Color32::RED, false),
            ],
        }
    }
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

impl eframe::App for App {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                    for (i, &(x, y, c, e)) in self.objects.iter().enumerate() {
                        if !e {
                            continue;
                        }
                        // Draw a sphere for each object as a circle on the plot
                        let sphere = egui_plot::Points::new(format!("sphere_{i}"), vec![[x, y]])
                            .radius(8.0)
                            .color(c);
                        plot_ui.points(sphere);
                    }
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
