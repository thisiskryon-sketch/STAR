use eframe::egui;
use star_backend::export::{SourceMeta, records_from_tiles, write_cells};
use star_backend::features::multiscale::extract_all_tiles;
use star_backend::preprocessing::dem::RasterGrid;
use star_backend::preprocessing::dem_batch::{TerrainExtractionReport, extract_terrain_batch};
use star_backend::preprocessing::geotiff::{GeoTiffDem, load_geotiff};
use std::f64::consts::PI;

const BG: egui::Color32 = egui::Color32::from_rgb(24, 27, 26);
const PANEL: egui::Color32 = egui::Color32::from_rgb(31, 35, 33);
const PANEL_2: egui::Color32 = egui::Color32::from_rgb(40, 45, 42);
const LINE: egui::Color32 = egui::Color32::from_rgb(58, 65, 60);
const MUTED: egui::Color32 = egui::Color32::from_rgb(164, 172, 166);
const GREEN: egui::Color32 = egui::Color32::from_rgb(166, 185, 132);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Overview,
    Terrain,
    Discovery,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layer {
    Elevation,
    Hillshade,
    Slope,
    Roughness,
    Curvature,
}
impl Layer {
    fn label(self) -> &'static str {
        match self {
            Self::Elevation => "Elevation model",
            Self::Hillshade => "Hillshade",
            Self::Slope => "Slope",
            Self::Roughness => "Roughness",
            Self::Curvature => "Curvature",
        }
    }
    fn all() -> [Self; 5] {
        [
            Self::Elevation,
            Self::Hillshade,
            Self::Slope,
            Self::Roughness,
            Self::Curvature,
        ]
    }
}

struct StarApp {
    page: Page,
    radius: usize,

    dem: Option<GeoTiffDem>,
    report: Option<TerrainExtractionReport>,
    error: Option<String>,

    layer: Layer,
    texture: Option<egui::TextureHandle>,
    texture_layer: Option<Layer>,
    selected: Option<(usize, usize)>,

    zoom: f32,
    pan: egui::Vec2,
    show_grid: bool,

    export_body: String,
    export_region: String,
    export_role: String,
    export_dem_name: String,
    export_tile_px: usize,
    export_q_min: f64,
    export_status: Option<String>,
}

impl Default for StarApp {
    fn default() -> Self {
        Self {
            page: Page::Terrain,
            radius: 2,

            dem: None,
            report: None,
            error: None,

            layer: Layer::Hillshade,
            texture: None,
            texture_layer: None,
            selected: None,

            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            show_grid: false,

            export_body: "earth".into(),
            export_region: "Unknown".into(),
            export_role: "candidate".into(),
            export_dem_name: String::new(),
            export_tile_px: 64,
            export_q_min: 0.90,
            export_status: None,
        }
    }
}
impl StarApp {
    fn load_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("GeoTIFF DEM", &["tif", "tiff"])
            .pick_file()
        {
            self.error = None;
            self.report = None;
            match load_geotiff(&path) {
                Ok(dem) => {
                    self.dem = Some(dem);
                    self.invalidate_map();
                }
                Err(e) => self.error = Some(e),
            }
        }
    }
    fn invalidate_map(&mut self) {
        self.texture = None;
        self.texture_layer = None;
        self.selected = None;
    }
    fn generate_demo(&mut self) {
        let (w, h) = (420usize, 300usize);
        let mut z = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let nx = x as f64 / w as f64;
                let ny = y as f64 / h as f64;
                let ridge = 430.0 * ((nx * PI * 3.2).sin() * (ny * PI * 2.4).cos());
                let hill = 900.0 * (-((nx - 0.57).powi(2) + (ny - 0.46).powi(2)) * 17.0).exp();
                let valleys = 170.0 * ((nx * PI * 8.0 + ny * PI * 3.0).sin());
                z.push(1100.0 + ridge + hill + valleys);
            }
        }
        match RasterGrid::new(w, h, 30.0, z) {
            Ok(grid) => {
                self.dem = Some(GeoTiffDem {
                    grid,
                    path: "synthetic_demo_dem".into(),
                    epsg: 32767,
                    crs_label: "Synthetic local metric grid".into(),
                    origin_x: 0.0,
                    origin_y: 0.0,
                    pixel_size_m: 30.0,
                    nodata: None,
                });
                self.report = None;
                self.error = None;
                self.invalidate_map();
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn run_analysis(&mut self) {
        self.error = None;
        self.report = None;
        let Some(dem) = &self.dem else {
            self.error = Some("Load a DEM before running analysis.".into());
            return;
        };
        match extract_terrain_batch(&dem.grid, self.radius) {
            Ok(r) => self.report = Some(r),
            Err(e) => self.error = Some(e),
        }
    }
    fn export_fingerprints(&mut self) {
        self.export_status = None;

        let prepared = (|| -> Result<Vec<star_backend::export::CellRecord>, String> {
            let dem = self
                .dem
                .as_ref()
                .ok_or_else(|| "Load a DEM before exporting fingerprints.".to_string())?;

            if self.export_region.trim().is_empty() {
                return Err("Region cannot be empty.".into());
            }

            let dem_name = if self.export_dem_name.trim().is_empty() {
                dem.path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unnamed_dem")
                    .to_string()
            } else {
                self.export_dem_name.trim().to_string()
            };

            let meta = SourceMeta {
                body: self.export_body.clone(),
                region: self.export_region.trim().to_string(),
                role: self.export_role.clone(),
                dem: dem_name,
                epsg: dem.epsg,
                origin_x: dem.origin_x,
                origin_y: dem.origin_y,
            };

            let tiles = extract_all_tiles(&dem.grid, self.export_tile_px, self.export_q_min);

            if tiles.is_empty() {
                return Err("No valid tiles found. Reduce tile size or lower minimum Q.".into());
            }

            records_from_tiles(&tiles, &meta)
        })();

        let records = match prepared {
            Ok(records) => records,
            Err(error) => {
                self.export_status = Some(format!("ERROR: {error}"));
                return;
            }
        };

        let count = records.len();

        let average_q = if count > 0 {
            records.iter().map(|cell| cell.quality_q).sum::<f64>() / count as f64
        } else {
            0.0
        };

        let Some(path) = rfd::FileDialog::new()
            .add_filter("STAR cells JSON", &["json"])
            .set_file_name("cells.json")
            .save_file()
        else {
            self.export_status = Some("Export cancelled.".into());
            return;
        };

        match write_cells(&path, records) {
            Ok(()) => {
                self.export_status = Some(format!(
                    "SUCCESS | {} fingerprints | Mean Q: {:.3} | {}",
                    count,
                    average_q,
                    path.display()
                ));
            }
            Err(error) => {
                self.export_status = Some(format!("EXPORT FAILED: {error}"));
            }
        }
    }
    fn color_ramp(t: f64, stops: &[(f64, [u8; 3])]) -> egui::Color32 {
        let t = t.clamp(0.0, 1.0);
        let mut a = stops[0];
        let mut b = stops[stops.len() - 1];
        for pair in stops.windows(2) {
            if t >= pair[0].0 && t <= pair[1].0 {
                a = pair[0];
                b = pair[1];
                break;
            }
        }
        let q = if b.0 > a.0 {
            (t - a.0) / (b.0 - a.0)
        } else {
            0.0
        };
        let c = |i: usize| (a.1[i] as f64 + (b.1[i] as f64 - a.1[i] as f64) * q).round() as u8;
        egui::Color32::from_rgb(c(0), c(1), c(2))
    }
    fn elevation_color(t: f64) -> egui::Color32 {
        Self::color_ramp(
            t,
            &[
                (0.0, [13, 65, 91]),
                (0.22, [20, 119, 112]),
                (0.43, [58, 145, 82]),
                (0.62, [151, 165, 54]),
                (0.8, [190, 111, 42]),
                (1.0, [145, 47, 36]),
            ],
        )
    }
    fn sample_metric(g: &RasterGrid, x: usize, y: usize) -> Option<(f64, f64, f64, f64)> {
        let z = g.get(x, y)?;
        let dx = g.cell_size_m.max(0.001);
        let l = g.get(x.saturating_sub(1), y).unwrap_or(z);
        let r = g.get((x + 1).min(g.width - 1), y).unwrap_or(z);
        let u = g.get(x, y.saturating_sub(1)).unwrap_or(z);
        let d = g.get(x, (y + 1).min(g.height - 1)).unwrap_or(z);
        let gx = (r - l) / (2.0 * dx);
        let gy = (d - u) / (2.0 * dx);
        let slope = (gx * gx + gy * gy).sqrt().atan().to_degrees();
        let shade =
            (0.48 + 0.72 * ((-gx * 0.5) + (-gy * 0.5) + 0.707).max(0.0) / 1.7).clamp(0.35, 1.15);
        let lap = (l + r + u + d - 4.0 * z) / (dx * dx);
        let mut sum = 0.0;
        let mut sum2 = 0.0;
        let mut n = 0.0;
        for yy in y.saturating_sub(1)..=(y + 1).min(g.height - 1) {
            for xx in x.saturating_sub(1)..=(x + 1).min(g.width - 1) {
                if let Some(v) = g.get(xx, yy) {
                    sum += v;
                    sum2 += v * v;
                    n += 1.0;
                }
            }
        }
        let rough = if n > 0.0 {
            (sum2 / n - (sum / n).powi(2)).max(0.0).sqrt()
        } else {
            0.0
        };
        Some((slope, shade, rough, lap))
    }
    fn sample_relief(g: &RasterGrid, x: usize, y: usize) -> Option<(f64, f64, f64)> {
        let z = g.get(x, y)?;
        let dx = g.cell_size_m.max(0.001);
        let l = g.get(x.saturating_sub(1), y).unwrap_or(z);
        let r = g.get((x + 1).min(g.width - 1), y).unwrap_or(z);
        let u = g.get(x, y.saturating_sub(1)).unwrap_or(z);
        let d = g.get(x, (y + 1).min(g.height - 1)).unwrap_or(z);
        let gx = (r - l) / (2.0 * dx);
        let gy = (d - u) / (2.0 * dx);
        let slope = (gx * gx + gy * gy).sqrt().atan().to_degrees();
        let shade =
            (0.48 + 0.72 * ((-gx * 0.5) + (-gy * 0.5) + 0.707).max(0.0) / 1.7).clamp(0.35, 1.15);
        let lap = (l + r + u + d - 4.0 * z) / (dx * dx);
        Some((slope, shade, lap))
    }
    fn build_texture(&mut self, ctx: &egui::Context) {
        if self.texture.is_some() && self.texture_layer == Some(self.layer) {
            return;
        }
        let Some(dem) = &self.dem else {
            self.texture = None;
            return;
        };
        let g = &dem.grid;
        let Some((min, max)) = g.min_max() else {
            return;
        };
        let scale = (g.width.max(g.height) as f64 / 900.0).max(1.0);
        let w = ((g.width as f64 / scale).ceil() as usize).max(1);
        let h = ((g.height as f64 / scale).ceil() as usize).max(1);
        let mut pixels = Vec::with_capacity(w * h);
        let mut metric_max = 0.0_f64;
        if self.layer == Layer::Slope || self.layer == Layer::Roughness {
            for y in (0..g.height).step_by(scale.ceil() as usize) {
                for x in (0..g.width).step_by(scale.ceil() as usize) {
                    if let Some(m) = Self::sample_metric(g, x, y) {
                        metric_max =
                            metric_max.max(if self.layer == Layer::Slope { m.0 } else { m.2 });
                    }
                }
            }
        }
        let metric_max = metric_max.max(1e-9);
        for py in 0..h {
            for px in 0..w {
                let x = ((px as f64 * scale).round() as usize).min(g.width - 1);
                let y = ((py as f64 * scale).round() as usize).min(g.height - 1);
                let Some(v) = g.get(x, y) else {
                    pixels.push(egui::Color32::TRANSPARENT);
                    continue;
                };
                let t = (v - min) / (max - min).max(1e-9);
                let color = match self.layer {
                    Layer::Elevation => Self::elevation_color(t),
                    Layer::Hillshade => {
                        let base = Self::elevation_color(t);
                        let sh = Self::sample_relief(g, x, y)
                            .map(|m| m.1 as f32)
                            .unwrap_or(0.5);
                        egui::Color32::from_rgb(
                            (base.r() as f32 * sh).clamp(0., 255.) as u8,
                            (base.g() as f32 * sh).clamp(0., 255.) as u8,
                            (base.b() as f32 * sh).clamp(0., 255.) as u8,
                        )
                    }
                    Layer::Slope => Self::color_ramp(
                        Self::sample_metric(g, x, y)
                            .map(|m| m.0 / metric_max)
                            .unwrap_or(0.0),
                        &[
                            (0., [24, 83, 130]),
                            (0.35, [43, 157, 150]),
                            (0.7, [232, 184, 66]),
                            (1., [177, 53, 43]),
                        ],
                    ),
                    Layer::Roughness => Self::color_ramp(
                        Self::sample_metric(g, x, y)
                            .map(|m| m.2 / metric_max)
                            .unwrap_or(0.0),
                        &[
                            (0., [20, 90, 120]),
                            (0.4, [63, 160, 115]),
                            (0.75, [220, 174, 61]),
                            (1., [158, 52, 43]),
                        ],
                    ),
                    Layer::Curvature => {
                        let c = Self::sample_relief(g, x, y).map(|m| m.2).unwrap_or(0.0);
                        let q = (c * 10000.0).tanh();
                        if q >= 0.0 {
                            egui::Color32::from_rgb(
                                (55.0 + 150.0 * q) as u8,
                                (75.0 + 30.0 * q) as u8,
                                45,
                            )
                        } else {
                            egui::Color32::from_rgb(
                                45,
                                (80.0 + 50.0 * q) as u8,
                                (120.0 - 70.0 * q) as u8,
                            )
                        }
                    }
                };
                pixels.push(color);
            }
        }
        let image = egui::ColorImage {
            size: [w, h],
            pixels,
        };
        self.texture = Some(ctx.load_texture(
            format!("star-{:?}", self.layer),
            image,
            egui::TextureOptions::LINEAR,
        ));
        self.texture_layer = Some(self.layer);
    }
    fn panel_frame<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        egui::Frame::none()
            .fill(PANEL)
            .stroke(egui::Stroke::new(1.0_f32, LINE))
            .inner_margin(egui::Margin::same(13))
            .show(ui, add)
            .inner
    }
    fn metric_tile(ui: &mut egui::Ui, label: &str, value: String, unit: &str) {
        egui::Frame::none()
            .fill(PANEL_2)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(label.to_uppercase())
                        .size(9.)
                        .color(GREEN),
                );
                ui.label(
                    egui::RichText::new(value)
                        .size(21.)
                        .strong()
                        .color(egui::Color32::WHITE),
                );
                ui.label(egui::RichText::new(unit).size(9.).color(MUTED));
            });
    }
    fn topbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("star-topbar")
            .exact_height(58.)
            .frame(
                egui::Frame::none()
                    .fill(BG)
                    .stroke(egui::Stroke::new(1_f32, LINE)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(10.);
                    ui.label(egui::RichText::new("☰").size(22.).color(MUTED));
                    ui.add_space(12.);
                    ui.label(
                        egui::RichText::new("S.T.A.R.")
                            .size(24.)
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("TERRAIN INTELLIGENCE")
                                .strong()
                                .size(13.),
                        );
                        ui.label(
                            egui::RichText::new("SPATIO-TEMPORAL & PHYSICAL ANALOG DISCOVERY")
                                .size(9.)
                                .color(GREEN),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(GREEN, "● ENGINE ONLINE");
                        ui.add_space(18.);
                        if ui
                            .selectable_label(self.page == Page::Discovery, "Analog Discovery")
                            .clicked()
                        {
                            self.page = Page::Discovery;
                        }
                        if ui
                            .selectable_label(self.page == Page::Terrain, "Terrain Analysis")
                            .clicked()
                        {
                            self.page = Page::Terrain;
                        }
                        if ui
                            .selectable_label(self.page == Page::Overview, "Overview")
                            .clicked()
                        {
                            self.page = Page::Overview;
                        }
                    });
                });
            });
    }
    fn left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("star-project-panel")
            .resizable(true)
            .default_width(315.)
            .width_range(275.0..=390.0)
            .frame(
                egui::Frame::none()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1_f32, LINE)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Self::panel_frame(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("▣  PROJECT INFORMATION")
                                        .strong()
                                        .size(12.),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("⋯").clicked() {}
                                    },
                                );
                            });
                            ui.add_space(8.);
                            if let Some(d) = &self.dem {
                                let (lo, hi) = d.grid.min_max().unwrap_or((0., 0.));
                                ui.columns(2, |c| {
                                    Self::metric_tile(
                                        &mut c[0],
                                        "Elevation",
                                        format!("{:.0} m", (lo + hi) / 2.),
                                        "mean raster height",
                                    );
                                    Self::metric_tile(
                                        &mut c[1],
                                        "Resolution",
                                        format!("{:.1} m", d.pixel_size_m),
                                        "ground sample distance",
                                    );
                                });
                                ui.add_space(6.);
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} × {}  ·  {}",
                                        d.grid.width, d.grid.height, d.crs_label
                                    ))
                                    .size(10.)
                                    .color(MUTED),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("No terrain dataset loaded").color(MUTED),
                                );
                            }
                        });
                        ui.add_space(7.);
                        Self::panel_frame(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("▤  INFORMATION LAYERS")
                                        .strong()
                                        .size(12.),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(egui::RichText::new("⌄").color(MUTED));
                                    },
                                );
                            });
                            ui.add_space(6.);
                            for layer in Layer::all() {
                                let active = self.layer == layer;
                                let (_color, desc) = match layer {
                                    Layer::Elevation => {
                                        (egui::Color32::from_rgb(209, 165, 67), "DEM")
                                    }
                                    Layer::Hillshade => {
                                        (egui::Color32::from_rgb(83, 180, 174), "SHADE")
                                    }
                                    Layer::Slope => (egui::Color32::from_rgb(221, 130, 67), "DEG"),
                                    Layer::Roughness => {
                                        (egui::Color32::from_rgb(132, 190, 105), "STD")
                                    }
                                    Layer::Curvature => {
                                        (egui::Color32::from_rgb(147, 137, 214), "LAP")
                                    }
                                };
                                ui.horizontal(|ui| {
                                    let r = ui.selectable_label(
                                        active,
                                        egui::RichText::new(format!("◈  {}", layer.label())).color(
                                            if active { egui::Color32::WHITE } else { MUTED },
                                        ),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(if active {
                                                    "●"
                                                } else {
                                                    "○"
                                                })
                                                .color(if active { GREEN } else { MUTED }),
                                            );
                                            ui.label(
                                                egui::RichText::new(desc).size(8.).color(MUTED),
                                            );
                                        },
                                    );
                                    if r.clicked() {
                                        self.layer = layer;
                                        self.texture = None;
                                        self.texture_layer = None;
                                    }
                                });
                            }
                        });
                        ui.add_space(7.);
                        Self::panel_frame(ui, |ui| {
                            ui.label(
                                egui::RichText::new("⌁  ELEVATION PROFILE")
                                    .strong()
                                    .size(12.),
                            );
                            ui.add_space(6.);
                            if let Some(d) = &self.dem {
                                Self::profile_chart(ui, &d.grid);
                            } else {
                                ui.label(
                                    egui::RichText::new("Load a DEM to calculate profile")
                                        .size(10.)
                                        .color(MUTED),
                                );
                            }
                        });
                        ui.add_space(7.);
                        Self::panel_frame(ui, |ui| {
                            ui.label(egui::RichText::new("DATA SOURCE").strong().size(11.));
                            if ui.button("＋  Load GeoTIFF").clicked() {
                                self.load_file();
                            }
                            if ui.button("◈  Generate demo terrain").clicked() {
                                self.generate_demo();
                            }
                            if ui.button("▶  Run terrain analysis").clicked() {
                                self.run_analysis();
                            }
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Neighborhood").size(10.).color(MUTED),
                                );
                                ui.add(egui::Slider::new(&mut self.radius, 1..=8).suffix(" px"));
                            });
                            if let Some(e) = &self.error {
                                ui.colored_label(egui::Color32::LIGHT_RED, e);
                            }
                        });
                        ui.add_space(7.);
                        self.export_panel(ui);
                    });
            });
    }
    fn export_panel(&mut self, ui: &mut egui::Ui) {
        Self::panel_frame(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("⬡  FINGERPRINT EXPORT")
                        .strong()
                        .size(12.),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("JSON / TEMP-0").size(8.).color(GREEN));
                });
            });

            ui.add_space(8.);

            ui.label(egui::RichText::new("PLANETARY BODY").size(9.).color(GREEN));

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.export_body, "earth".to_string(), "Earth");

                ui.selectable_value(&mut self.export_body, "moon".to_string(), "Moon");
            });

            ui.add_space(5.);

            ui.label(egui::RichText::new("REGION").size(9.).color(GREEN));

            ui.add(
                egui::TextEdit::singleline(&mut self.export_region)
                    .hint_text("e.g. Iceland Highlands")
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(5.);

            ui.label(egui::RichText::new("DATASET ROLE").size(9.).color(GREEN));

            egui::ComboBox::from_id_source("star-export-role")
                .selected_text(match self.export_role.as_str() {
                    "known_analog" => "Known analog",
                    "negative_control" => "Negative control",
                    "target" => "Target",
                    _ => "Candidate",
                })
                .width(f32::INFINITY)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.export_role,
                        "candidate".to_string(),
                        "Candidate",
                    );

                    ui.selectable_value(
                        &mut self.export_role,
                        "known_analog".to_string(),
                        "Known analog",
                    );

                    ui.selectable_value(
                        &mut self.export_role,
                        "negative_control".to_string(),
                        "Negative control",
                    );

                    ui.selectable_value(&mut self.export_role, "target".to_string(), "Target");
                });

            ui.add_space(5.);

            ui.label(egui::RichText::new("DEM IDENTIFIER").size(9.).color(GREEN));

            ui.add(
                egui::TextEdit::singleline(&mut self.export_dem_name)
                    .hint_text("Empty = use source filename")
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(10.);
            ui.separator();

            ui.label(egui::RichText::new("TILE CONFIGURATION").strong().size(10.));

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tile size").size(10.).color(MUTED));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{} px", self.export_tile_px))
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                });
            });

            ui.add(egui::Slider::new(&mut self.export_tile_px, 16..=256).text("pixels"));

            ui.add_space(4.);

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Minimum quality Q")
                        .size(10.)
                        .color(MUTED),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{:.2}", self.export_q_min))
                            .strong()
                            .color(GREEN),
                    );
                });
            });

            ui.add(egui::Slider::new(&mut self.export_q_min, 0.50..=1.00).fixed_decimals(2));

            ui.add_space(8.);

            ui.label(
                egui::RichText::new("Multiscale features: 1x / 3x / 9x")
                    .size(9.)
                    .color(MUTED),
            );

            ui.label(
                egui::RichText::new(
                    "Shadow metrics will be populated when the illumination module is integrated.",
                )
                .size(9.)
                .color(MUTED),
            );

            ui.add_space(9.);

            let can_export = self.dem.is_some();

            if ui
                .add_enabled(
                    can_export,
                    egui::Button::new(
                        egui::RichText::new("GENERATE & EXPORT CELLS.JSON")
                            .strong()
                            .color(egui::Color32::WHITE),
                    )
                    .fill(egui::Color32::from_rgb(24, 91, 75)),
                )
                .clicked()
            {
                self.export_fingerprints();
            }

            if !can_export {
                ui.label(
                    egui::RichText::new("Load a DEM to enable export.")
                        .size(9.)
                        .color(MUTED),
                );
            }

            if let Some(status) = &self.export_status {
                ui.add_space(7.);

                let success = status.starts_with("SUCCESS");

                ui.label(egui::RichText::new(status).size(9.).color(if success {
                    GREEN
                } else {
                    egui::Color32::LIGHT_RED
                }));
            }
        });
    }
    fn profile_chart(ui: &mut egui::Ui, g: &RasterGrid) {
        let row = g.height / 2;
        let vals: Vec<f64> = (0..g.width).filter_map(|x| g.get(x, row)).collect();
        if vals.len() < 2 {
            return;
        }
        let min = vals.iter().copied().fold(f64::INFINITY, f64::min);
        let max = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let rect = ui.allocate_space(egui::vec2(ui.available_width(), 142.)).1;
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 4., PANEL_2);
        for i in 0..4 {
            let y = rect.top() + rect.height() * i as f32 / 3.;
            p.line_segment(
                [
                    egui::pos2(rect.left() + 5., y),
                    egui::pos2(rect.right() - 5., y),
                ],
                egui::Stroke::new(1_f32, LINE),
            );
        }
        let pts: Vec<_> = vals
            .iter()
            .enumerate()
            .map(|(i, v)| {
                egui::pos2(
                    rect.left() + 5. + (rect.width() - 10.) * i as f32 / (vals.len() - 1) as f32,
                    rect.bottom()
                        - 5.
                        - ((*v - min) / (max - min).max(1e-9)) as f32 * (rect.height() - 10.),
                )
            })
            .collect();
        p.add(egui::Shape::line(pts, egui::Stroke::new(1.8_f32, GREEN)));
        p.text(
            rect.left_top() + egui::vec2(7., 5.),
            egui::Align2::LEFT_TOP,
            format!("{max:.0} m"),
            egui::FontId::proportional(9.),
            MUTED,
        );
        p.text(
            rect.left_bottom() + egui::vec2(7., -5.),
            egui::Align2::LEFT_BOTTOM,
            format!("{min:.0} m"),
            egui::FontId::proportional(9.),
            MUTED,
        );
    }
    fn right_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("star-inspector")
            .resizable(true)
            .default_width(260.)
            .width_range(230.0..=330.0)
            .frame(
                egui::Frame::none()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1_f32, LINE)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    Self::panel_frame(ui, |ui| {
                        ui.label(egui::RichText::new("⌖  POINT INSPECTOR").strong().size(12.));
                        ui.add_space(9.);
                        if let (Some(d), Some((x, y))) = (&self.dem, self.selected) {
                            let g = &d.grid;
                            ui.label(egui::RichText::new("SELECTED PIXEL").size(9.).color(GREEN));
                            ui.heading(format!("{x} , {y}"));
                            let z = g.get(x, y);
                            ui.separator();
                            if let Some(z) = z {
                                Self::detail(ui, "Elevation", format!("{z:.2} m"));
                                if let Some((s, _, r, c)) = Self::sample_metric(g, x, y) {
                                    Self::detail(ui, "Slope", format!("{s:.2}°"));
                                    Self::detail(ui, "Roughness", format!("{r:.2} m"));
                                    Self::detail(ui, "Curvature proxy", format!("{c:.6}"));
                                }
                            } else {
                                ui.label("NoData cell");
                            }
                            let mx = d.origin_x + (x as f64 + 0.5) * d.pixel_size_m;
                            let my = d.origin_y - (y as f64 + 0.5) * d.pixel_size_m;
                            Self::detail(ui, "Map X", format!("{mx:.2}"));
                            Self::detail(ui, "Map Y", format!("{my:.2}"));
                        } else {
                            ui.label(
                                egui::RichText::new(
                                    "Click anywhere on the terrain map to inspect a raster cell.",
                                )
                                .size(11.)
                                .color(MUTED),
                            );
                        }
                    });
                    ui.add_space(8.);
                    Self::panel_frame(ui, |ui| {
                        ui.label(egui::RichText::new("MAP LEGEND").strong().size(12.));
                        ui.label(
                            egui::RichText::new(self.layer.label())
                                .size(10.)
                                .color(MUTED),
                        );
                        let stops = match self.layer {
                            Layer::Elevation | Layer::Hillshade => vec![
                                ("LOW", egui::Color32::from_rgb(13, 65, 91)),
                                ("MID", egui::Color32::from_rgb(58, 145, 82)),
                                ("HIGH", egui::Color32::from_rgb(145, 47, 36)),
                            ],
                            Layer::Slope => vec![
                                ("0°", egui::Color32::from_rgb(24, 83, 130)),
                                ("Moderate", egui::Color32::from_rgb(232, 184, 66)),
                                ("Steep", egui::Color32::from_rgb(177, 53, 43)),
                            ],
                            _ => vec![
                                ("Low", egui::Color32::from_rgb(20, 90, 120)),
                                ("Medium", egui::Color32::from_rgb(63, 160, 115)),
                                ("High", egui::Color32::from_rgb(158, 52, 43)),
                            ],
                        };
                        for (label, col) in stops {
                            ui.horizontal(|ui| {
                                ui.colored_label(col, "■");
                                ui.label(egui::RichText::new(label).size(10.).color(MUTED));
                            });
                        }
                    });
                    ui.add_space(8.);
                    Self::panel_frame(ui, |ui| {
                        ui.label(egui::RichText::new("MAP CONTROLS").strong().size(12.));
                        ui.horizontal(|ui| {
                            if ui.button("＋").clicked() {
                                self.zoom = (self.zoom * 1.25).clamp(0.35, 8.0);
                            }
                            if ui.button("−").clicked() {
                                self.zoom = (self.zoom / 1.25).clamp(0.35, 8.0);
                            }
                        });
                        ui.label(
                            egui::RichText::new("Drag map to pan · Click to inspect")
                                .size(10.)
                                .color(MUTED),
                        );
                    });
                });
            });
    }
    fn detail(ui: &mut egui::Ui, label: &str, value: String) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).size(10.).color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(value).size(11.).strong());
            });
        });
    }
    fn map_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some((gw, gh, path)) = self
            .dem
            .as_ref()
            .map(|d| (d.grid.width, d.grid.height, d.path.display().to_string()))
        else {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("S.T.A.R. Terrain Workspace");
                    ui.label(
                        egui::RichText::new(
                            "Load a projected GeoTIFF DEM or generate sample terrain to begin.",
                        )
                        .color(MUTED),
                    );
                    if ui.button("Generate demo terrain").clicked() {
                        self.generate_demo();
                    }
                });
            });
            return;
        };
        self.build_texture(ctx);
        let Some(tex) = self.texture.clone() else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{}  /  {}", path, self.layer.label()))
                    .size(10.)
                    .color(MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Grid").clicked() {
                    self.show_grid = !self.show_grid;
                }
                if ui.button("Reset view").clicked() {
                    self.zoom = 1.;
                    self.pan = egui::Vec2::ZERO;
                }
                ui.label(format!("{}%", (self.zoom * 100.) as u32));
            });
        });
        ui.add_space(5.);
        let available = ui.available_size().max(egui::vec2(200., 200.));
        let aspect = gw as f32 / gh as f32;
        let base = (available.x / aspect).min(available.y).max(120.);
        let base_size = egui::vec2(base * aspect, base);
        let size = base_size * self.zoom;
        let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        let map_rect = egui::Rect::from_center_size(
            rect.center() + self.pan,
            size.min(egui::vec2(available.x * 2.5, available.y * 2.5)),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 3., egui::Color32::from_rgb(4, 13, 22));
        painter.image(
            tex.id(),
            map_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            egui::Color32::WHITE,
        );
        if self.show_grid {
            for i in 1..5 {
                let x = map_rect.left() + map_rect.width() * i as f32 / 5.;
                let y = map_rect.top() + map_rect.height() * i as f32 / 5.;
                painter.line_segment(
                    [
                        egui::pos2(x, map_rect.top()),
                        egui::pos2(x, map_rect.bottom()),
                    ],
                    egui::Stroke::new(0.7_f32, egui::Color32::from_white_alpha(55)),
                );
                painter.line_segment(
                    [
                        egui::pos2(map_rect.left(), y),
                        egui::pos2(map_rect.right(), y),
                    ],
                    egui::Stroke::new(0.7_f32, egui::Color32::from_white_alpha(55)),
                );
            }
        }
        painter.rect_stroke(
            map_rect,
            2.,
            egui::Stroke::new(1_f32, egui::Color32::from_rgb(91, 126, 137)),
            egui::StrokeKind::Inside,
        );
        if let Some((x, y)) = self.selected {
            let px = map_rect.left() + map_rect.width() * x as f32 / gw as f32;
            let py = map_rect.top() + map_rect.height() * y as f32 / gh as f32;
            let pos = egui::pos2(px, py);
            painter.circle_stroke(pos, 8., egui::Stroke::new(2_f32, egui::Color32::WHITE));
            painter.line_segment(
                [pos - egui::vec2(13., 0.), pos + egui::vec2(13., 0.)],
                egui::Stroke::new(1_f32, egui::Color32::WHITE),
            );
            painter.line_segment(
                [pos - egui::vec2(0., 13.), pos + egui::vec2(0., 13.)],
                egui::Stroke::new(1_f32, egui::Color32::WHITE),
            );
        }
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                if map_rect.contains(pos) {
                    let nx = ((pos.x - map_rect.left()) / map_rect.width()).clamp(0., 0.999999);
                    let ny = ((pos.y - map_rect.top()) / map_rect.height()).clamp(0., 0.999999);
                    self.selected = Some(((nx * gw as f32) as usize, (ny * gh as f32) as usize));
                }
            }
        }
        if response.dragged() {
            self.pan += response.drag_delta();
        }
        if response.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.zoom = (self.zoom * (1.0 + scroll * 0.001)).clamp(0.35, 8.0);
            }
        }
        let legend_rect = egui::Rect::from_min_size(
            rect.right_bottom() - egui::vec2(172., 112.),
            egui::vec2(158., 98.),
        );
        painter.rect_filled(legend_rect, 6., egui::Color32::from_black_alpha(205));
        painter.text(
            legend_rect.left_top() + egui::vec2(10., 8.),
            egui::Align2::LEFT_TOP,
            self.layer.label(),
            egui::FontId::proportional(11.),
            egui::Color32::WHITE,
        );
        for (i, (c, l)) in [
            (egui::Color32::from_rgb(13, 65, 91), "LOW"),
            (egui::Color32::from_rgb(58, 145, 82), "MID"),
            (egui::Color32::from_rgb(145, 47, 36), "HIGH"),
        ]
        .iter()
        .enumerate()
        {
            let y = legend_rect.top() + 32. + i as f32 * 19.;
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(legend_rect.left() + 10., y),
                    egui::vec2(12., 10.),
                ),
                1.,
                *c,
            );
            painter.text(
                egui::pos2(legend_rect.left() + 30., y),
                egui::Align2::LEFT_TOP,
                *l,
                egui::FontId::proportional(9.),
                MUTED,
            );
        }
        let compass = egui::pos2(rect.right() - 30., rect.top() + 26.);
        painter.circle_stroke(
            compass,
            15.,
            egui::Stroke::new(1_f32, egui::Color32::from_white_alpha(140)),
        );
        painter.text(
            compass,
            egui::Align2::CENTER_CENTER,
            "N",
            egui::FontId::proportional(13.),
            egui::Color32::WHITE,
        );
    }
}

impl eframe::App for StarApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(8)))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("S.T.A.R.").size(22.0).strong());
                    ui.separator();
                    ui.selectable_value(&mut self.page, Page::Overview, "Overview");
                    ui.selectable_value(&mut self.page, Page::Terrain, "Terrain Analysis");
                    ui.selectable_value(&mut self.page, Page::Discovery, "Analog Discovery");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Load GeoTIFF").clicked() { self.load_file(); }
                        if ui.button("Generate demo").clicked() { self.generate_demo(); }
                        if ui.add_enabled(self.dem.is_some(), egui::Button::new("Run analysis")).clicked() { self.run_analysis(); }
                    });
                });
                ui.separator();

                let panel_height = ui.available_height();
                let map_width = (ui.available_width() - 591.0).max(180.0);
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(egui::vec2(315.0, panel_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        egui::Frame::new().fill(PANEL).stroke(egui::Stroke::new(1_f32, LINE)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                            Self::panel_frame(ui, |ui| {
                                ui.label(egui::RichText::new("PROJECT INFORMATION").strong().color(GREEN));
                                if let Some(dem) = &self.dem {
                                    let (lo, hi) = dem.grid.min_max().unwrap_or((0.0, 0.0));
                                    ui.label(format!("Elevation: {lo:.1} to {hi:.1} m"));
                                    ui.label(format!("{} x {} | {}", dem.grid.width, dem.grid.height, dem.crs_label));
                                    ui.label(format!("Resolution: {:.1} m", dem.pixel_size_m));
                                } else { ui.label(egui::RichText::new("No terrain dataset loaded").color(MUTED)); }
                            });
                            ui.add_space(7.0);
                            Self::panel_frame(ui, |ui| {
                                ui.label(egui::RichText::new("INFORMATION LAYERS").strong().color(GREEN));
                                for layer in Layer::all() {
                                    ui.selectable_value(&mut self.layer, layer, layer.label());
                                }
                                ui.horizontal(|ui| {
                                    if ui.button("Load DEM").clicked() { self.load_file(); }
                                    if ui.button("Demo").clicked() { self.generate_demo(); }
                                });
                                if ui.button("Run terrain analysis").clicked() { self.run_analysis(); }
                                ui.add(egui::Slider::new(&mut self.radius, 1..=8).text("Radius (px)"));
                            });
                            ui.add_space(7.0);
                            egui::CollapsingHeader::new("Elevation profile").default_open(false).show(ui, |ui| {
                                if let Some(dem) = &self.dem { Self::profile_chart(ui, &dem.grid); }
                                else { ui.label("Load a DEM to calculate profile"); }
                            });
                            if self.page == Page::Discovery {
                                self.export_panel(ui);
                            } else {
                                egui::CollapsingHeader::new("Fingerprint export").default_open(false).show(ui, |ui| self.export_panel(ui));
                            }
                        });
                    });

                    ui.allocate_ui_with_layout(egui::vec2(map_width, panel_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                            match self.page {
                                Page::Terrain => self.map_view(ui, ctx),
                                Page::Overview => {
                                    ui.heading("Mission Overview");
                                    ui.label("Spatio-Temporal & Physical Analog Discovery");
                                    ui.add_space(15.0);
                                    ui.label("Load a projected Earth DEM, derive terrain fingerprints, and compare physical surface characteristics with lunar and Martian target regions.");
                                    ui.add_space(12.0);
                                    if ui.button("Open Terrain Workspace").clicked() { self.page = Page::Terrain; }
                                    if self.dem.is_none() && ui.button("Generate demo terrain").clicked() { self.generate_demo(); self.page = Page::Terrain; }
                                }
                                Page::Discovery => {
                                    ui.heading("Analog Discovery");
                                    ui.label("Extract terrain fingerprints here, then export from the left panel.");
                                    ui.add_space(8.0);
                                    if let Some(report) = &self.report {
                                        ui.label(format!("{} valid fingerprints | {:.1}% coverage", report.processed_cells(), report.coverage_fraction() * 100.0));
                                    } else if self.dem.is_none() { ui.label("Load a GeoTIFF or generate demo terrain to begin."); }
                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new("Planetary analog ranking needs reference datasets and is not included in this build.").small().color(MUTED));
                                }
                            }
                        });
                    });

                    ui.allocate_ui_with_layout(egui::vec2(260.0, panel_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        egui::Frame::new().fill(PANEL).stroke(egui::Stroke::new(1_f32, LINE)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                            Self::panel_frame(ui, |ui| {
                                ui.label(egui::RichText::new("POINT INSPECTOR").strong().color(GREEN));
                                if let (Some(dem), Some((x, y))) = (&self.dem, self.selected) {
                                    ui.heading(format!("{x} , {y}"));
                                    if let Some(z) = dem.grid.get(x, y) { Self::detail(ui, "Elevation", format!("{z:.2} m")); }
                                    if let Some((slope, _, roughness, curvature)) = Self::sample_metric(&dem.grid, x, y) {
                                        Self::detail(ui, "Slope", format!("{slope:.2} deg"));
                                        Self::detail(ui, "Roughness", format!("{roughness:.2} m"));
                                        Self::detail(ui, "Curvature", format!("{curvature:.6}"));
                                    }
                                } else { ui.label("Click the map to inspect a raster cell."); }
                            });
                            ui.add_space(7.0);
                            Self::panel_frame(ui, |ui| {
                                ui.label(egui::RichText::new("MAP CONTROLS").strong().color(GREEN));
                                ui.horizontal(|ui| {
                                    if ui.button("Zoom +").clicked() { self.zoom = (self.zoom * 1.25).clamp(0.35, 8.0); }
                                    if ui.button("Zoom -").clicked() { self.zoom = (self.zoom / 1.25).clamp(0.35, 8.0); }
                                    if ui.button("Reset").clicked() { self.zoom = 1.0; self.pan = egui::Vec2::ZERO; }
                                });
                                ui.checkbox(&mut self.show_grid, "Show grid");
                                ui.label(egui::RichText::new("Drag to pan; scroll to zoom; click to inspect.").size(10.0).color(MUTED));
                            });
                            ui.add_space(7.0);
                            Self::panel_frame(ui, |ui| {
                                ui.label(egui::RichText::new("ANALYSIS STATUS").strong().color(GREEN));
                                if let Some(report) = &self.report { ui.label(format!("{} fingerprints | {:.1}% coverage", report.processed_cells(), report.coverage_fraction() * 100.0)); }
                                else { ui.label("Run terrain analysis to see results."); }
                                if let Some(error) = &self.error { ui.colored_label(egui::Color32::LIGHT_RED, error); }
                                if let Some(status) = &self.export_status { ui.label(status); }
                            });
                        });
                    });
                });
            });
    }
}
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("S.T.A.R. | Terrain Workbench")
            .with_inner_size([1600., 1000.])
            .with_min_inner_size([1100., 720.]),
        ..Default::default()
    };
    eframe::run_native(
        "S.T.A.R.",
        options,
        Box::new(|cc| {
            let mut v = egui::Visuals::dark();
            v.panel_fill = BG;
            v.window_fill = PANEL;
            v.extreme_bg_color = BG;
            v.faint_bg_color = PANEL_2;
            v.selection.bg_fill = egui::Color32::from_rgb(57, 70, 52);
            v.selection.stroke = egui::Stroke::new(1_f32, GREEN);
            v.widgets.noninteractive.bg_fill = PANEL;
            v.widgets.inactive.bg_fill = PANEL_2;
            v.widgets.hovered.bg_fill = egui::Color32::from_rgb(51, 59, 49);
            v.widgets.active.bg_fill = egui::Color32::from_rgb(67, 81, 58);
            cc.egui_ctx.set_visuals(v);
            let mut s = (*cc.egui_ctx.style()).clone();
            s.spacing.item_spacing = egui::vec2(8., 7.);
            s.spacing.button_padding = egui::vec2(10., 7.);
            cc.egui_ctx.set_style(s);
            Ok(Box::new(StarApp::default()))
        }),
    )
}
