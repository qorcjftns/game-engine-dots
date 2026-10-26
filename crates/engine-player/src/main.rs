//! 편집 UI와 제어 서버 없이 프로젝트를 실행하는 최소 호스트.
use eframe::egui;
use engine_runtime::{PlaySession, RuntimeWorld};
struct Player {
    run: PlaySession,
    view: engine_host::GameView,
    time: std::time::Instant,
}
impl eframe::App for Player {
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if !ctx.input(|i| i.focused) {
            self.run.clear_input();
        }
        let now = std::time::Instant::now();
        self.run
            .advance(now.duration_since(self.time).as_secs_f64());
        self.time = now;
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.view.show(ui, &mut self.run, true));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() != 1 && !(arguments.len() == 3 && arguments[1] == "--headless-ticks") {
        return Err("Usage: engine-player PROJECT.json [--headless-ticks N]".into());
    }
    let session = engine_document::EditorSession::open(&arguments[0])?;
    let mut run = PlaySession::new(RuntimeWorld::snapshot(&session)?, 1);
    // smoke도 같은 runtime을 사용하며 파일을 수정하지 않는다. 배포용 제어 서버는 없다.
    if arguments.len() == 3 {
        let ticks: u32 = arguments[2].to_str().ok_or("tick 인코딩 오류")?.parse()?;
        run.pause(true);
        run.step(ticks)?;
        println!("{}", serde_json::to_string(run.status())?);
        return Ok(());
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 540.0])
            .with_title("Engine-1 Player"),
        system_font_fallback: true,
        ..Default::default()
    };
    eframe::run_native(
        "Engine-1 Player",
        options,
        Box::new(move |_| {
            Ok(Box::new(Player {
                run,
                view: Default::default(),
                time: std::time::Instant::now(),
            }))
        }),
    )?;
    Ok(())
}
