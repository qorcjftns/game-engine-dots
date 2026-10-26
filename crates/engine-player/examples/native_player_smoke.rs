//! production Player와 같은 표시 경로를 쓰는 native CI host.
#![allow(dead_code)]
#[path = "../src/main.rs"]
mod player;
use eframe::{App, egui};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
struct Smoke {
    player: player::Player,
    frames: u32,
    output: std::path::PathBuf,
    captured: Arc<AtomicBool>,
}
impl App for Smoke {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.player.logic(ctx, frame);
        if self.captured.load(Ordering::Acquire) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.player.ui(ui, frame);
        self.frames += 1;
        if self.frames == 10 {
            let path = self.output.join("player-frame.png");
            let captured = self.captured.clone();
            ui.ctx().request_screenshot(move |pixels| {
                let bytes: Vec<_> = pixels.pixels.iter().flat_map(|p| p.to_array()).collect();
                image::save_buffer_with_format(
                    path,
                    &bytes,
                    pixels.size[0] as u32,
                    pixels.size[1] as u32,
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .unwrap();
                captured.store(true, Ordering::Release);
            });
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let s = engine_document::EditorSession::open(&args[0])?;
    let run = engine_runtime::PlaySession::new(engine_runtime::RuntimeWorld::snapshot(&s)?, 1);
    let output = args[1].clone().into();
    eframe::run_native(
        "Engine-1 Native Player Smoke",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default().with_inner_size([960.0, 540.0]),
            ..Default::default()
        },
        Box::new(move |_| {
            Ok(Box::new(Smoke {
                player: player::Player {
                    run,
                    view: Default::default(),
                    time: std::time::Instant::now(),
                },
                frames: 0,
                output,
                captured: Arc::new(AtomicBool::new(false)),
            }))
        }),
    )?;
    Ok(())
}
