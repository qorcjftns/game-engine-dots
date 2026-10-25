//! 첫 렌더링 확인용 데모. 게임 전용 상태는 추후 examples/로 옮긴다.

use engine_core::{LOGICAL_HEIGHT, LOGICAL_WIDTH, clock::FixedClock, viewport::Viewport};
use macroquad::prelude::*;

fn window_configuration() -> Conf {
    Conf {
        window_title: "Engine-1 | Arrow keys: move | Esc: exit".to_owned(),
        window_width: (LOGICAL_WIDTH * 3) as i32,
        window_height: (LOGICAL_HEIGHT * 3) as i32,
        window_resizable: true,
        // 첫 버전은 OS 좌표와 출력 픽셀 간 배율을 단순하게 유지한다.
        high_dpi: false,
        ..Default::default()
    }
}

#[macroquad::main(window_configuration)]
async fn main() {
    let target = render_target(LOGICAL_WIDTH, LOGICAL_HEIGHT);
    target.texture.set_filter(FilterMode::Nearest);
    let camera = Camera2D {
        render_target: Some(target.clone()),
        ..Camera2D::from_display_rect(Rect::new(
            0.0,
            0.0,
            LOGICAL_WIDTH as f32,
            LOGICAL_HEIGHT as f32,
        ))
    };

    let mut clock = FixedClock::default();
    let mut player = vec2(40.0, 80.0);

    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }

        let direction = vec2(
            i32::from(is_key_down(KeyCode::Right)) as f32
                - i32::from(is_key_down(KeyCode::Left)) as f32,
            i32::from(is_key_down(KeyCode::Down)) as f32
                - i32::from(is_key_down(KeyCode::Up)) as f32,
        )
        .normalize_or_zero();

        for _ in 0..clock.advance(f64::from(get_frame_time())) {
            // 계산에는 소수점을 유지하고 그릴 때만 반올림한다.
            player += direction * 1.5;
            player.x = player.x.clamp(0.0, LOGICAL_WIDTH as f32 - 8.0);
            player.y = player.y.clamp(0.0, LOGICAL_HEIGHT as f32 - 8.0);
        }

        set_camera(&camera);
        clear_background(Color::from_rgba(20, 24, 40, 255));
        for x in (0..LOGICAL_WIDTH).step_by(16) {
            draw_rectangle(x as f32, 0.0, 1.0, LOGICAL_HEIGHT as f32, DARKGRAY);
        }
        for y in (0..LOGICAL_HEIGHT).step_by(16) {
            draw_rectangle(0.0, y as f32, LOGICAL_WIDTH as f32, 1.0, DARKGRAY);
        }
        draw_rectangle(player.x.round(), player.y.round(), 8.0, 8.0, GOLD);

        set_default_camera();
        clear_background(BLACK);
        if let Some(viewport) = Viewport::fit(screen_width() as u32, screen_height() as u32) {
            draw_texture_ex(
                &target.texture,
                viewport.x as f32,
                viewport.y as f32,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(viewport.width as f32, viewport.height as f32)),
                    flip_y: true,
                    ..Default::default()
                },
            );
        }

        next_frame().await;
    }
}
