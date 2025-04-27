//This is a half serious I use for my ssh site so have fun :3

use std::{
    f64::consts::E,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use anyhow::Ok;
use async_trait::async_trait;
use rand::{seq::{IndexedRandom, SliceRandom}, Rng};
use ratatui::{
    layout::{Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{
        canvas::{Canvas, Circle, Line, Rectangle},
        Paragraph,
    },
    Frame,
};
use sshdance::{
    site::{Code, Page, SshInput, SshPage},
    SshDanceBuilder,
};
use tracing::{info, warn};

const COLOGS: [Color; 4] = [Color::Green, Color::Red, Color::Magenta, Color::Cyan];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    console_subscriber::init();
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 2222);
    SshDanceBuilder::new(socket, |_| Asteroids::new())
        .run()
        .await
}

pub struct Asteroids {
    asteroids: Vec<Asteroid>,
    circles: Vec<Circle>,
    spawn: usize,

    viewpoint_data: Option<(Rect, f64)>,
}

struct Asteroid {
    velocity: (i32, i32),
    pos: (u16, u16),
    start: (u16, u16),
    color: Color,
}

impl Asteroids {
    pub fn new() -> SshPage {
        Box::new(Asteroids {
            asteroids: Vec::new(),
            circles: Vec::new(),
            spawn: 0,
            viewpoint_data: None,
        }) as SshPage
    }

    pub fn render_background(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let (area, max_size) = self.viewpoint_data.unwrap();
        self.circles.retain(|x| x.radius < 25.0);

        let circles = &self.circles;

        let asteroids = &self.asteroids;

        let secondary = Canvas::default()
            .x_bounds([0f64, area.width as f64])
            .y_bounds([0f64, area.height as f64])
            .marker(ratatui::symbols::Marker::Dot)
            .paint(|canvas| {
                for ele in circles {
                    canvas.draw(ele);
                }
            });

        frame.render_widget(secondary, area);

        let primary = Canvas::default()
            .x_bounds([0f64, area.width as f64])
            .y_bounds([0f64, area.height as f64])
            .marker(ratatui::symbols::Marker::Bar)
            .paint(|canvas| {
                for ele in asteroids {
                    let (current_x, current_y) = ele.pos;
                    let (start_x, start_y) = ele.start;
                    canvas.draw(&Line::new(
                        current_x as f64,
                        current_y as f64,
                        start_x as f64,
                        start_y as f64,
                        ele.color.clone(),
                    ));
                }

                canvas.layer();

                for ele in asteroids {
                    let (x, y) = ele.pos;
                    canvas.draw(&Rectangle {
                        x: x.into(),
                        y: y.into(),
                        width: 1f64,
                        height: 1f64,
                        color: Color::White,
                    });
                }
            });

        frame.render_widget(primary, area);
    }

    fn update_rect(&mut self, area: Rect) {
        let max_radius = f64::sqrt((area.height as f64).powi(2) + (area.width as f64).powi(2));
        self.viewpoint_data = Some((area, max_radius));
    }
}

#[async_trait]
impl Page for Asteroids {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.update_rect(area);
        self.render_background(frame, area);
    }

    fn get_tps(&self) -> Option<u16> {
        Some(20)
    }

    fn tick(&mut self) -> anyhow::Result<Code> {
        let mut rand = rand::rng();

        if let Some((area, _)) = self.viewpoint_data {
            match self.spawn.checked_sub(1) {
                Some(a) => {
                    self.spawn = a;
                }
                None => {
                    let start = (rand.random_range(area.x..area.x + area.width), area.bottom());
                    self.asteroids.push(Asteroid {
                        velocity: (rand.random_range(-10..10), rand.random_range(-10..0)),
                        pos: start,
                        start,
                        color: COLOGS.choose(&mut rand).unwrap().clone(),
                    });

                    self.spawn = 3;
                }
            };

            for ele in &mut self.circles {
                ele.radius += 5f64;
            }

            self.asteroids.retain_mut(|ele| {
                let (x, y) = ele.pos;
                let (x_vel, y_vel) = ele.velocity;

                let new_x = ((x as i32) + x_vel) as u16;
                let new_y = ((y as i32) + y_vel) as u16;

                if (area.x..area.x + area.width).contains(&new_x)
                    && (area.y..area.y + area.height).contains(&new_y)
                {
                    ele.pos = (new_x, new_y);
                    true
                } else {
                    self.circles.push(Circle {
                        x: x.into(),
                        y: y.into(),
                        radius: 1f64,
                        color: ele.color.clone(),
                    });
                    false
                }
            });
        }

        Ok(Code::Render)
    }

    async fn handle_input(&mut self, _input: SshInput) -> anyhow::Result<Code> {
        Ok(Code::SkipRenderer)
    }
}
