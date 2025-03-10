# SSHDance

So here is a small networking lib made by me that **hopefully** will make it a bit easier to make ssh sites...

I do not like being overly formal so if you want to see an example of it working look at `examples/intro`. There probably be a branch in this repo titled `qubic-experimental` or something that I will push changes I'm making to SSHdance for my site...

Have fun and may meme driven development be with you

## Security
This is less secure than correctly setup HTTPs. There are no protections against potential MITM-attacks on first connections, depending on client configuration, public keys are typically stored to compare with the key served from the server. This also means there are no proper means to rotate keys without it looking like a MITM-attack.

**Use at your own risk!**

## I have no attention span and want to get this working NOW

Great get yourself some `nix` and run `nix flake init --template github:AwesomeQubic/sshdance/qubic-experimental`.<br>
Remember to enable nix-command and flakes in your nix configs see the [docs](https://nixos.wiki/wiki/Flakes)

## I want to help develop this lib

Well great tho look at the root of this project you see a file called `flake.nix`?

If yes then great, its a file used to build environments using `nix` package manager and should get us to use same compiler and everything, so I politely ask you to either use NixOs or get standalone nix to work on this project

## I have nix now what do I work on?

Well here is a list of my current TODOs:

 - Nix templates so we do not have to bother people with getting this lib to work
 - Better input handling
 - Make this more formal

## Example

```rust
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use anyhow::Ok;
use async_trait::async_trait;
use ratatui::{
    layout::Rect,
    widgets::Paragraph,
    Frame,
};
use sshdance::{
    site::{Code, Page, SshInput, SshPage},
    SshDanceBuilder,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 2222);
    SshDanceBuilder::new(socket, |_| HelloWorld::new())
        .run()
        .await
}

pub struct HelloWorld;

impl HelloWorld {
    pub fn new() -> SshPage {
        Box::new(HelloWorld) as SshPage
    }
}

#[async_trait]
impl Page for HelloWorld {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_widget(Paragraph::new("Hello world"), area);
    }

    async fn handle_input(&mut self, _input: SshInput) -> anyhow::Result<Code> {
        Ok(Code::Render)
    }
}
```