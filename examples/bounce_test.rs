// A simple bounce test using Macroquad. Here, I learned how to implement basic physics in a 2D environment by making a ball bounce off the edges of the screen.

use macroquad::prelude::*;

#[macroquad::main("Bounce Test")]
async fn main() {
    // Initialize radius, position, and velocity
    let radius = 30.0; // Radius means the distance from the center to the edge
    let mut x = screen_width() / 2.0;
    let mut y = screen_height() / 2.0;
    let mut speed_x = 300.0;
    let mut speed_y = 300.0;

    loop {
        // Clear screen each frame (no trail)
        clear_background(BLACK);

        // Get the elapsed time since the last frame
        let delta_time = get_frame_time();

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

        draw_circle(x, y, radius, WHITE);

        next_frame().await;
    }
}