#![warn(clippy::pedantic)]
#![warn(clippy::implicit_return)]
#![allow(clippy::needless_return)]

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use backtrace::Backtrace;
use eframe::{icon_data, NativeOptions};
use egui::{CentralPanel, Context, ScrollArea, TopBottomPanel, vec2, ViewportBuilder, Window};
use rfd::FileDialog;
use crate::internals::{execute, get_executable_paths};

mod internals;

struct MyApp {
    executables: HashSet<String>,
    selected_executable: Option<String>,
    modified_executables: HashSet<String>,
    wrapper_dir: PathBuf,
    config_path: PathBuf,
    show_picker_dialog: bool,
}

impl MyApp {
    fn new() -> Self {
        // Create our data folder
        let xdg_dirs = xdg::BaseDirectories::with_prefix("NvidiaManager").unwrap();
        let wrapper_dir = xdg_dirs.create_data_directory("ONLY_DELETE_IF_YOU_KNOW_WHAT_YOU_ARE_DOING").unwrap();
        
        // Create our config folder
        let config_dir = xdg_dirs.create_data_directory("config").unwrap();
        let config_path = config_dir.join("config.txt");
        if !config_path.exists() {
            let mut file = fs::File::create(&config_path).unwrap();
            file.write_all(b"").unwrap();
        }
        
        // Read the config file, split on newlines, and remove empty lines
        validate_config(&config_path);
        let config = read_config(&config_path);

        // Fetch the initial list of processes with executables
        let executables = get_executable_paths().unwrap_or_default();

        return MyApp {
            executables,
            selected_executable: None,
            modified_executables: config,
            wrapper_dir,
            config_path,
            show_picker_dialog: false
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // The top panel
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::ComboBox::from_label("Processes (WARNING: Be careful with what processes you choose!)")
                .selected_text(self.selected_executable.as_deref().unwrap_or("Select a process"))
                .show_ui(ui, |ui| {
                    for process in &self.executables {
                        ui.selectable_value(&mut self.selected_executable, Some(process.clone()), process);
                    }
                });

            ui.horizontal(|ui| {
                if ui.button("Add/Remove").clicked() {
                    if let Some(selected) = &self.selected_executable { // If an item is selected, and the button is pressed
                        match execute(&self.wrapper_dir, &PathBuf::from(selected)) { // Execute the main logic
                            Ok(reverted) => {
                                if reverted { // If we reverted our changes then remove it from the list and config file, otherwise add it
                                    self.modified_executables.remove(selected);
                                    remove_config(selected, &self.config_path);
                                    return;
                                }
                                self.modified_executables.insert(selected.to_string());
                                add_config(selected, &self.config_path);
                            },
                            Err(e) => { // If there's an error, backtrace and print it
                                let backtrace = Backtrace::new();
                                eprintln!("Failed to execute the wrapper script for {selected}: {e}\nBacktrace:\n{backtrace:?}");
                                return;
                            }
                        }
                    }
                }

                if ui.button("File Picker").clicked() {
                    self.show_picker_dialog = true;
                }
            });
        });

        // Show the list of added processes
        CentralPanel::default().show(ctx, |ui| {
            ui.heading("Added Processes That Use NVIDIA GPU");
            ScrollArea::vertical().show(ui, |ui| {
                for item in &self.modified_executables {
                    ui.selectable_value(&mut self.selected_executable, Some(item.clone()), item);
                }
            });
        });
        
        
        // File Picker
        if self.show_picker_dialog {
            Window::new("Pick File or Folder")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Pick a File or Folder");
                    ui.horizontal(|ui| {
                        if ui.button("Pick File").clicked() {
                            if let Some(picked_path) = FileDialog::new().pick_file() {
                                let _ = self.selected_executable.insert(picked_path.display().to_string());
                            }
                            self.show_picker_dialog = false;
                        }
                        if ui.button("Pick Folder").clicked() {
                            if let Some(picked_path) = FileDialog::new().pick_folder() {
                                let _ = self.selected_executable.insert(picked_path.display().to_string());
                            }
                            self.show_picker_dialog = false;
                        }
                    });
                    if ui.button("Cancel").clicked() {
                        self.show_picker_dialog = false;
                    }
                });
        }
    }
}

fn read_config(config_path: &PathBuf) -> HashSet<String> {
    return fs::read_to_string(config_path)
        .unwrap_or_default()
        .lines()
        .filter(|line| return !line.is_empty())
        .map(ToString::to_string)
        .collect();
}

fn add_config(text: &str, config_path: &PathBuf) {
    let mut config = read_config(config_path);

    if !config.insert(text.to_string()) {
        eprintln!("{text} is already in the config file");
    }

    let modified_executable: Vec<String> = config.into_iter().collect();
    fs::write(config_path, modified_executable.join("\n")).expect("Failed to write to config file");
}

fn remove_config(text: &str, config_path: &PathBuf) {
    let mut config = read_config(config_path);
    
    if !config.remove(text) {
        eprintln!("{text} is not in the config file");
    }

    let config: Vec<String> = config.into_iter().collect();
    fs::write(config_path, config.join("\n")).expect("Failed to write to config file");
}

fn validate_config(config_path: &PathBuf) {
    let config = read_config(config_path);
    for item in config {
        let path = PathBuf::from(&item);
        if path.is_dir() { continue; }
        if !path.with_extension("bak").exists() {
            remove_config(&item, config_path);
        }
    }
}

fn main() {
    // TODO: Check if we need sudo perms or something
    eframe::run_native(
        "Nvidia Manager",
        NativeOptions {
            viewport: ViewportBuilder::default()
                .with_inner_size(vec2(800.0, 600.0))
                .with_icon(icon_data::from_png_bytes(&include_bytes!("../icons/nvidia_manager.png")[..]).unwrap()),
            ..Default::default()
        },
        Box::new(|_cc| return Ok(Box::new(MyApp::new()))),
    ).expect("Error running the app");
}
