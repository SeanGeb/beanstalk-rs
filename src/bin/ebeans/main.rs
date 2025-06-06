mod args;

use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use futures::sink::SinkExt;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::{select, signal};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, warn, Level};

use crate::args::Args;
use beanstalk_rs::wire::events::BeanstalkClientEvent;
use beanstalk_rs::wire::{self, decoder};

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = Args::parse();

    // Logging
    if args.debug {
        tracing_subscriber::fmt()
            .with_max_level(Level::TRACE)
            .init();
    } else {
        tracing_subscriber::fmt().json().init();
    }

    if let Some(_wal_dir) = args.wal_dir {
        error!("unsupported configuration: WAL not yet implemented");
        return ExitCode::from(2);
    }

    // Cancellation and termination channel.
    // TODO: this termination channel is a mpsc - so could be repurposed when
    // implementing durability as a stream of events.
    let cancel = CancellationToken::new();
    {
        let cancel = cancel.clone();
        tokio::spawn(async move {
            if let Err(error) = signal::ctrl_c().await {
                warn!(%error, "something strange with ctrl-c handling!");
            };
            cancel.cancel();
        });
    }

    let listener = match TcpListener::bind((args.listen, args.port)).await {
        Ok(l) => l,
        Err(error) => {
            error!(%error, "failed to listen for connections");
            return ExitCode::from(111);
        },
    };

    let (shutdown_hold, mut shutdown_wait) = mpsc::channel::<()>(1);

    let exit_code =
        match accept_loop(cancel, shutdown_hold, listener, args.max_job_size)
            .await
        {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                error!(%error, "encountered runtime error");
                ExitCode::FAILURE
            },
        };

    shutdown_wait.recv().await;

    exit_code
}

async fn accept_loop(
    cancel: CancellationToken,
    shutdown_hold: mpsc::Sender<()>,
    listener: TcpListener,
    max_job_size: u32,
) -> Result<()> {
    info!(addr = %listener.local_addr()?, "listening");

    // Accept incoming connections until an exit signal is sent, and handle each
    // connection as its own task.
    loop {
        match select! {
            accept = listener.accept() => accept,
            _ = cancel.cancelled() => return Ok(()),
        } {
            Ok((conn, _)) => {
                tokio::spawn(do_client_loop(
                    cancel.clone(),
                    shutdown_hold.clone(),
                    conn,
                    max_job_size,
                ));
            },
            Err(error) => {
                warn!(%error, "failed to accept connection");
                continue;
            },
        };
    }
}

#[instrument(name = "client_loop", err(level = Level::WARN), fields(peer = %conn.peer_addr()?), skip_all)]
async fn do_client_loop(
    cancel: CancellationToken,
    _shutdown_hold: mpsc::Sender<()>,
    conn: TcpStream,
    max_job_size: u32,
) -> Result<()> {
    use wire::protocol::*;

    debug!("accepted connection");

    conn.set_nodelay(true).context("setting NODELAY")?;

    let mut framed = wire::framed(conn);

    let conn_result = loop {
        let evt = select! {
            x = framed.next() => match x {
                None => {
                    debug!("connection dropped");
                    break Ok(())
                },
                Some(r) => r,
            },
            _ = cancel.cancelled() => break Ok(()),
        };

        let evt = match evt {
            Ok(BeanstalkClientEvent::Discarded) => continue,
            Ok(e) => e,
            Err(decoder::Error::IO(e)) => break Err(e.into()),
            Err(decoder::Error::Client(resp)) => {
                // Decoder says to send a particular response to the client
                select! {
                    x = framed.send(resp) => x?,
                    _ = cancel.cancelled() => break Ok(()),
                }

                break Err(anyhow!(
                    "client sent bad request and was disconnected"
                ));
            },
        };

        let BeanstalkClientEvent::Command(cmd) = evt else {
            framed.send(Response::BadFormat).await?;
            continue;
        };

        let resp = match cmd {
            _ => Response::InternalError,
        };

        select! {
            x = framed.send(resp) => x?,
            _ = cancel.cancelled() => break Ok(()),
        }
    };

    framed
        .into_inner()
        .shutdown()
        .await
        .context("during shutdown")?;

    conn_result
}
