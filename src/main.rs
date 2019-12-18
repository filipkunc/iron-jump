use std::env;
use std::path;
use std::f32;

use nalgebra as na;

use ggez::{graphics, Context, GameResult};
use ggez::graphics::{Image, Rect};
use ggez::event::{self, EventHandler, KeyCode, KeyMods};
use ggez::*;

const COLLISION_TOLERANCE: f32 = 3.0;
const RECT_TOLERANCE: f32 = 0.1;
const MAX_SPEED: f32 = 5.8;
const SPEED_POWER_UP: f32 = 1.5;
const UP_SPEED: f32 = 7.0;
const MAX_FALL_SPEED: f32= -15.0;
const ACCELERATION: f32 = 1.1;
const DECELERATION: f32 = 1.1 * 0.2;
const CHANGE_DIRECTION_SPEED: f32 = 3.0;
const MAX_SPEEDUP_COUNT: i32 = 60 * 6; // 60 FPS * 6 sec

const SCREEN_WIDTH: f32 = 480.0;
const SCREEN_HEIGHT: f32 = 320.0;
const TILE_SIZE: f32 = 32.0;

fn rect_intersection(a: Rect, b: Rect) -> Rect {
    let x = f32::max(a.left(), b.left());
    let y = f32::max(a.top(), b.top());
    
    let width = f32::min(a.right(), b.right()) - x;
    let height = f32::min(a.bottom(), b.bottom()) - y;
    
    let intersection = Rect::new(x, y, width, height);
    
    if intersection.w < RECT_TOLERANCE || intersection.h < RECT_TOLERANCE {
        return Rect::zero();
    }
    return intersection;
}

fn rect_is_empty_with_tolerance(a: Rect) -> bool {
    a.w < RECT_TOLERANCE || a.h < RECT_TOLERANCE
}

trait GameObject {
    fn move_world(&mut self, offset_x: f32, offset_y: f32);
    fn draw(&self, game: &Game, ctx: &mut Context) -> GameResult<()>;
    fn update(&mut self);
    fn is_platform(&self) -> bool;
    fn rect(&self) -> Rect;
}

struct Player {
    x: f32,
    y: f32,
    move_x: f32,
    move_y: f32,
    jumping: bool,
    alpha: f32,
    rotation: f32,
    speed_up_counter: i32
}

impl Player {
    fn new(x: f32, y: f32) -> Player {
        Player{
            x,
            y,
            move_x: 0.0,
            move_y: 0.0,
            jumping: false,
            alpha: 1.0,
            rotation: 0.0,
            speed_up_counter: 0
        }
    }

    fn draw(&self, game: &Game, ctx: &mut Context) -> GameResult<()> {
        graphics::draw(ctx, &game.player_image,
            graphics::DrawParam::new()
                .dest(na::Point2::new(self.x + TILE_SIZE / 2.0, self.y + TILE_SIZE / 2.0))
                .rotation(self.rotation)
                .offset(na::Point2::new(0.5, 0.5))
            )
    }

    fn update_from_input(&mut self, input_acceleration: na::Vector2<f32>) {
        let mut move_left_or_right = false;

        if self.speed_up_counter > 0 {
            self.speed_up_counter += 1;
            if self.speed_up_counter > MAX_SPEEDUP_COUNT {
                self.speed_up_counter = 0;
            }
        }

        let current_max_speed = if self.speed_up_counter > 0 { MAX_SPEED * SPEED_POWER_UP } else { MAX_SPEED };

        if input_acceleration.x < 0.0 {
            if self.move_x < 0.0 {
                self.move_x += f32::abs(input_acceleration.x) * ACCELERATION * CHANGE_DIRECTION_SPEED;
            }
            self.move_x += f32::abs(input_acceleration.x) * ACCELERATION;
            if self.move_x > current_max_speed {
                self.move_x = current_max_speed;
            }

            move_left_or_right = true;
        }
        else if input_acceleration.x > 0.0 {
            if self.move_x > 0.0 {
                self.move_x -= f32::abs(input_acceleration.x) * ACCELERATION * CHANGE_DIRECTION_SPEED;
            }
            self.move_x -= f32::abs(input_acceleration.x) * ACCELERATION;
            if self.move_x < -current_max_speed {
                self.move_x = -current_max_speed;
            }
            move_left_or_right = true;
        }

        if !self.jumping && input_acceleration.y > 0.0 {
            if self.move_y < UP_SPEED {
                self.move_y = UP_SPEED;
            }
            self.jumping = true;
        }

        if !move_left_or_right {
            if f32::abs(self.move_x) < DECELERATION {
                self.move_x = 0.0;
            }
            else if self.move_x > 0.0 {
                self.move_x -= DECELERATION;
            }
            else if self.move_x < 0.0 {
                self.move_x += DECELERATION;
            }
        }

        self.move_y -= DECELERATION;
        if self.move_y < MAX_FALL_SPEED {
            self.move_y = MAX_FALL_SPEED;
        }
        self.jumping = true;

        self.alpha += 0.07;
        if self.alpha > f32::consts::PI {
            self.alpha -= f32::consts::PI;
        }
    }

    fn update_after_collision(&mut self) {
        let unit_velocity = self.move_x / (TILE_SIZE / 2.0);
        self.rotation -= unit_velocity * 0.55;
    }

    fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, TILE_SIZE, TILE_SIZE)
    }
}

struct Platform {
    x: f32,
    y: f32,
    width_segments: i32,
    height_segments: i32,
}

impl GameObject for Platform {
    fn move_world(&mut self, offset_x: f32, offset_y: f32) {
        self.x += offset_x;
        self.y += offset_y;
    }

    fn draw(&self, game: &Game, ctx: &mut Context) -> GameResult<()> {
        game.draw_tiles(ctx, &game.platform_image,
            self.x, self.y, self.width_segments, self.height_segments)
    }

    fn update(&mut self) {

    }

    fn is_platform(&self) -> bool { true }
    fn rect(&self) -> Rect {
        Rect::new(self.x, self.y,
            self.width_segments as f32 * TILE_SIZE,
            self.height_segments as f32 * TILE_SIZE)
    }
}

struct Game {
    player_image: Image,
    platform_image: Image,
    background_image: Image,

    player: Player,
    game_objects: Vec<Box<dyn GameObject>>,

    input_acceleration: na::Vector2<f32>,
    background_offset: na::Vector2<f32>,
}

impl Game {
    pub fn new(ctx: &mut Context) -> ggez::GameResult<Game> {
        let player_image = Image::new(ctx, "/ball.png")?;
        let platform_image = Image::new(ctx, "/platform.png")?;
        let background_image = Image::new(ctx, "/background.png")?;

        let mut game_objects = Vec::<Box<dyn GameObject>>::new();
        game_objects.push(Box::new(Platform{
            x: 256.0,
            y: 512.0,
            width_segments: 11,
            height_segments: 5
        }));
        game_objects.push(Box::new(Platform{
            x: 768.0,
            y: 512.0,
            width_segments: 10,
            height_segments: 5
        }));
        game_objects.push(Box::new(Platform{
            x: 1216.0,
            y: 512.0,
            width_segments: 9,
            height_segments: 5
        }));

        let player_x = 384.0;
        let player_y = 480.0;

        let center_x = SCREEN_WIDTH / 2.0 - TILE_SIZE / 2.0;
        let center_y = SCREEN_HEIGHT / 2.0 - TILE_SIZE / 2.0;

        let player = Player::new(center_x, center_y);

        let input_acceleration = na::Vector2::new(0.0, 0.0);
        let background_offset = na::Vector2::new(0.0, 0.0);

        let mut game = Game {
            player_image,
            platform_image,
            background_image,
            player,
            game_objects,
            input_acceleration,
            background_offset
        };

        game.move_world(center_x - player_x, center_y - player_y);

        Ok(game)
    }

    pub fn draw_tiles(&self, ctx: &mut Context, image: &Image,
        x: f32, y: f32, width_segments: i32, height_segments: i32) -> GameResult<()> {
        for iy in 0..height_segments {
            for ix in 0..width_segments {
                graphics::draw(ctx, image,
                    (na::Point2::new(x + ix as f32 * TILE_SIZE, y + iy as f32 * TILE_SIZE), ))?;
            }
        }
        Ok(())
    }

    pub fn move_world(&mut self, x: f32, y: f32) {
        for game_object in &mut self.game_objects {
            game_object.move_world(x, y);
        }

        self.background_offset.x += x * 0.25;
        self.background_offset.y += y * 0.25;
    }

    fn draw_background(&mut self, ctx: &mut Context) -> GameResult<()> {
        let offset = na::Vector2::new(
            self.background_offset.x % TILE_SIZE - TILE_SIZE,
            self.background_offset.y % TILE_SIZE - TILE_SIZE);

        self.draw_tiles(ctx, &self.background_image,
           offset.x, offset.y,
           SCREEN_WIDTH as i32 / TILE_SIZE as i32 + 3,
           SCREEN_HEIGHT as i32 / TILE_SIZE as i32 + 2
        )
    }

    fn collision_left_right(&mut self) {
        let mut is_colliding = false;
        let mut offset_x = 0.0;

        for platform in self.game_objects.iter_mut() {
            if platform.is_platform() {
                let intersection = rect_intersection(platform.rect(), self.player.rect());
                if rect_is_empty_with_tolerance(intersection) {
                    continue;
                }

                if platform.rect().left() > self.player.rect().left() {
                    offset_x = intersection.w;
                    is_colliding = true;
                }
                else if platform.rect().right() < self.player.rect().right() {
                    offset_x = -intersection.w;
                    is_colliding = true;
                }
            }
        }

        if is_colliding {
            self.move_world(offset_x, 0.0);
            self.player.move_x = 0.0;
        }
    }

    fn collision_up_down(&mut self) {
        let mut is_colliding = false;
        let mut offset_y = 0.0;
        
        for platform in self.game_objects.iter_mut() {
            if platform.is_platform() {
                let intersection = rect_intersection(platform.rect(), self.player.rect());
                if rect_is_empty_with_tolerance(intersection) {
                    continue;
                }

                if platform.rect().bottom() < self.player.rect().bottom() {
                    if self.player.move_y > 0.0 {
                        self.player.move_y = 0.0;
                    }

                    offset_y = -intersection.h;
                    is_colliding = true;
                }
                else if self.player.move_y < 0.0 {
                    if platform.rect().top() > self.player.rect().bottom() - COLLISION_TOLERANCE + self.player.move_y {
                        self.player.move_y = 0.0;
                        self.player.jumping = false;
                        offset_y = intersection.h;
                        is_colliding = true;
                    }
                }
                else if platform.rect().top() > self.player.rect().bottom() - COLLISION_TOLERANCE + self.player.move_y {
                    self.player.jumping = false;
                    offset_y = intersection.h;
                    is_colliding = true;
                }
            }
        }

        if is_colliding {
            self.move_world(0.0, offset_y);
        }
    }
}

impl EventHandler for Game {
    fn key_down_event(&mut self, _ctx: &mut Context, keycode: KeyCode, _keymods: KeyMods, _repeat: bool) {
        if keycode == KeyCode::Left {
            self.input_acceleration.x = -1.0;
        } 
        else if keycode == KeyCode::Right {
            self.input_acceleration.x = 1.0;
        }
        else if keycode == KeyCode::Up {
            self.input_acceleration.y = 1.0;
        }
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: KeyCode, _keymods: KeyMods) {
        if keycode == KeyCode::Left {
            self.input_acceleration.x = 0.0;
        } 
        else if keycode == KeyCode::Right {
            self.input_acceleration.x = 0.0;
        }
        else if keycode == KeyCode::Up {
            self.input_acceleration.y = 0.0;
        }
    }

    fn update(&mut self, _ctx: &mut Context) -> GameResult<()> {
        for game_object in self.game_objects.iter_mut() {
            game_object.update();
        }

        self.player.update_from_input(self.input_acceleration);
        self.move_world(self.player.move_x, 0.0);
        self.collision_left_right();
        self.move_world(0.0, self.player.move_y);
        self.collision_up_down();
        self.player.update_after_collision();

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx, graphics::WHITE);

        self.draw_background(ctx)?;

        for game_object in &self.game_objects {
            game_object.draw(self, ctx)?;
        }

        self.player.draw(self, ctx)?;

        graphics::present(ctx)
    }
}

fn main() -> GameResult {
    let resource_dir = if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let mut path = path::PathBuf::from(manifest_dir);
        path.push("resources");
        path
    } else {
        path::PathBuf::from("./resources")
    };

    let cb = ggez::ContextBuilder::new("iron-jump", "ggez")
        .add_resource_path(resource_dir)
        .window_setup(ggez::conf::WindowSetup::default().title("iron-jump"))
        .window_mode(ggez::conf::WindowMode::default().dimensions(SCREEN_WIDTH, SCREEN_HEIGHT));
    
    let (ctx, event_loop) = &mut cb.build()?;
    let state = &mut Game::new(ctx)?;
    event::run(ctx, event_loop, state)
}
