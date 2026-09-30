// A simple loop test using Macroquad. This was my first time getting my feet wet with the basics of simulation in Rust, starting with a rectangle moving across the screen.

use macroquad::prelude::*;

#[macroquad::main("Loop Decoupling Test")]
async fn main() {
    let mut x = 0.0;

    // Runs loop to animate a white square moving right across the screen (increasing x)
    loop {
        x += 1.0;
        if x > screen_width() { x = 0.0; }

        clear_background(BLACK);
        draw_rectangle(x, 100.0, 30.0, 30.0, WHITE);
        next_frame().await;
    }
}