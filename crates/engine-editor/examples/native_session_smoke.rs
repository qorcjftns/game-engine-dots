//! 실제 WindowServer/desktop과 renderer를 사용하는 CI 전용 host입니다.
//! production 바이너리에 테스트 옵션을 넣지 않고 같은 EditorApp 모듈을 재사용합니다.
#![allow(dead_code)]

#[path = "../src/agent_control.rs"]
mod agent_control;
#[path = "../src/agent_editing.rs"]
mod agent_editing;
#[path = "../src/app.rs"]
mod app;
#[path = "../src/canvas.rs"]
mod canvas;
#[path = "../src/canvas_input.rs"]
mod canvas_input;
#[path = "../src/editing.rs"]
mod editing;
#[path = "../src/editor_actions.rs"]
mod editor_actions;
#[path = "../src/inspector.rs"]
mod inspector;
#[path = "../src/new_project.rs"]
mod new_project;
#[path = "../src/platform.rs"]
mod platform;
#[path = "../src/project_workflow.rs"]
mod project_workflow;
#[path = "../src/raster.rs"]
mod raster;
#[path = "../src/resources.rs"]
mod resources;
#[path = "../src/startup.rs"]
mod startup;

#[cfg(test)]
#[path = "../src/editing_tests.rs"]
mod editing_tests;

use editor_actions::EditorAction;
use eframe::{App, egui};
use engine_document::EditorSession;
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

struct NativeSmoke {
    editor: app::EditorApp,
    directory: PathBuf,
    frames: u64,
    last_command: u64,
}

impl NativeSmoke {
    fn observe(&mut self, id: u64) {
        let snapshot = self.editor.publish_live(false).expect("live snapshot");
        let value = serde_json::json!({"id": id, "frames": self.frames, "snapshot": snapshot});
        fs::write(
            self.directory.join("host-observation.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }

    fn commands(&mut self, ctx: &egui::Context) {
        let Ok(bytes) = fs::read(self.directory.join("host-command.json")) else {
            return;
        };
        let Ok(command) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return;
        };
        let id = command["id"].as_u64().expect("command id");
        if id <= self.last_command {
            return;
        }
        match command["action"].as_str().expect("command action") {
            "observe" => {}
            "undo" => self.editor.process_action(ctx, EditorAction::Undo),
            "redo" => self.editor.process_action(ctx, EditorAction::Redo),
            "save_reopen" => {
                self.editor.process_action(ctx, EditorAction::Save);
                assert!(self.editor.workspace.error.is_none());
                let path = self
                    .editor
                    .workspace
                    .session
                    .as_ref()
                    .unwrap()
                    .project_file()
                    .to_owned();
                let reopened = EditorSession::open(path).unwrap();
                self.editor.workspace.textures.reload(ctx, &reopened);
                self.editor.workspace.session = Some(reopened);
                self.editor.workspace.generation += 1;
                self.editor.synchronize_selection();
            }
            "capture" => {
                let path = self.directory.join(format!("frame-{id}.png"));
                ctx.request_screenshot(move |pixels| {
                    let bytes: Vec<_> = pixels.pixels.iter().flat_map(|p| p.to_array()).collect();
                    image::save_buffer(
                        path,
                        &bytes,
                        pixels.size[0] as u32,
                        pixels.size[1] as u32,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                });
            }
            "close" => self.editor.process_action(ctx, EditorAction::Quit),
            _ => panic!("Unknown test-only host command"),
        }
        self.last_command = id;
        self.observe(id);
    }
}

impl App for NativeSmoke {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.editor.logic(ctx, frame);
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.editor.ui(ui, frame);
        self.frames += 1;
        let ctx = ui.ctx().clone();
        ctx.request_repaint_after(Duration::from_millis(20));
        if self.frames == 3 {
            self.observe(0);
        }
        self.commands(&ctx);
    }
}

fn main() -> eframe::Result {
    let directory = PathBuf::from(std::env::args_os().nth(1).expect("QA directory"));
    let finished = Arc::new(AtomicBool::new(false));
    let watchdog = finished.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        if !watchdog.load(Ordering::Acquire) {
            eprintln!(
                "Native GUI smoke exceeded 30 seconds; graphical session/renderer or workflow failed."
            );
            std::process::exit(124);
        }
    });
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Engine-1 Native Session Smoke")
            .with_inner_size([1280.0, 800.0]),
        renderer: eframe::Renderer::Wgpu,
        event_loop_builder: platform::event_loop_hook(),
        system_font_fallback: true,
        ..Default::default()
    };
    let result = eframe::run_native(
        "Engine-1 Native Session Smoke",
        options,
        Box::new(move |creation| {
            let mut editor = app::EditorApp::new(creation);
            let session = EditorSession::open(directory.join("project/project.json")).unwrap();
            editor
                .workspace
                .textures
                .reload(&creation.egui_ctx, &session);
            editor.workspace.session = Some(session);
            editor.workspace.generation = 1;
            editor.start_agent_control(
                &creation.egui_ctx,
                startup::AgentControlOptions {
                    connection_file: Some(directory.join("connection.json")),
                },
            );
            assert!(
                editor.live_control.is_some(),
                "native control initialization failed"
            );
            Ok(Box::new(NativeSmoke {
                editor,
                directory,
                frames: 0,
                last_command: 0,
            }))
        }),
    );
    finished.store(true, Ordering::Release);
    result
}
