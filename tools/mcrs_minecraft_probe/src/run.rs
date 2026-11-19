use crate::client::{self, Event, LoadSignal, Login, Order, QueuedOrder, Signal};
use crate::swing::CapturedSwing;
use crate::{
    Action, AnimateObserved, ChatMessage, Checkpoint, CommandTree, ConnectionState, Direction,
    Disconnect, ExpectedPacket, Host, MenuEvent, Observe, Observed, PacketKind, PacketObserved,
    Scenario, StepTrace, SwingObserved, SwingWireCheck, Trace, TraceVersion,
};
use anyhow::Context;
use mcrs_minecraft_core::VERSION;
use std::collections::BTreeMap;
use std::mem;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::time;

const FIRST_EVENT: Duration = Duration::from_secs(10);
const FIRST_PACKET: Duration = Duration::from_secs(2);
const JOIN_LIMIT: Duration = Duration::from_secs(60);

struct Clients {
    tracks_pose: bool,
    known_players: Vec<String>,
    sink: SettleSink,
    events_in: UnboundedSender<Event>,
    events: UnboundedReceiver<Event>,
    next_suggest_id: i32,
    step_start: time::Instant,
    packet_evidence: Vec<Event>,
    early_terminals: Vec<Event>,
}

impl Clients {
    fn new(
        tracks_pose: bool,
        known_players: Vec<String>,
        quiet: Duration,
        observe: Observe,
    ) -> Self {
        let (events_in, events) = mpsc::unbounded_channel();
        Self {
            tracks_pose,
            known_players,
            sink: SettleSink::new(quiet, observe),
            events_in,
            events,
            next_suggest_id: 1,
            step_start: time::Instant::now(),
            packet_evidence: Vec::new(),
            early_terminals: Vec::new(),
        }
    }
}

struct SettleSink {
    quiet: Duration,
    observe: Observe,
    menu: Vec<MenuEvent>,
    sounds: Vec<crate::SoundPlayed>,
    trees: Vec<CommandTree>,
    swings: Vec<CapturedSwing>,
    animates: Vec<AnimateObserved>,
    packets: Vec<PacketObserved>,
    checkpoints: Vec<Checkpoint>,
    orders: BTreeMap<String, UnboundedSender<QueuedOrder>>,
    packet_sample_start: Option<time::Instant>,
}

impl SettleSink {
    fn new(quiet: Duration, observe: Observe) -> Self {
        Self {
            quiet,
            observe,
            menu: Vec::new(),
            sounds: Vec::new(),
            trees: Vec::new(),
            swings: Vec::new(),
            animates: Vec::new(),
            packets: Vec::new(),
            checkpoints: Vec::new(),
            orders: BTreeMap::new(),
            packet_sample_start: None,
        }
    }
}

enum RecvOutcome {
    Event(Event),
    Closed,
    Timeout,
}

trait SettleClock {
    fn now(&self) -> time::Instant;
    fn try_recv(&mut self) -> RecvOutcome;
    async fn recv_before(&mut self, deadline: time::Instant) -> RecvOutcome;
}

struct LiveClock<'a> {
    events: &'a mut UnboundedReceiver<Event>,
}

impl SettleClock for LiveClock<'_> {
    fn now(&self) -> time::Instant {
        time::Instant::now()
    }

    fn try_recv(&mut self) -> RecvOutcome {
        match self.events.try_recv() {
            Ok(event) => RecvOutcome::Event(event),
            Err(mpsc::error::TryRecvError::Disconnected) => RecvOutcome::Closed,
            Err(mpsc::error::TryRecvError::Empty) => RecvOutcome::Timeout,
        }
    }

    async fn recv_before(&mut self, deadline: time::Instant) -> RecvOutcome {
        match time::timeout_at(deadline, self.events.recv()).await {
            Ok(Some(event)) => RecvOutcome::Event(event),
            Ok(None) => RecvOutcome::Closed,
            Err(_) => RecvOutcome::Timeout,
        }
    }
}

pub(crate) fn is_sequence_step(action: &Action) -> bool {
    order_of(action).is_some()
        || matches!(
            action,
            Action::Wait { .. }
                | Action::Console { .. }
                | Action::AwaitPacket { .. }
                | Action::AdvanceTicks { .. }
                | Action::AwaitPlayerTicks { .. }
        )
}

pub async fn run<H: Host>(
    scenario: &Scenario,
    address: SocketAddr,
    quiet: Duration,
    host: &mut H,
) -> anyhow::Result<Trace> {
    let tracks_pose =
        client::needs_world_acks(&scenario.observe) || crate::scenario::needs_pose(&scenario.steps);
    let mut clients = Clients::new(
        tracks_pose,
        collect_players(scenario),
        quiet,
        scenario.observe.clone(),
    );
    let mut steps = Vec::with_capacity(scenario.steps.len());
    for step in &scenario.steps {
        clients.step_start = time::Instant::now();
        clients.packet_evidence.clear();
        clients.early_terminals.clear();
        clients.sink.packet_sample_start =
            (scenario.observe.packet_order && step.packet_order).then_some(clients.step_start);
        let observed = clients
            .act(&step.action, address, host)
            .await
            .with_context(|| format!("step {}", step.id))?;
        let player_bans = if step.read_player_bans {
            let mut bans = host.read_player_bans()?;
            bans.sort_by(|left, right| left.name.cmp(&right.name));
            Some(bans)
        } else {
            None
        };
        let player_file = match &step.read_player_file {
            Some(read) => {
                let file = host.read_player_file(&read.client).with_context(|| {
                    format!("step {}: the player file of {}", step.id, read.client)
                })?;
                let fields = read
                    .fields
                    .iter()
                    .map(|field| {
                        (
                            field.clone(),
                            file.get(field).map(mcrs_minecraft_snbt::write),
                        )
                    })
                    .collect();
                Some(fields)
            }
            None => None,
        };
        let menu = mem::take(&mut clients.sink.menu);
        let sounds = mem::take(&mut clients.sink.sounds);
        let trees = mem::take(&mut clients.sink.trees);
        let captured_swings = mem::take(&mut clients.sink.swings);
        let swing_bytes = scenario
            .observe
            .swing
            .then(|| compute_swing_bytes(&captured_swings))
            .flatten();
        let swings: Vec<SwingObserved> = captured_swings
            .into_iter()
            .map(|entry| entry.observed)
            .collect();
        let animates = mem::take(&mut clients.sink.animates);
        let packets = mem::take(&mut clients.sink.packets);
        let mut step_trace = StepTrace {
            id: step.id.clone(),
            observed,
            player_bans,
            inventory: scenario.observe.inventory.then_some(menu),
            sounds: scenario.observe.sound.then_some(sounds),
            command_trees: (!scenario.observe.commands.is_empty()).then_some(trees),
            swings: scenario.observe.swing.then_some(swings),
            swing_bytes,
            animates: scenario.observe.animate.then_some(animates),
            packets: (scenario.observe.packet_order && step.packet_order).then_some(packets),
            checkpoints: mem::take(&mut clients.sink.checkpoints),
            player_file,
        };
        step_trace.normalize_per_client();
        steps.push(step_trace);
    }
    Ok(Trace {
        version: TraceVersion {
            name: VERSION.name.to_owned(),
            protocol: VERSION.protocol_version,
        },
        steps,
    })
}

pub(crate) fn compute_swing_bytes(captured: &[CapturedSwing]) -> Option<SwingWireCheck> {
    if captured.is_empty() {
        return None;
    }
    let canonical = captured.iter().all(|entry| entry.canonical);
    let mut client_counts: BTreeMap<(&str, i32), usize> = BTreeMap::new();
    let mut groups: BTreeMap<(i32, usize), Vec<&[u8]>> = BTreeMap::new();
    for entry in captured {
        let occ = client_counts
            .entry((entry.observed.client.as_str(), entry.entity_id))
            .or_insert(0);
        groups
            .entry((entry.entity_id, *occ))
            .or_default()
            .push(entry.raw_body.as_slice());
        *occ += 1;
    }
    let mut shared_groups = groups.values().filter(|bodies| bodies.len() > 1).peekable();
    let shared_across_recipients = if shared_groups.peek().is_some() {
        Some(shared_groups.all(|bodies| match bodies.first() {
            Some(first) => bodies.iter().all(|body| body == first),
            None => true,
        }))
    } else {
        None
    };
    Some(SwingWireCheck {
        canonical,
        shared_across_recipients,
    })
}

impl Clients {
    async fn act<H: Host>(
        &mut self,
        action: &Action,
        address: SocketAddr,
        host: &mut H,
    ) -> anyhow::Result<Observed> {
        if let Some((client, order)) = order_of(action) {
            return self.send(client, order).await;
        }
        match action {
            Action::Join(name) => self.join(address, name, LoadSignal::Send).await,
            Action::JoinDeferred(name) => self.join(address, name, LoadSignal::Defer).await,
            Action::Leave(name) => self.leave(name).await,
            Action::Command { client, line } => self.command(client, line).await,
            Action::Suggest { client, line } => self.suggest(client, line).await,
            Action::Console { lines } => self.console(lines, host).await,
            Action::Wait { ms } => {
                time::sleep(Duration::from_millis(*ms)).await;
                let events = self.settle(self.sink.quiet).await?;
                collect_sent(&[], &events, Vec::new())
            }
            Action::AwaitPacket { client, packet } => {
                self.await_packet(client, packet).await?;
                self.finish_checkpoint().await
            }
            Action::AdvanceTicks { ticks } => {
                self.advance_ticks(host, *ticks).await?;
                self.finish_checkpoint().await
            }
            Action::AwaitPlayerTicks { client, ticks } => {
                self.await_player_ticks(host, client, *ticks).await?;
                self.finish_checkpoint().await
            }
            Action::Sequence { steps } => self.sequence(steps, host).await,
            Action::RestartServer {} => self.restart(host).await,
            _ => anyhow::bail!("unhandled probe action"),
        }
    }

    async fn join(
        &mut self,
        address: SocketAddr,
        name: &str,
        load_signal: LoadSignal,
    ) -> anyhow::Result<Observed> {
        anyhow::ensure!(
            !self.sink.orders.contains_key(name),
            "{name} is already connected"
        );
        let login = time::timeout(JOIN_LIMIT, client::log_in(address, name))
            .await
            .with_context(|| format!("{name} did not join in {JOIN_LIMIT:?}"))??;
        let joined = match login {
            Login::Joined(joined) => joined,
            Login::Refused { state, reason } => return Ok(Observed::Refused { state, reason }),
        };
        let (orders_in, orders) = mpsc::unbounded_channel();
        self.sink.orders.insert(name.to_owned(), orders_in);
        tokio::spawn(client::session(
            joined,
            name.to_owned(),
            client::SessionConfig {
                observe: self.sink.observe.clone(),
                tracks_pose: self.tracks_pose,
                known_players: self.known_players.clone(),
                load_signal,
            },
            orders,
            self.events_in.clone(),
        ));
        let first_limit = if self.tracks_pose {
            FIRST_PACKET
        } else {
            self.sink.quiet
        };
        for event in self.settle(first_limit).await? {
            if event.client != name {
                continue;
            }
            let reason = match event.signal {
                Signal::Chat(_)
                | Signal::Menu(_)
                | Signal::Sound(_)
                | Signal::Commands(_)
                | Signal::Suggestions { .. }
                | Signal::Swing(_)
                | Signal::BlockAck(_)
                | Signal::Added(_)
                | Signal::Removed(_)
                | Signal::PositionAccepted(_)
                | Signal::Animate(_)
                | Signal::Packet(_) => continue,
                Signal::Disconnected(reason) => Some(reason),
                Signal::Closed | Signal::Failed(_) => None,
            };
            return Ok(Observed::Refused {
                state: ConnectionState::Play,
                reason,
            });
        }
        Ok(Observed::Joined)
    }

    async fn leave(&mut self, name: &str) -> anyhow::Result<Observed> {
        self.order_written(name, Order::Leave).await?;
        let closed = self.settle(FIRST_EVENT).await?.iter().any(|event| {
            event.client == name
                && matches!(
                    event.signal,
                    Signal::Disconnected(_) | Signal::Closed | Signal::Failed(_)
                )
        });
        anyhow::ensure!(closed, "the connection of {name} did not close");
        Ok(Observed::Left)
    }

    async fn command(&mut self, sender: &str, line: &str) -> anyhow::Result<Observed> {
        self.order_written(sender, Order::Command(line.to_owned()))
            .await?;
        let mut replies = Vec::new();
        let mut chat = Vec::new();
        let mut disconnects = Vec::new();
        for event in self.settle(FIRST_EVENT).await? {
            match event.signal {
                Signal::Chat(text) if event.client == sender => replies.push(text),
                Signal::Chat(text) if self.sink.observe.chat => chat.push(ChatMessage {
                    client: event.client,
                    text,
                }),
                Signal::Disconnected(reason) => disconnects.push(Disconnect {
                    client: event.client,
                    reason,
                }),
                Signal::Chat(_)
                | Signal::Menu(_)
                | Signal::Sound(_)
                | Signal::Commands(_)
                | Signal::Suggestions { .. }
                | Signal::Swing(_)
                | Signal::BlockAck(_)
                | Signal::Added(_)
                | Signal::Removed(_)
                | Signal::PositionAccepted(_)
                | Signal::Animate(_)
                | Signal::Packet(_)
                | Signal::Closed
                | Signal::Failed(_) => {}
            }
        }
        disconnects.sort_by(|left, right| left.client.cmp(&right.client));
        Ok(Observed::Command {
            replies,
            chat,
            disconnects,
        })
    }

    async fn suggest(&mut self, sender: &str, line: &str) -> anyhow::Result<Observed> {
        let request_id = self.next_suggest_id;
        self.next_suggest_id = self.next_suggest_id.wrapping_add(1).max(1);
        self.order_written(
            sender,
            Order::Suggest {
                id: request_id,
                line: line.to_owned(),
            },
        )
        .await?;
        let mut reply = None;
        let mut disconnects = Vec::new();
        let mut terminal = None;
        let match_deadline = time::Instant::now() + FIRST_PACKET;
        while let Ok(Some(event)) = time::timeout_at(match_deadline, self.events.recv()).await {
            let is_sender = event.client == sender;
            match event.signal {
                Signal::Suggestions {
                    id,
                    start,
                    length,
                    suggestions,
                } if is_sender && id == request_id => {
                    reply = Some((start, length, suggestions));
                    break;
                }
                Signal::Disconnected(reason) => {
                    self.sink.orders.remove(&event.client);
                    disconnects.push(Disconnect {
                        client: event.client,
                        reason: reason.clone(),
                    });
                    if is_sender {
                        terminal = Some(format!("disconnected: {reason:?}"));
                        break;
                    }
                }
                Signal::Closed => {
                    self.sink.orders.remove(&event.client);
                    if is_sender {
                        terminal = Some("connection closed".to_owned());
                        break;
                    }
                }
                Signal::Failed(error) => {
                    self.sink.orders.remove(&event.client);
                    anyhow::bail!("{} session failed: {error}", event.client);
                }
                other => {
                    self.sink.record_non_terminal(event.client, other, event.at);
                }
            }
        }
        if reply.is_some() || terminal.is_some() {
            for event in self.settle(self.sink.quiet).await? {
                if let Signal::Disconnected(reason) = event.signal {
                    disconnects.push(Disconnect {
                        client: event.client,
                        reason,
                    });
                }
            }
        }
        disconnects.sort_by(|left, right| left.client.cmp(&right.client));
        let Some((start, length, suggestions)) = reply else {
            if let Some(cause) = terminal {
                anyhow::bail!("{sender} did not receive command_suggestions ({cause})");
            }
            anyhow::bail!("{sender} did not receive command_suggestions");
        };
        Ok(Observed::Suggestions {
            start,
            length,
            suggestions,
            disconnects,
        })
    }

    async fn console<H: Host>(
        &mut self,
        lines: &[String],
        host: &mut H,
    ) -> anyhow::Result<Observed> {
        let output = host.console(lines)?;
        let mut chat = Vec::new();
        for event in self.settle(self.sink.quiet).await? {
            if let Signal::Chat(text) = event.signal {
                chat.push(ChatMessage {
                    client: event.client,
                    text,
                });
            }
        }
        Ok(Observed::Console { output, chat })
    }

    async fn send(&mut self, sender: &str, order: Order) -> anyhow::Result<Observed> {
        self.order_written(sender, order).await?;
        let events = self.settle(FIRST_PACKET).await?;
        collect_sent(&[sender], &events, Vec::new())
    }

    async fn sequence<H: Host>(
        &mut self,
        steps: &[Action],
        host: &mut H,
    ) -> anyhow::Result<Observed> {
        let mut senders = Vec::new();
        let mut console_output = Vec::new();
        for step in steps {
            if let Some((client, order)) = order_of(step) {
                self.order_written(client, order).await?;
                if !senders.contains(&client) {
                    senders.push(client);
                }
            } else {
                match step {
                    Action::Wait { ms } => time::sleep(Duration::from_millis(*ms)).await,
                    Action::Console { lines } => {
                        console_output.extend(host.console(lines)?);
                    }
                    Action::AwaitPacket { client, packet } => {
                        self.await_packet(client, packet).await?
                    }
                    Action::AdvanceTicks { ticks } => self.advance_ticks(host, *ticks).await?,
                    Action::AwaitPlayerTicks { client, ticks } => {
                        self.await_player_ticks(host, client, *ticks).await?;
                    }
                    _ => anyhow::bail!("unsupported action inside sequence"),
                }
            }
        }
        let events = self.settle(FIRST_PACKET).await?;
        collect_sent(&senders, &events, console_output)
    }

    async fn finish_checkpoint(&mut self) -> anyhow::Result<Observed> {
        let events = self.settle(self.sink.quiet).await?;
        collect_sent(&[], &events, Vec::new())
    }

    async fn await_packet(&mut self, client: &str, packet: &ExpectedPacket) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.sink.orders.contains_key(client),
            "{client} is not connected"
        );
        self.wait_for_evidence(client, packet).await?;
        self.sink.checkpoints.push(Checkpoint::Packet {
            client: client.to_owned(),
            packet: packet.clone(),
        });
        Ok(())
    }

    async fn wait_for_evidence(
        &mut self,
        client: &str,
        packet: &ExpectedPacket,
    ) -> anyhow::Result<()> {
        if let Some(index) = self
            .packet_evidence
            .iter()
            .position(|event| matches_packet(event, client, packet))
        {
            self.packet_evidence.remove(index);
            return Ok(());
        }
        let deadline = time::Instant::now() + FIRST_EVENT;
        while time::Instant::now() < deadline
            && let Ok(Some(event)) = time::timeout_at(deadline, self.events.recv()).await
        {
            let is_fresh = event.at >= self.step_start && event.at < deadline;
            let matches = is_fresh && matches_packet(&event, client, packet);
            match event.signal {
                Signal::Failed(error) => anyhow::bail!("{} session failed: {error}", event.client),
                Signal::Disconnected(_) | Signal::Closed => {
                    self.sink.orders.remove(&event.client);
                    if event.client == client {
                        anyhow::bail!("{client} disconnected while waiting for {packet:?}");
                    }
                    self.early_terminals.push(event);
                }
                other => {
                    if is_fresh
                        && !matches
                        && let Some(signal) = checkpoint_evidence(&other)
                    {
                        self.packet_evidence.push(Event {
                            client: event.client.clone(),
                            signal,
                            at: event.at,
                        });
                    }
                    self.sink.record_non_terminal(event.client, other, event.at);
                }
            }
            if matches {
                return Ok(());
            }
        }
        anyhow::bail!("{client} did not receive {packet:?}")
    }

    async fn await_player_ticks<H: Host>(
        &mut self,
        host: &mut H,
        client: &str,
        ticks: u32,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.sink.orders.contains_key(client),
            "{client} is not connected"
        );
        crate::player_ticks::wait_at_least(host, client, ticks).await?;
        self.sink.checkpoints.push(Checkpoint::PlayerTicks {
            client: client.to_owned(),
            at_least: ticks,
        });
        Ok(())
    }

    async fn advance_ticks<H: Host>(&mut self, host: &mut H, ticks: u32) -> anyhow::Result<()> {
        crate::frozen_ticks::advance(host, ticks).await?;
        self.sink.checkpoints.push(Checkpoint::GameTicks { ticks });
        Ok(())
    }

    async fn restart<H: Host>(&mut self, host: &mut H) -> anyhow::Result<Observed> {
        if !self.sink.orders.is_empty() {
            self.sink.orders.clear();
            self.settle(FIRST_EVENT).await?;
        }
        host.restart_server()?;
        Ok(Observed::Restarted)
    }

    async fn order_written(&mut self, name: &str, order: Order) -> anyhow::Result<()> {
        let write_wait = client::WRITE_LIMIT + Duration::from_secs(1);
        let deadline = time::Instant::now() + write_wait;
        let orders = self
            .sink
            .orders
            .get(name)
            .with_context(|| format!("{name} is not connected"))?;
        let (tx, rx) = oneshot::channel();
        if orders
            .send(QueuedOrder {
                order,
                deadline,
                written: Some(tx),
            })
            .is_err()
        {
            self.sink.orders.remove(name);
            anyhow::bail!("the connection of {name} ended");
        }
        match time::timeout_at(deadline, rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(error))) => {
                self.sink.orders.remove(name);
                anyhow::bail!("{name} packet write failed: {error}")
            }
            Ok(Err(_)) => {
                self.sink.orders.remove(name);
                anyhow::bail!("the connection of {name} ended before writing the packet")
            }
            Err(_) => {
                self.sink.orders.remove(name);
                anyhow::bail!("{name} packet write timed out after {write_wait:?}")
            }
        }
    }

    async fn settle(&mut self, first_limit: Duration) -> anyhow::Result<Vec<Event>> {
        let mut clock = LiveClock {
            events: &mut self.events,
        };
        let events = self
            .sink
            .settle_with_clock(first_limit, Duration::from_secs(10), &mut clock)
            .await?;
        let mut early = mem::take(&mut self.early_terminals);
        early.extend(events);
        Ok(early)
    }
}

fn matches_packet(event: &Event, client: &str, packet: &ExpectedPacket) -> bool {
    event.client == client
        && match (&event.signal, packet) {
            (Signal::Swing(swing), ExpectedPacket::Swing { subject }) => {
                &swing.observed.subject == subject
            }
            (Signal::BlockAck(received), ExpectedPacket::BlockAck { sequence }) => {
                received == sequence
            }
            (Signal::Added(added), ExpectedPacket::AddEntity { subject }) => added == subject,
            (Signal::Removed(removed), ExpectedPacket::RemoveEntity { subject }) => {
                removed.contains(subject)
            }
            (
                Signal::PositionAccepted(accepted),
                ExpectedPacket::PlayerPosition { block_center },
            ) => {
                let [x, y, z] = block_center.map(f64::from);
                *accepted == mcrs_minecraft_protocol_game::Position::new(x + 0.5, y, z + 0.5)
            }
            _ => false,
        }
}

fn checkpoint_evidence(signal: &Signal) -> Option<Signal> {
    Some(match signal {
        Signal::Swing(swing) => Signal::Swing(swing.clone()),
        Signal::BlockAck(sequence) => Signal::BlockAck(*sequence),
        Signal::Added(subject) => Signal::Added(subject.clone()),
        Signal::Removed(subjects) => Signal::Removed(subjects.clone()),
        Signal::PositionAccepted(position) => Signal::PositionAccepted(*position),
        _ => return None,
    })
}

impl SettleSink {
    fn in_sample(&self, at: time::Instant) -> bool {
        self.packet_sample_start.is_some_and(|start| at >= start)
    }

    fn record_non_terminal(&mut self, client: String, signal: Signal, at: time::Instant) {
        match signal {
            Signal::Menu(packet) => self.menu.push(MenuEvent { client, packet }),
            Signal::Sound(packet) => {
                if self.in_sample(at) {
                    self.packets.push(PacketObserved {
                        client,
                        packet: PacketKind::Sound,
                    });
                }
                self.sounds.push(packet);
            }
            Signal::Commands(tree) => self.trees.push(tree),
            Signal::Swing(packet) => {
                if self.in_sample(at) {
                    self.packets.push(PacketObserved {
                        client,
                        packet: PacketKind::SwingAnimation,
                    });
                }
                self.swings.push(packet);
            }
            Signal::Animate(packet) => self.animates.push(packet),
            Signal::Packet(packet) => {
                if self.in_sample(at) {
                    self.packets.push(PacketObserved { client, packet });
                }
            }
            Signal::Chat(_)
            | Signal::BlockAck(_)
            | Signal::Added(_)
            | Signal::Removed(_)
            | Signal::PositionAccepted(_)
            | Signal::Suggestions { .. }
            | Signal::Disconnected(_)
            | Signal::Closed
            | Signal::Failed(_) => {}
        }
    }

    async fn settle_with_clock<C: SettleClock>(
        &mut self,
        first_limit: Duration,
        extension: Duration,
        clock: &mut C,
    ) -> anyhow::Result<Vec<Event>> {
        let mut seen = Vec::new();
        let total_limit = first_limit
            .saturating_add(self.quiet)
            .saturating_add(extension);
        let start = clock.now();
        let mut idle_deadline = start + first_limit;
        let total_deadline = start + total_limit;
        let mut saw_event = false;
        loop {
            let now = clock.now();
            let stop_at = idle_deadline.min(total_deadline);
            let received = if self.quiet.is_zero() && now < total_deadline && stop_at <= now {
                clock.try_recv()
            } else if stop_at <= now {
                if saw_event && idle_deadline > total_deadline {
                    anyhow::bail!("settling did not become quiet within {total_limit:?}");
                }
                break;
            } else {
                clock.recv_before(stop_at).await
            };
            let event = match received {
                RecvOutcome::Event(event) => event,
                RecvOutcome::Closed => break,
                RecvOutcome::Timeout => {
                    if saw_event && idle_deadline > total_deadline {
                        anyhow::bail!("settling did not become quiet within {total_limit:?}");
                    }
                    break;
                }
            };
            if matches!(event.signal, Signal::Packet(_)) && !self.in_sample(event.at) {
                continue;
            }
            if matches!(
                event.signal,
                Signal::BlockAck(_)
                    | Signal::Added(_)
                    | Signal::Removed(_)
                    | Signal::PositionAccepted(_)
            ) {
                continue;
            }
            saw_event = true;
            idle_deadline = clock.now() + self.quiet;
            match event.signal {
                Signal::Failed(error) => {
                    self.orders.remove(&event.client);
                    anyhow::bail!("{} session failed: {error}", event.client);
                }
                Signal::Disconnected(_) | Signal::Closed => {
                    self.orders.remove(&event.client);
                    seen.push(event);
                }
                Signal::Chat(_) | Signal::Suggestions { .. } => seen.push(event),
                other => self.record_non_terminal(event.client, other, event.at),
            }
        }
        Ok(seen)
    }
}

fn collect_players(scenario: &Scenario) -> Vec<String> {
    let mut players = Vec::new();
    for step in &scenario.steps {
        let name = match &step.action {
            Action::Join(name) | Action::JoinDeferred(name) => Some(name.clone()),
            _ => None,
        };
        if let Some(name) = name
            && !players.contains(&name)
        {
            players.push(name);
        }
    }
    players
}

fn order_of(action: &Action) -> Option<(&str, Order)> {
    match action {
        Action::SendLoaded(name) => Some((name.as_str(), Order::SendLoaded)),
        Action::SyncPosition(name) => Some((name.as_str(), Order::SyncPosition)),
        Action::Click(click) => Some((click.client.as_str(), Order::Click(click.clone()))),
        Action::Close { client, container } => Some((client.as_str(), Order::Close(*container))),
        Action::SelectSlot { client, slot } => Some((client.as_str(), Order::SelectSlot(*slot))),
        Action::PlayerAction {
            client,
            status,
            position,
            face,
        } => Some((
            client.as_str(),
            Order::PlayerAction {
                action: (*status).into(),
                position: position.unwrap_or([0, 0, 0]),
                face: face.unwrap_or(Direction::Down),
            },
        )),
        Action::CreativeSlot {
            client,
            slot,
            stack,
        } => Some((
            client.as_str(),
            Order::CreativeSlot {
                slot: *slot,
                stack: *stack,
            },
        )),
        Action::UseBlock { client, position } => {
            Some((client.as_str(), Order::UseBlock(*position)))
        }
        Action::Punch { client } => Some((client.as_str(), Order::Punch)),
        Action::PunchBurst { client, count } => Some((client.as_str(), Order::PunchBurst(*count))),
        Action::UseItem {
            client,
            hand,
            sequence,
        } => Some((
            client.as_str(),
            Order::UseItem {
                hand: (*hand).into(),
                sequence: *sequence,
            },
        )),
        Action::UseItemOn {
            client,
            hand,
            position,
            face,
            sequence,
        } => Some((
            client.as_str(),
            Order::UseItemOn {
                hand: (*hand).into(),
                position: *position,
                face: *face,
                sequence: *sequence,
            },
        )),
        Action::Attack { client, target } => Some((client.as_str(), Order::Attack(target.clone()))),
        Action::Interact {
            client,
            target,
            hand,
            secondary,
        } => Some((
            client.as_str(),
            Order::Interact {
                target: target.clone(),
                hand: (*hand).into(),
                secondary: *secondary,
            },
        )),
        Action::ClientMainArm { client, arm } => {
            Some((client.as_str(), Order::ClientMainArm((*arm).into())))
        }
        Action::PunchExtraByte { client } => Some((client.as_str(), Order::PunchExtraByte)),
        Action::Join(_)
        | Action::JoinDeferred(_)
        | Action::Leave(_)
        | Action::Command { .. }
        | Action::Suggest { .. }
        | Action::Console { .. }
        | Action::Wait { .. }
        | Action::AwaitPacket { .. }
        | Action::AdvanceTicks { .. }
        | Action::AwaitPlayerTicks { .. }
        | Action::Sequence { .. }
        | Action::RestartServer {} => None,
    }
}

fn collect_sent(
    senders: &[&str],
    events: &[Event],
    console_output: Vec<String>,
) -> anyhow::Result<Observed> {
    let mut disconnects = Vec::new();
    for event in events {
        match &event.signal {
            Signal::Disconnected(reason) => disconnects.push(Disconnect {
                client: event.client.clone(),
                reason: reason.clone(),
            }),
            Signal::Closed if senders.contains(&event.client.as_str()) => {
                anyhow::bail!(
                    "the connection of {} closed without a disconnect packet",
                    event.client
                )
            }
            Signal::Failed(error) if senders.contains(&event.client.as_str()) => {
                anyhow::bail!(
                    "the connection of {} closed after an error: {error}",
                    event.client
                )
            }
            Signal::Chat(_)
            | Signal::Menu(_)
            | Signal::Sound(_)
            | Signal::Commands(_)
            | Signal::Suggestions { .. }
            | Signal::Swing(_)
            | Signal::BlockAck(_)
            | Signal::Added(_)
            | Signal::Removed(_)
            | Signal::PositionAccepted(_)
            | Signal::Animate(_)
            | Signal::Packet(_)
            | Signal::Closed
            | Signal::Failed(_) => {}
        }
    }
    disconnects.sort_by(|left, right| left.client.cmp(&right.client));
    Ok(Observed::Sent {
        disconnects,
        console_output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProbeHand, SwingAnimationKind};
    use mcrs_minecraft_protocol_game::Position;
    use mcrs_minecraft_text::Text;

    #[tokio::test]
    async fn checkpoints_keep_early_packets_and_unrelated_disconnects() {
        let mut clients = Clients::new(true, Vec::new(), Duration::ZERO, Observe::default());
        let (bob_orders, _bob) = mpsc::unbounded_channel();
        let (charlie_orders, _charlie) = mpsc::unbounded_channel();
        let (dan_orders, _dan) = mpsc::unbounded_channel();
        clients.sink.orders.insert("Bob".to_owned(), bob_orders);
        clients
            .sink
            .orders
            .insert("Charlie".to_owned(), charlie_orders);
        clients.sink.orders.insert("Dan".to_owned(), dan_orders);
        let arrival = [
            Event::now(
                "Bob".to_owned(),
                Signal::Swing(entry("Bob", "Alice", 1, 6, 1)),
            ),
            Event::now(
                "Charlie".to_owned(),
                Signal::PositionAccepted(Position::new(600.5, 100.0, 0.5)),
            ),
            Event::now(
                "Dan".to_owned(),
                Signal::Disconnected(Text::text("server disconnect")),
            ),
            Event::now(
                "Charlie".to_owned(),
                Signal::PositionAccepted(Position::new(4.5, 100.0, 0.5)),
            ),
            Event::now(
                "Charlie".to_owned(),
                Signal::Swing(entry("Charlie", "Alice", 1, 6, 1)),
            ),
        ];
        for event in arrival {
            clients.events_in.send(event).unwrap();
        }
        let position = ExpectedPacket::PlayerPosition {
            block_center: [4, 100, 0],
        };
        clients.await_packet("Charlie", &position).await.unwrap();
        assert!(
            !clients.sink.orders.contains_key("Dan"),
            "the stale position cannot finish the wait before Dan's disconnect"
        );
        let swing = ExpectedPacket::Swing {
            subject: "Alice".to_owned(),
        };
        clients.await_packet("Bob", &swing).await.unwrap();
        let observed = clients.finish_checkpoint().await.unwrap();
        assert_eq!(
            observed,
            Observed::Sent {
                disconnects: vec![Disconnect {
                    client: "Dan".to_owned(),
                    reason: Text::text("server disconnect")
                }],
                console_output: Vec::new(),
            }
        );
        assert_eq!(
            clients
                .sink
                .swings
                .iter()
                .map(|swing| swing.observed.client.as_str())
                .collect::<Vec<_>>(),
            ["Bob", "Charlie"],
            "evidence is recorded once and settling records the later observer"
        );
    }

    fn entry(
        client: &str,
        subject: &str,
        entity_id: u8,
        duration: i32,
        raw_byte: u8,
    ) -> CapturedSwing {
        CapturedSwing {
            observed: SwingObserved {
                client: client.to_owned(),
                subject: subject.to_owned(),
                hand: ProbeHand::Main,
                animation: SwingAnimationKind::Whack,
                duration,
            },
            entity_id: i32::from(entity_id),
            canonical: true,
            raw_body: vec![entity_id, 0, 1, raw_byte],
        }
    }

    #[test]
    fn compute_swing_bytes_groups_by_entity_id_and_detects_mismatch() {
        let distinct_pigs = [
            entry("Bob", "minecraft:pig", 11, 6, 6),
            entry("Charlie", "minecraft:pig", 12, 6, 6),
        ];
        assert_eq!(
            compute_swing_bytes(&distinct_pigs),
            Some(SwingWireCheck {
                canonical: true,
                shared_across_recipients: None,
            })
        );

        let mismatched_durations = [
            entry("Bob", "Alice", 1, 6, 6),
            entry("Charlie", "Alice", 1, 7, 7),
        ];
        assert_eq!(
            compute_swing_bytes(&mismatched_durations),
            Some(SwingWireCheck {
                canonical: true,
                shared_across_recipients: Some(false),
            })
        );

        let identical = [
            entry("Bob", "Alice", 1, 6, 6),
            entry("Charlie", "Alice", 1, 6, 6),
        ];
        assert_eq!(
            compute_swing_bytes(&identical),
            Some(SwingWireCheck {
                canonical: true,
                shared_across_recipients: Some(true),
            })
        );
    }

    struct ScriptedClock {
        base: time::Instant,
        offset: Duration,
        queue: std::collections::VecDeque<(Duration, Event)>,
        repeat: Option<(Duration, String, fn() -> Signal)>,
        delivered: usize,
    }

    impl SettleClock for ScriptedClock {
        fn now(&self) -> time::Instant {
            self.base + self.offset
        }

        fn try_recv(&mut self) -> RecvOutcome {
            if self.queue.front().is_some_and(|(at, _)| *at <= self.offset)
                && let Some((_, event)) = self.queue.pop_front()
            {
                self.delivered += 1;
                return RecvOutcome::Event(event);
            }
            RecvOutcome::Timeout
        }

        async fn recv_before(&mut self, deadline: time::Instant) -> RecvOutcome {
            if let Some(at_offset) = self.queue.front().map(|(at, _)| *at) {
                let event_time = self.base + at_offset;
                if event_time <= deadline {
                    let (_, event) = self.queue.pop_front().unwrap();
                    self.offset = at_offset;
                    self.delivered += 1;
                    return RecvOutcome::Event(event);
                }
            }
            if let Some((step, client, build_signal)) = &self.repeat {
                let next_offset = self.offset + *step;
                let event_time = self.base + next_offset;
                if event_time <= deadline {
                    self.offset = next_offset;
                    self.delivered += 1;
                    return RecvOutcome::Event(Event {
                        client: client.clone(),
                        signal: build_signal(),
                        at: event_time,
                    });
                }
            }
            let deadline_offset = deadline.saturating_duration_since(self.base);
            self.offset = self.offset.max(deadline_offset);
            RecvOutcome::Timeout
        }
    }

    fn block_update_signal() -> Signal {
        Signal::Packet(PacketKind::BlockUpdate)
    }

    #[tokio::test]
    async fn zero_quiet_window_drains_ready_packets() {
        let (sender, mut events) = mpsc::unbounded_channel();
        let mut sink = SettleSink::new(
            Duration::ZERO,
            Observe {
                packet_order: true,
                ..Observe::default()
            },
        );
        sink.packet_sample_start = Some(time::Instant::now());
        for client in ["Bob", "Charlie"] {
            sender
                .send(Event::now(client.to_owned(), block_update_signal()))
                .unwrap();
        }
        sink.settle_with_clock(
            Duration::ZERO,
            FIRST_EVENT,
            &mut LiveClock {
                events: &mut events,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            sink.packets.len(),
            2,
            "both packets were ready before settling"
        );
    }

    #[tokio::test]
    async fn settle_ignores_unmarked_flood_excludes_pre_boundary_and_bounds_marked_flood() {
        let base = time::Instant::now();
        let mut sink = SettleSink::new(
            Duration::from_millis(25),
            Observe {
                packet_order: true,
                ..Observe::default()
            },
        );

        let mut unmarked_clock = ScriptedClock {
            base,
            offset: Duration::ZERO,
            queue: std::collections::VecDeque::new(),
            repeat: Some((
                Duration::from_millis(5),
                "Alice".to_owned(),
                block_update_signal,
            )),
            delivered: 0,
        };
        sink.settle_with_clock(
            Duration::from_millis(25),
            Duration::from_millis(80),
            &mut unmarked_clock,
        )
        .await
        .unwrap();
        assert!(unmarked_clock.delivered >= 4);
        assert!(sink.packets.is_empty());

        let sample_start = unmarked_clock.now() + Duration::from_millis(10);
        sink.packet_sample_start = Some(sample_start);
        let mut boundary_clock = ScriptedClock {
            base,
            offset: unmarked_clock.offset,
            queue: std::collections::VecDeque::from([
                (
                    unmarked_clock.offset + Duration::from_millis(2),
                    Event {
                        client: "Bob".to_owned(),
                        signal: Signal::Packet(PacketKind::BlockUpdate),
                        at: unmarked_clock.now() + Duration::from_millis(2),
                    },
                ),
                (
                    unmarked_clock.offset + Duration::from_millis(15),
                    Event {
                        client: "Bob".to_owned(),
                        signal: Signal::Packet(PacketKind::Sound),
                        at: sample_start + Duration::from_millis(5),
                    },
                ),
            ]),
            repeat: None,
            delivered: 0,
        };
        sink.settle_with_clock(
            Duration::from_millis(25),
            Duration::from_millis(80),
            &mut boundary_clock,
        )
        .await
        .unwrap();
        assert_eq!(sink.packets.len(), 1);
        assert_eq!(sink.packets[0].packet, PacketKind::Sound);

        let mut marked_clock = ScriptedClock {
            base,
            offset: boundary_clock.offset,
            queue: std::collections::VecDeque::new(),
            repeat: Some((
                Duration::from_millis(5),
                "Alice".to_owned(),
                block_update_signal,
            )),
            delivered: 0,
        };
        sink.packet_sample_start = Some(marked_clock.now());
        let Err(error) = sink
            .settle_with_clock(
                Duration::from_millis(25),
                Duration::from_millis(80),
                &mut marked_clock,
            )
            .await
        else {
            panic!("expected settle deadline error on marked packet flood");
        };
        assert!(marked_clock.delivered >= 20);
        let err = error.to_string();
        assert!(err.contains("settling did not become quiet"), "{err}");
    }

    #[tokio::test]
    async fn settle_honors_caller_quiet_budget_and_propagates_observer_failure() {
        let base = time::Instant::now();
        let mut quiet_budget_sink = SettleSink::new(Duration::from_secs(15), Observe::default());
        let mut quiet_clock = ScriptedClock {
            base,
            offset: Duration::ZERO,
            queue: std::collections::VecDeque::from([(
                Duration::from_millis(500),
                Event {
                    client: "Alice".to_owned(),
                    signal: Signal::Chat(mcrs_minecraft_text::Text::text("ok")),
                    at: base + Duration::from_millis(500),
                },
            )]),
            repeat: None,
            delivered: 0,
        };
        let seen = quiet_budget_sink
            .settle_with_clock(
                Duration::from_secs(2),
                Duration::from_secs(10),
                &mut quiet_clock,
            )
            .await
            .unwrap();
        assert_eq!(seen.len(), 1);

        let mut observer_fail_sink = SettleSink::new(Duration::from_millis(25), Observe::default());
        let mut fail_clock = ScriptedClock {
            base,
            offset: Duration::ZERO,
            queue: std::collections::VecDeque::from([(
                Duration::from_millis(5),
                Event {
                    client: "Bob".to_owned(),
                    signal: Signal::Failed("trailing bytes".to_owned()),
                    at: base + Duration::from_millis(5),
                },
            )]),
            repeat: None,
            delivered: 0,
        };
        let Err(fail_err) = observer_fail_sink
            .settle_with_clock(
                Duration::from_millis(25),
                Duration::from_millis(80),
                &mut fail_clock,
            )
            .await
        else {
            panic!("expected observer failure to abort settle");
        };
        assert!(fail_err.to_string().contains("Bob"), "{fail_err}");
    }
}
