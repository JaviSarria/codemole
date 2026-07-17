#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::{egui, App};
use rfd::FileDialog;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};

const LANGUAGES: [&str; 3] = ["java", "python", "go"];
const TEMPLATES: [&str; 7] = [
    "Seleccionar plantilla...",
    "Spring - Endpoint",
    "Spring - Funcion",
    "FastAPI - Endpoint",
    "FastAPI - Funcion",
    "Gin - Endpoint",
    "Gin - Funcion",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunMode {
    Endpoint,
    Funcion,
}

#[derive(Debug)]
enum RunEvent {
    Started(String),
    Stdout(String),
    Stderr(String),
    Finished(i32),
}

#[derive(Default)]
struct ValidationResult {
    errors: Vec<String>,
}

impl ValidationResult {
    fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

struct CodemoleDesktopApp {
    selected_template_idx: usize,
    lang_idx: usize,
    mode: RunMode,
    endpoint: String,
    funcion: String,
    clase: String,
    paquete: String,
    modulo: String,
    path: String,
    output: String,
    db: String,
    java_parser: String,
    running: bool,
    logs: String,
    command_preview: String,
    info_dialog: Option<InfoDialog>,
    rx: Option<Receiver<RunEvent>>,
}

struct InfoDialog {
    title: String,
    message: String,
}

impl Default for CodemoleDesktopApp {
    fn default() -> Self {
        Self {
            selected_template_idx: 0,
            lang_idx: 0,
            mode: RunMode::Endpoint,
            endpoint: String::new(),
            funcion: String::new(),
            clase: String::new(),
            paquete: String::new(),
            modulo: String::new(),
            path: String::new(),
            output: String::new(),
            db: String::new(),
            java_parser: String::new(),
            running: false,
            logs: String::new(),
            command_preview: String::new(),
            info_dialog: None,
            rx: None,
        }
    }
}

impl CodemoleDesktopApp {
    fn language(&self) -> &'static str {
        LANGUAGES[self.lang_idx]
    }

    fn validate(&self) -> ValidationResult {
        let mut errors = Vec::new();

        if self.path.trim().is_empty() {
            errors.push("El campo 'Source path' es obligatorio.".to_string());
        }

        match self.mode {
            RunMode::Endpoint => {
                if self.endpoint.trim().is_empty() {
                    errors.push("El campo 'Endpoint' es obligatorio en modo endpoint.".to_string());
                }
            }
            RunMode::Funcion => {
                if self.funcion.trim().is_empty() {
                    errors.push("El campo 'Funcion' es obligatorio en modo funcion.".to_string());
                }

                match self.language() {
                    "java" => {
                        if self.clase.trim().is_empty() {
                            errors.push("Para Java en modo funcion, '--clase' es obligatorio.".to_string());
                        }
                        if self.paquete.trim().is_empty() {
                            errors.push("Para Java en modo funcion, '--paquete' es obligatorio.".to_string());
                        }
                    }
                    "python" => {
                        if self.clase.trim().is_empty() {
                            errors.push("Para Python en modo funcion, '--clase' es obligatorio.".to_string());
                        }
                        if self.modulo.trim().is_empty() {
                            errors.push("Para Python en modo funcion, '--modulo' es obligatorio.".to_string());
                        }
                    }
                    "go" => {
                        if self.modulo.trim().is_empty() {
                            errors.push("Para Go en modo funcion, '--modulo' es obligatorio.".to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        ValidationResult { errors }
    }

    fn apply_template(&mut self) {
        match self.selected_template_idx {
            1 => {
                // Spring endpoint
                self.lang_idx = 0;
                self.mode = RunMode::Endpoint;
                self.endpoint = "/api/users".to_string();
                self.funcion.clear();
                self.clase.clear();
                self.paquete.clear();
                self.modulo.clear();
            }
            2 => {
                // Spring function
                self.lang_idx = 0;
                self.mode = RunMode::Funcion;
                self.endpoint.clear();
                self.funcion = "getUserById".to_string();
                self.clase = "UserController".to_string();
                self.paquete = "com.example.users".to_string();
                self.modulo.clear();
            }
            3 => {
                // FastAPI endpoint
                self.lang_idx = 1;
                self.mode = RunMode::Endpoint;
                self.endpoint = "/items/{id}".to_string();
                self.funcion.clear();
                self.clase.clear();
                self.paquete.clear();
                self.modulo.clear();
            }
            4 => {
                // FastAPI function
                self.lang_idx = 1;
                self.mode = RunMode::Funcion;
                self.endpoint.clear();
                self.funcion = "get_item".to_string();
                self.clase = "ItemService".to_string();
                self.paquete.clear();
                self.modulo = "services.items".to_string();
            }
            5 => {
                // Gin endpoint
                self.lang_idx = 2;
                self.mode = RunMode::Endpoint;
                self.endpoint = "/health".to_string();
                self.funcion.clear();
                self.clase.clear();
                self.paquete.clear();
                self.modulo.clear();
            }
            6 => {
                // Gin function
                self.lang_idx = 2;
                self.mode = RunMode::Funcion;
                self.endpoint.clear();
                self.funcion = "GetHealth".to_string();
                self.clase.clear();
                self.paquete.clear();
                self.modulo = "handlers".to_string();
            }
            _ => {}
        }
    }

    fn build_args(&self) -> Vec<String> {
        let mut args = vec!["--lang".to_string(), self.language().to_string()];

        match self.mode {
            RunMode::Endpoint => {
                args.push("--endpoint".to_string());
                args.push(self.endpoint.trim().to_string());
            }
            RunMode::Funcion => {
                args.push("--funcion".to_string());
                args.push(self.funcion.trim().to_string());

                if !self.clase.trim().is_empty() {
                    args.push("--clase".to_string());
                    args.push(self.clase.trim().to_string());
                }
                if !self.paquete.trim().is_empty() {
                    args.push("--paquete".to_string());
                    args.push(self.paquete.trim().to_string());
                }
                if !self.modulo.trim().is_empty() {
                    args.push("--modulo".to_string());
                    args.push(self.modulo.trim().to_string());
                }
            }
        }

        args.push("--path".to_string());
        args.push(self.path.trim().to_string());

        if !self.output.trim().is_empty() {
            args.push("--output".to_string());
            args.push(self.output.trim().to_string());
        }

        if !self.db.trim().is_empty() {
            args.push("--db".to_string());
            args.push(self.db.trim().to_string());
        }

        if !self.java_parser.trim().is_empty() {
            args.push("--java-parser".to_string());
            args.push(self.java_parser.trim().to_string());
        }

        args
    }

    fn start_run(&mut self) {
        let validation = self.validate();
        if !validation.ok() {
            self.logs = validation.errors.join("\n");
            return;
        }

        let args = self.build_args();
        let (tx, rx) = mpsc::channel::<RunEvent>();
        self.rx = Some(rx);
        self.running = true;
        self.logs.clear();

        let runner = detect_runner();
        self.command_preview = preview_command(&runner, &args);

        std::thread::spawn(move || run_process(runner, args, tx));
    }

    fn consume_events(&mut self, ctx: &egui::Context) {
        let mut finished = false;
        if let Some(rx) = &self.rx {
            while let Ok(evt) = rx.try_recv() {
                match evt {
                    RunEvent::Started(line) => {
                        self.logs.push_str(&line);
                        self.logs.push('\n');
                    }
                    RunEvent::Stdout(line) => {
                        self.logs.push_str(&line);
                        self.logs.push('\n');
                    }
                    RunEvent::Stderr(line) => {
                        self.logs.push_str("[stderr] ");
                        self.logs.push_str(&line);
                        self.logs.push('\n');
                    }
                    RunEvent::Finished(code) => {
                        self.logs.push_str(&format!("\nProceso finalizado con codigo: {}\n", code));
                        finished = true;
                    }
                }
            }
        }

        if finished {
            self.running = false;
            self.rx = None;
        }

        if self.running {
            ctx.request_repaint();
        }
    }

    fn param_help(param: &str) -> &'static str {
        match param {
            "--lang" => "Language / framework: java | python | go",
            "--endpoint" => "HTTP endpoint to trace, e.g. /api/users or /items/{id}. Cannot be combined with --funcion.",
            "--funcion" => "Function or method name to trace. Cannot be combined with --endpoint.",
            "--clase" => "Class name. Required with --funcion for Java and Python.",
            "--paquete" => "Package name. Required with --funcion for Java.",
            "--modulo" => "Module name. Required with --funcion for Python and Go.",
            "--path" => "Root directory of the source code to analyse. Default: current directory.",
            "--output" => "Base output directory. A sub-folder named after endpoint or function is created inside.",
            "--db" => "Path to the skip-symbols SQLite database. Created and seeded on first run.",
            "--java-parser" => "Path to java-parser.jar for CFG-enriched Java sequence diagrams.",
            _ => "No help available for this parameter.",
        }
    }

    fn label_with_info(ui: &mut egui::Ui, label: &str, help: &str, info_dialog: &mut Option<InfoDialog>) {
        ui.horizontal(|ui| {
            ui.label(label);
            if ui
                .add_sized([20.0, 20.0], egui::Button::new("i"))
                .on_hover_text("Mostrar ayuda de este parametro")
                .clicked()
            {
                *info_dialog = Some(InfoDialog {
                    title: format!("Ayuda {}", label),
                    message: help.to_string(),
                });
            }
        });
    }

    fn path_row(
        ui: &mut egui::Ui,
        label: &str,
        help: &str,
        value: &mut String,
        is_file: bool,
        info_dialog: &mut Option<InfoDialog>,
    ) {
        Self::label_with_info(ui, label, help, info_dialog);
        ui.horizontal(|ui| {
            let button_width = 140.0;
            let edit_width = (ui.available_width() - button_width - 8.0).max(220.0);
            ui.add_sized([edit_width, 24.0], egui::TextEdit::singleline(value));
            let button_label = if is_file { "Elegir archivo" } else { "Elegir carpeta" };
            if ui.add_sized([button_width, 24.0], egui::Button::new(button_label)).clicked() {
                if is_file {
                    if let Some(path) = FileDialog::new().pick_file() {
                        *value = path.display().to_string();
                    }
                } else if let Some(path) = FileDialog::new().pick_folder() {
                    *value = path.display().to_string();
                }
            }
        });
    }

    fn show_info_dialog(&mut self, ctx: &egui::Context) {
        let mut close_dialog = false;
        if let Some(dialog) = self.info_dialog.as_ref() {
            egui::Window::new(dialog.title.clone())
                .collapsible(false)
                .resizable(true)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(&dialog.message);
                    ui.add_space(8.0);
                    if ui.button("Cerrar").clicked() {
                        close_dialog = true;
                    }
                });
        }

        if close_dialog {
            self.info_dialog = None;
        }
    }
}

impl App for CodemoleDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.consume_events(ctx);

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.heading("codemole desktop");
            ui.label("Frontend de escritorio para ejecutar codemole por endpoint o por funcion");
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 10.0);

            ui.group(|ui| {
                ui.label("Plantillas");
                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("Preset")
                        .selected_text(TEMPLATES[self.selected_template_idx])
                        .show_ui(ui, |ui| {
                            for (idx, name) in TEMPLATES.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_template_idx, idx, *name);
                            }
                        });

                    if ui
                        .add_enabled(self.selected_template_idx != 0, egui::Button::new("Aplicar plantilla"))
                        .clicked()
                    {
                        self.apply_template();
                    }
                });

                ui.label("Las plantillas rellenan lenguaje, modo y parametros principales con ejemplos editables.");
            });

            ui.separator();

            ui.group(|ui| {
                ui.label("Configuracion base");
                Self::label_with_info(ui, "Lenguaje", Self::param_help("--lang"), &mut self.info_dialog);
                egui::ComboBox::from_id_salt("lang_select")
                    .selected_text(self.language())
                    .show_ui(ui, |ui| {
                        for (idx, lang) in LANGUAGES.iter().enumerate() {
                            ui.selectable_value(&mut self.lang_idx, idx, *lang);
                        }
                    });

                ui.horizontal(|ui| {
                    ui.label("Modo");
                    if ui
                        .add_sized([20.0, 20.0], egui::Button::new("i"))
                        .on_hover_text("Mostrar ayuda de endpoint y funcion")
                        .clicked()
                    {
                        self.info_dialog = Some(InfoDialog {
                            title: "Ayuda Modo".to_string(),
                            message: format!(
                                "Endpoint: {}\n\nFuncion: {}",
                                Self::param_help("--endpoint"),
                                Self::param_help("--funcion")
                            ),
                        });
                    }
                    ui.radio_value(&mut self.mode, RunMode::Endpoint, "Endpoint");
                    ui.radio_value(&mut self.mode, RunMode::Funcion, "Funcion");
                });

                match self.mode {
                    RunMode::Endpoint => {
                        Self::label_with_info(ui, "Endpoint", Self::param_help("--endpoint"), &mut self.info_dialog);
                        ui.add(egui::TextEdit::singleline(&mut self.endpoint).desired_width(f32::INFINITY));
                    }
                    RunMode::Funcion => {
                        Self::label_with_info(ui, "Funcion", Self::param_help("--funcion"), &mut self.info_dialog);
                        ui.add(egui::TextEdit::singleline(&mut self.funcion).desired_width(f32::INFINITY));

                        if self.language() != "go" {
                            Self::label_with_info(ui, "Clase", Self::param_help("--clase"), &mut self.info_dialog);
                            ui.add(egui::TextEdit::singleline(&mut self.clase).desired_width(f32::INFINITY));
                        }

                        if self.language() == "java" {
                            Self::label_with_info(ui, "Paquete", Self::param_help("--paquete"), &mut self.info_dialog);
                            ui.add(egui::TextEdit::singleline(&mut self.paquete).desired_width(f32::INFINITY));
                        }

                        if self.language() == "python" || self.language() == "go" {
                            Self::label_with_info(ui, "Modulo", Self::param_help("--modulo"), &mut self.info_dialog);
                            ui.add(egui::TextEdit::singleline(&mut self.modulo).desired_width(f32::INFINITY));
                        }
                    }
                }
            });

            ui.separator();

            ui.group(|ui| {
                ui.label("Rutas y opciones opcionales");
                Self::path_row(ui, "Source path", Self::param_help("--path"), &mut self.path, false, &mut self.info_dialog);
                Self::path_row(ui, "Output", Self::param_help("--output"), &mut self.output, false, &mut self.info_dialog);
                Self::path_row(ui, "DB", Self::param_help("--db"), &mut self.db, true, &mut self.info_dialog);
                Self::path_row(
                    ui,
                    "Java parser JAR",
                    Self::param_help("--java-parser"),
                    &mut self.java_parser,
                    true,
                    &mut self.info_dialog,
                );
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.running, egui::Button::new("Ejecutar codemole"))
                    .clicked()
                {
                    self.start_run();
                }

                if !self.output.trim().is_empty() && ui.button("Abrir output").clicked() {
                    open_in_file_explorer(&self.output);
                }
            });

            if !self.command_preview.trim().is_empty() {
                ui.label("Comando ejecutado");
                ui.code(self.command_preview.clone());
            }

            ui.separator();
            ui.label("Salida");
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.logs)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(20)
                            .desired_width(f32::INFINITY),
                    );
                });
        });

            self.show_info_dialog(ctx);
    }
}

#[derive(Debug)]
enum Runner {
    Binary(PathBuf),
    Cargo,
}

fn detect_runner() -> Runner {
    let current = std::env::current_exe().ok();
    if let Some(current) = current {
        if let Some(dir) = current.parent() {
            let candidate = if cfg!(windows) {
                dir.join("codemole.exe")
            } else {
                dir.join("codemole")
            };

            if candidate.exists() {
                return Runner::Binary(candidate);
            }
        }
    }

    Runner::Cargo
}

fn preview_command(runner: &Runner, args: &[String]) -> String {
    match runner {
        Runner::Binary(bin) => {
            let mut full = vec![bin.display().to_string()];
            full.extend(args.iter().cloned());
            full.join(" ")
        }
        Runner::Cargo => {
            let mut full = vec![
                "cargo".to_string(),
                "run".to_string(),
                "--bin".to_string(),
                "codemole".to_string(),
                "--".to_string(),
            ];
            full.extend(args.iter().cloned());
            full.join(" ")
        }
    }
}

fn run_process(runner: Runner, args: Vec<String>, tx: Sender<RunEvent>) {
    let mut cmd = match runner {
        Runner::Binary(bin) => {
            let mut c = Command::new(bin);
            c.args(&args);
            c
        }
        Runner::Cargo => {
            let mut c = Command::new("cargo");
            c.arg("run")
                .arg("--bin")
                .arg("codemole")
                .arg("--")
                .args(&args)
                .current_dir(env!("CARGO_MANIFEST_DIR"));
            c
        }
    };

    let _ = tx.send(RunEvent::Started("Iniciando proceso...".to_string()));

    let mut child = match cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(RunEvent::Stderr(format!("No se pudo iniciar el proceso: {}", e)));
            let _ = tx.send(RunEvent::Finished(-1));
            return;
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let tx_out = tx.clone();
    let out_handle = std::thread::spawn(move || {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let _ = tx_out.send(RunEvent::Stdout(line));
                }
            }
        }
    });

    let tx_err = tx.clone();
    let err_handle = std::thread::spawn(move || {
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let _ = tx_err.send(RunEvent::Stderr(line));
                }
            }
        }
    });

    let code = match child.wait() {
        Ok(status) => status.code().unwrap_or(-1),
        Err(_) => -1,
    };

    let _ = out_handle.join();
    let _ = err_handle.join();

    let _ = tx.send(RunEvent::Finished(code));
}

fn open_in_file_explorer(path: &str) {
    let target = Path::new(path);
    if !target.exists() {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("explorer").arg(path).spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(path).spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(path).spawn();
    }
}

pub fn run_desktop() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 820.0])
            .with_min_inner_size([900.0, 680.0])
            .with_title("codemole desktop"),
        ..Default::default()
    };

    eframe::run_native(
        "codemole desktop",
        options,
        Box::new(|_cc| Ok(Box::<CodemoleDesktopApp>::default())),
    )
}

fn main() -> eframe::Result<()> {
    run_desktop()
}
