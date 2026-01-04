//This is a half serious I use for my ssh site so have fun :3

use rand::{seq::IndexedRandom, Rng};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::canvas::{Canvas, Circle, Line, Rectangle},
    Frame,
};
use sshdance::{
    api::{
        term::{CallbackRez, SshTerminal},
        utils::SimpleTerminalHandler,
    },
    SshDanceBuilder,
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::NonZero,
};

const COLOGS: [Color; 4] = [Color::Green, Color::Red, Color::Magenta, Color::Cyan];

#[tokio::main]
async fn main() -> Result<(), sshdance::Error> {
    tracing_subscriber::fmt::init();
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 2223);
    SshDanceBuilder::<SimpleTerminalHandler<Asteroids>>::new(socket)
        .run()
        .await
        .unwrap();
    Ok(())
}

#[derive(Default)]
pub struct Asteroids {
    asteroids: Vec<Asteroid>,
    circles: Vec<Circle>,
    spawn: usize,
}

struct Asteroid {
    velocity: (i32, i32),
    pos: (u16, u16),
    start: (u16, u16),
    color: Color,
}

impl SshTerminal for Asteroids {
    type MessageType = ();
    const DEFAULT_TPS: Option<std::num::NonZero<u8>> = Some(NonZero::new(10).unwrap());

    fn on_animation(
        &mut self,
        engine: &mut impl sshdance::api::term::EngineRef<Self>,
    ) -> CallbackRez {
        let mut rand = rand::rng();
        let area = engine.current_size();

        match self.spawn.checked_sub(1) {
            Some(a) => {
                self.spawn = a;
            }
            None => {
                let start = (rand.random_range(0..area.width), area.height);
                self.asteroids.push(Asteroid {
                    velocity: (rand.random_range(-10..10), rand.random_range(-10..0)),
                    pos: start,
                    start,
                    color: COLOGS.choose(&mut rand).unwrap().clone(),
                });

                self.spawn = rand.random_range(0..3);
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

            if (0..area.width).contains(&new_x) && (0..area.height).contains(&new_y) {
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

        CallbackRez::PushToRenderer
    }

    fn draw(&mut self, frame: &mut Frame<'_>) {
        let area = frame.area();
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

        let line = ratatui::text::Line::default()
            .spans([Span::styled(
                "SSHDance",
                Style::new().add_modifier(Modifier::BOLD).fg(Color::Reset),
            )])
            .centered();
        let centered_y = area.height / 2;
        let new_area: Rect = Rect::new(area.x, centered_y, area.width, 1);
        frame.render_widget(line, new_area);

        let line = ratatui::text::Line::default()
            .spans([Span::styled(
                "[Install from crates to proceed]",
                Style::new().fg(Color::Reset).add_modifier(Modifier::BOLD),
            )])
            .centered();
        let new_area = Rect::new(area.x, centered_y + 1, area.width, 1);
        frame.render_widget(line, new_area);
    }
}
