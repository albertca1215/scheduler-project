// Once bounce_test was completed, I decided to create a more advanced version, using the iconic DVD logo and having it change colors over time, showcasing timed events.

use macroquad::prelude::*;

#[macroquad::main("DVD Screensaver")]
async fn main() {
    // Load the DVD logo
    let texture: Texture2D = load_texture("examples/dvd-logo.png")
        .await
        .expect("Failed to load DVD logo");

    // Initialize radius, position, and velocity
    let radius = 100.0; // Radius means the distance from the center to the edge
    let mut x = screen_width() / 2.0;
    let mut y = screen_height() / 2.0;
    let mut speed_x = 150.0;
    let mut speed_y = 150.0;
    let mut color_change_timer = 0.0;
    let mut color = WHITE;


    loop {
        
        // Clear screen each frame (no trail)
        clear_background(BLACK);

        // Get the elapsed time since the last frame. Time is measured in seconds, so 1.0 represents 1 second.
        let delta_time = get_frame_time();
        color_change_timer += delta_time;

        // Macroquad colors use normalized channel values from 0.0 to 1.0.
        if color_change_timer >= 1.0 {
            color_change_timer = 0.0;
            color = Color::new(
                rand::gen_range(0.0, 1.0),
                rand::gen_range(0.0, 1.0),
                rand::gen_range(0.0, 1.0),
                1.0,
            );
        }

        // Move the ball
        x += speed_x * delta_time;
        y += speed_y * delta_time;

        // Bounce off  the left and right walls
        if x - radius < 0.0 { // If the ball hits the left wall
            x = radius;
            speed_x = -speed_x; // Negative of the x velocity makes it move the opposite horizontal direction
        }
        else if x + radius > screen_width() { // If the ball hits the right wall
            x = screen_width() - radius;
            speed_x = -speed_x;
        }

        // Bounce off the top and bottom walls
        if y - radius < 0.0 { // If the ball hits the top wall
            y = radius;
            speed_y = -speed_y; // Negative of the y velocity makes it move the opposite vertical direction
        }
        else if y + radius > screen_height() { // If the ball hits the bottom wall
            y = screen_height() - radius;
            speed_y = -speed_y;
        }

        let texture_scale = (radius * 2.0) / texture.width().max(texture.height());
        let draw_size = vec2(
            texture.width() * texture_scale,
            texture.height() * texture_scale,
        );
        draw_texture_ex(
            &texture,
            x - draw_size.x / 2.0,
            y - draw_size.y / 2.0,
            color,
            DrawTextureParams {
                dest_size: Some(draw_size),
                ..Default::default()
            },
        );

        next_frame().await;
    }
}