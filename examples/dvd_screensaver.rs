// And once the timed version was working, I made a more true-to-life version of the DVD screensaver, having it change colors when it bounces, showcasing event-driven programming.

use macroquad::prelude::*;

#[macroquad::main("DVD Screensaver")]
async fn main() {
    // Load the DVD logo
    let texture: Texture2D = load_texture("examples/dvd-logo.png")
        .await
        .expect("Failed to load DVD logo");

    // Initialize radius, position, and velocity
    let radius = 150.0; // Radius means the distance from the center to the edge
    let mut x = screen_width() / 2.0;
    let mut y = screen_height() / 2.0;
    let mut speed_x = 300.0;
    let mut speed_y = 300.0;
    let mut color = WHITE;


    loop {
        
        // Clear screen each frame (no trail)
        clear_background(BLACK);

        // Get the elapsed time since the last frame
        let delta_time = get_frame_time();

        // Move the ball
        x += speed_x * delta_time;
        y += speed_y * delta_time;
        let mut bounced = false;

        // Bounce off  the left and right walls
        if x - radius < 0.0 { // If the ball hits the left wall
            x = radius;
            speed_x = -speed_x; // Negative of the x velocity makes it move the opposite horizontal direction
            bounced = true;
        }
        else if x + radius > screen_width() { // If the ball hits the right wall
            x = screen_width() - radius;
            speed_x = -speed_x;
            bounced = true;
        }

        // Bounce off the top and bottom walls
        if y - radius < 0.0 { // If the ball hits the top wall
            y = radius;
            speed_y = -speed_y; // Negative of the y velocity makes it move the opposite vertical direction
            bounced = true;
        }
        else if y + radius > screen_height() { // If the ball hits the bottom wall
            y = screen_height() - radius;
            speed_y = -speed_y;
            bounced = true;
        }

        // Macroquad colors use normalized channel values from 0.0 to 1.0.
        if bounced {
            color = Color::new(
                rand::gen_range(0.0, 1.0),
                rand::gen_range(0.0, 1.0),
                rand::gen_range(0.0, 1.0),
                1.0,
            );
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