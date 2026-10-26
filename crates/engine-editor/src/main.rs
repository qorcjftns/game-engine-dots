//! 게임 제작 도구의 시작점. 출력 검증 데모와 별도 실행 파일이다.

mod agent_control;
mod agent_editing;
mod app;
mod behavior_editor;
mod canvas;
mod canvas_input;
mod editing;
mod editor_actions;
mod inspector;
mod new_project;
mod platform;
mod play;
mod project_workflow;
mod raster;
mod resources;
mod startup;

#[cfg(test)]
mod editing_tests;

fn main() -> eframe::Result {
    let control = match startup::parse(std::env::args_os().skip(1)) {
        Ok(startup::Startup::Run(options)) => options,
        Ok(startup::Startup::Help) => {
            eprintln!("{}", startup::USAGE);
            return Ok(());
        }
        Err(error) => {
            eprintln!("{error}\n{}", startup::USAGE);
            std::process::exit(2);
        }
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Engine-1 · 2D 게임 에디터")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 640.0]),
        renderer: eframe::Renderer::Wgpu,
        event_loop_builder: platform::event_loop_hook(),
        // 설치된 OS 글꼴로 한국어를 표시한다. OS 글꼴을 배포 파일에 복사하지 않는다.
        system_font_fallback: true,
        ..Default::default()
    };

    eframe::run_native(
        "Engine-1 · 2D 게임 에디터",
        options,
        Box::new(move |creation| {
            let mut app = app::EditorApp::new(creation);
            app.start_agent_control(&creation.egui_ctx, control);
            Ok(Box::new(app))
        }),
    )
}
