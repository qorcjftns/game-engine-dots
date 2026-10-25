//! 운영체제 종료 요청을 에디터의 미저장 보호 경로로 연결하기 위한 설정이다.

pub fn event_loop_hook() -> Option<eframe::EventLoopBuilderHook> {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;

        // 기본 Mac Quit 메뉴는 NSApplication.terminate:로 바로 종료해 Close 보호를 건너뛴다.
        // 기본 메뉴를 끄고 에디터의 파일 메뉴와 Command+Q에서 같은 전환 요청을 처리한다.
        Some(Box::new(|builder| {
            builder.with_default_menu(false);
        }))
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}
