use anyhow::Context;
use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use tokio::sync::mpsc;

const PAGE: &str = include_str!("console.html");

pub async fn serve(name: &str, port: u32) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(|| async { Html(PAGE) }))
        .route("/ws", get(ws_upgrade))
        .with_state(name.to_owned());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    println!("console for {name} at http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(name): State<String>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = pump(socket, &name).await {
            eprintln!("console session ended: {e:#}");
        }
    })
}

async fn pump(socket: WebSocket, name: &str) -> anyhow::Result<()> {
    let pty = NativePtySystem::default().openpty(PtySize {
        rows: 24,
        cols: 80,
        ..Default::default()
    })?;
    let mut cmd = CommandBuilder::new("virsh");
    cmd.args(["-c", "qemu:///system", "console", "--force", name]);
    let mut child = pty
        .slave
        .spawn_command(cmd)
        .context("spawning virsh console")?;
    drop(pty.slave);
    let mut reader = pty.master.try_clone_reader()?;
    let mut writer = pty.master.take_writer()?;

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(64);
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.blocking_send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    let (mut sink, mut stream) = socket.split();
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(bytes) => sink.send(Message::Binary(bytes.into())).await?,
                None => break, // virsh exited
            },
            msg = stream.next() => match msg {
                Some(Ok(Message::Binary(b))) => writer.write_all(&b)?,
                Some(Ok(Message::Text(t))) => writer.write_all(t.as_bytes())?,
                Some(Ok(_)) => {}
                _ => break, // browser closed
            },
        }
    }
    child.kill()?;
    Ok(())
}
