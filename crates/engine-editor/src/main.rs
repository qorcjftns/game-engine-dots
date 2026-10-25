//! 게임 제작 도구의 시작점. 출력 검증 데모와 별도 실행 파일이다.

mod app;
mod canvas;
mod new_project;
mod project_workflow;
mod raster;
mod resources;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Engine-1 · 2D 게임 에디터")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 640.0]),
        renderer: eframe::Renderer::Wgpu,
        // 설치된 OS 글꼴로 한국어를 표시한다. OS 글꼴을 배포 파일에 복사하지 않는다.
        system_font_fallback: true,
        ..Default::default()
    };

    eframe::run_native(
        "Engine-1 · 2D 게임 에디터",
        options,
        Box::new(|creation| Ok(Box::new(app::EditorApp::new(creation)))),
    )
}
