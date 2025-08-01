use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Terminal events.
#[derive(Clone, Debug)]
pub enum Event {
    /// Terminal tick.
    Tick,
    /// Key press.
    Key(KeyEvent),
    /// Mouse click/scroll.
    Mouse(()),
    /// Terminal resize.
    Resize((), ()),
}

/// Terminal event handler.
#[allow(dead_code)]
#[derive(Debug)]
pub struct EventHandler {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
    /// Event receiver channel.
    receiver: mpsc::UnboundedReceiver<Event>,
    /// Event handler thread.
    handler: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`].
    pub fn new(tick_rate: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate);
        let (sender, receiver) = mpsc::unbounded_channel();
        let _sender = sender.clone();

        let handler = tokio::spawn(async move {
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(Duration::ZERO);

                if event::poll(timeout).expect("no events available") {
                    match event::read().expect("unable to read event") {
                        CrosstermEvent::Key(e) => {
                            if e.kind == event::KeyEventKind::Press {
                                _sender
                                    .send(Event::Key(e))
                                    .expect("failed to send terminal event")
                            }
                        }
                        CrosstermEvent::Mouse(_) => _sender
                            .send(Event::Mouse(()))
                            .expect("failed to send terminal event"),
                        CrosstermEvent::Resize(_, _) => _sender
                            .send(Event::Resize((), ()))
                            .expect("failed to send terminal event"),
                        CrosstermEvent::FocusGained => {}
                        CrosstermEvent::FocusLost => {}
                        CrosstermEvent::Paste(_) => {}
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    _sender
                        .send(Event::Tick)
                        .expect("failed to send tick event");
                    last_tick = Instant::now();
                }
            }
        });

        Self {
            sender,
            receiver,
            handler,
        }
    }

    /// Receive the next event from the handler thread.
    pub async fn next(&mut self) -> Result<Event, Box<dyn std::error::Error + Send + Sync>> {
        self.receiver
            .recv()
            .await
            .ok_or("Event channel closed".into())
    }
}
