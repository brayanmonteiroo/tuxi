//! Lista de janelas via `zwlr_foreign_toplevel_management_v1` (taskbar).
//!
//! Corre numa thread com ligação Wayland própria (além do GTK) e notifica
//! a UI GTK via canal.

use std::collections::HashMap;
use std::os::fd::{AsFd, AsRawFd};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use anyhow::Context;
use tracing::{debug, info, warn};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{
    event_created_child, Connection, Dispatch, EventQueue, QueueHandle,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

/// Snapshot de uma janela para a taskbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskInfo {
    pub id: u32,
    pub title: String,
    pub app_id: String,
    pub active: bool,
}

#[derive(Debug)]
pub(crate) enum TaskCommand {
    Activate(u32),
    Close(u32),
}

#[derive(Debug)]
enum WlMsg {
    Snapshot(Vec<TaskInfo>),
}

struct ToplevelData {
    title: String,
    app_id: String,
    active: bool,
    closed: bool,
}

struct State {
    seats: Vec<wl_seat::WlSeat>,
    /// Mantido vivo — dropar cancela o protocolo.
    manager: Option<ZwlrForeignToplevelManagerV1>,
    toplevels: HashMap<u32, (ZwlrForeignToplevelHandleV1, ToplevelData)>,
    next_id: u32,
    dirty: bool,
}

impl State {
    fn snapshot(&self) -> Vec<TaskInfo> {
        let mut out: Vec<TaskInfo> = self
            .toplevels
            .iter()
            .filter(|(_, (_, d))| !d.closed && !should_hide_task(&d.app_id, &d.title))
            .map(|(id, (_, d))| TaskInfo {
                id: *id,
                title: display_title(&d.title, &d.app_id),
                app_id: d.app_id.clone(),
                active: d.active,
            })
            .collect();
        out.sort_by_key(|a| a.title.to_lowercase());
        out
    }
}

fn should_hide_task(app_id: &str, title: &str) -> bool {
    is_tuxi_chrome(app_id, title) || is_kde_host_leak(app_id, title)
}

fn is_tuxi_chrome(app_id: &str, title: &str) -> bool {
    let app = app_id.to_ascii_lowercase();
    if app.contains("org.tuxi.panel") || app == "tuxi-panel" {
        return true;
    }
    let t = title.trim();
    t.eq_ignore_ascii_case("TUXI Panel") || t.eq_ignore_ascii_case("TUXI Menu")
}

/// Diálogos KDE (KWallet / atalhos) que vazam para o nested via D-Bus.
fn is_kde_host_leak(app_id: &str, title: &str) -> bool {
    let app = app_id.to_ascii_lowercase();
    if app.contains("kwallet")
        || app.contains("kglobalaccel")
        || app.contains("kded")
        || app.contains("polkit")
        || app.contains("secretprompter")
        || app.contains("ksecret")
    {
        return true;
    }
    let t = title.trim();
    t.contains("Carteiras do KDE")
        || t.contains("Atalhos globais")
        || t.contains("KDE Wallet")
        || t.contains("kwallet")
}

fn display_title(title: &str, app_id: &str) -> String {
    let t = title.trim();
    if !t.is_empty() {
        return t.to_string();
    }
    if !app_id.is_empty() {
        return app_id
            .trim_end_matches(".desktop")
            .rsplit('.')
            .next()
            .unwrap_or(app_id)
            .to_string();
    }
    "Janela".to_string()
}

/// Arranca o watcher. Devolve o tx para comandos (activate/close).
pub(crate) fn spawn_task_watcher(
    on_update: impl Fn(Vec<TaskInfo>) + Send + 'static,
) -> Sender<TaskCommand> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<TaskCommand>();
    let (wl_tx, wl_rx) = mpsc::channel::<WlMsg>();

    glib_attach_updates(wl_rx, on_update);

    thread::Builder::new()
        .name("tuxi-foreign-toplevel".into())
        .spawn(move || {
            if let Err(err) = run_wayland_loop(cmd_rx, wl_tx) {
                warn!(?err, "foreign-toplevel: watcher terminou");
            }
        })
        .expect("spawn foreign-toplevel thread");

    cmd_tx
}

fn glib_attach_updates(
    rx: Receiver<WlMsg>,
    on_update: impl Fn(Vec<TaskInfo>) + Send + 'static,
) {
    use relm4::gtk::glib;

    glib::timeout_add_local(Duration::from_millis(33), move || {
        let mut last = None;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                WlMsg::Snapshot(tasks) => last = Some(tasks),
            }
        }
        if let Some(tasks) = last {
            on_update(tasks);
        }
        glib::ControlFlow::Continue
    });
}

fn run_wayland_loop(cmd_rx: Receiver<TaskCommand>, wl_tx: Sender<WlMsg>) -> anyhow::Result<()> {
    let conn = Connection::connect_to_env().context("ligar ao Wayland")?;
    let display = conn.display();
    let mut event_queue: EventQueue<State> = conn.new_event_queue();
    let qh = event_queue.handle();

    let mut state = State {
        seats: Vec::new(),
        manager: None,
        toplevels: HashMap::new(),
        next_id: 1,
        dirty: false,
    };

    display.get_registry(&qh, ());
    event_queue.roundtrip(&mut state)?;
    // Segundo roundtrip: após bind do manager o compositor envia os toplevels.
    event_queue.roundtrip(&mut state)?;
    flush_dirty(&mut state, &wl_tx);

    info!(
        seats = state.seats.len(),
        has_manager = state.manager.is_some(),
        toplevels = state.toplevels.len(),
        "foreign-toplevel: ligado ao compositor"
    );

    if state.manager.is_none() {
        anyhow::bail!(
            "compositor sem zwlr_foreign_toplevel_manager_v1 (taskbar indisponível)"
        );
    }

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            handle_command(&mut state, cmd);
            conn.flush()?;
        }

        event_queue.dispatch_pending(&mut state)?;
        flush_dirty(&mut state, &wl_tx);

        conn.flush()?;

        match conn.prepare_read() {
            Some(guard) => {
                let fd = conn.as_fd();
                let mut fds = [libc::pollfd {
                    fd: fd.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                }];
                let ret = unsafe { libc::poll(fds.as_mut_ptr(), 1, 50) };
                if ret > 0 {
                    let _ = guard.read();
                } else {
                    drop(guard);
                }
            }
            None => {
                thread::sleep(Duration::from_millis(10));
            }
        }

        event_queue.dispatch_pending(&mut state)?;
        flush_dirty(&mut state, &wl_tx);
    }
}

fn flush_dirty(state: &mut State, wl_tx: &Sender<WlMsg>) {
    if !state.dirty {
        return;
    }
    // Fecha de imediato vazamentos KDE (carteira / atalhos) — não só esconder.
    let leak_ids: Vec<u32> = state
        .toplevels
        .iter()
        .filter(|(_, (_, d))| !d.closed && is_kde_host_leak(&d.app_id, &d.title))
        .map(|(id, _)| *id)
        .collect();
    for id in leak_ids {
        if let Some((handle, data)) = state.toplevels.get_mut(&id) {
            debug!(
                id,
                app_id = %data.app_id,
                title = %data.title,
                "foreign-toplevel: fechar vazamento KDE"
            );
            handle.close();
            data.closed = true;
        }
    }

    let snap = state.snapshot();
    let _ = wl_tx.send(WlMsg::Snapshot(snap));
    state.dirty = false;
}

fn handle_command(state: &mut State, cmd: TaskCommand) {
    match cmd {
        TaskCommand::Activate(id) => {
            let Some(seat) = state.seats.first() else {
                warn!("foreign-toplevel: sem wl_seat para activate");
                return;
            };
            if let Some((handle, _)) = state.toplevels.get(&id) {
                handle.activate(seat);
                debug!(id, "activate");
            }
        }
        TaskCommand::Close(id) => {
            if let Some((handle, _)) = state.toplevels.get(&id) {
                handle.close();
                debug!(id, "close");
            }
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_seat" => {
                    let seat =
                        registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(1), qh, ());
                    state.seats.push(seat);
                }
                "zwlr_foreign_toplevel_manager_v1" if state.manager.is_none() => {
                    state.manager = Some(registry.bind::<ZwlrForeignToplevelManagerV1, _, _>(
                        name,
                        version.min(3),
                        qh,
                        (),
                    ));
                    debug!("foreign-toplevel manager bound");
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } => {
                let id = state.next_id;
                state.next_id = state.next_id.wrapping_add(1).max(1);
                debug!(id, "foreign-toplevel: nova janela");
                state.toplevels.insert(
                    id,
                    (
                        toplevel,
                        ToplevelData {
                            title: String::new(),
                            app_id: String::new(),
                            active: false,
                            closed: false,
                        },
                    ),
                );
                state.dirty = true;
            }
            zwlr_foreign_toplevel_manager_v1::Event::Finished => {
                warn!("foreign-toplevel manager finished");
            }
            _ => {}
        }
    }

    // Obrigatório: evento `toplevel` cria um new_id (sem isto a thread panica).
    event_created_child!(State, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some((_id, data)) = state
            .toplevels
            .iter_mut()
            .find(|(_, (h, _))| h == handle)
            .map(|(id, (_, d))| (*id, d))
        else {
            return;
        };

        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => {
                data.title = title;
                state.dirty = true;
            }
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                data.app_id = app_id;
                state.dirty = true;
            }
            zwlr_foreign_toplevel_handle_v1::Event::State { state: st } => {
                data.active = parse_activated(&st);
                state.dirty = true;
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                state.dirty = true;
            }
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                data.closed = true;
                state.dirty = true;
            }
            _ => {}
        }
    }
}

fn parse_activated(state_bytes: &[u8]) -> bool {
    state_bytes.chunks_exact(4).any(|c| {
        let v = u32::from_ne_bytes([c[0], c[1], c[2], c[3]]);
        // zwlr_foreign_toplevel_handle_v1.state activated = 2
        v == 2
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hide_tuxi_panel() {
        assert!(should_hide_task("org.tuxi.Panel", "x"));
        assert!(should_hide_task("", "TUXI Menu"));
        assert!(should_hide_task("org.kde.kwalletd6", "x"));
        assert!(should_hide_task("", "Serviço de Carteiras do KDE"));
        assert!(should_hide_task("", "Atalhos globais solicitados"));
        assert!(is_kde_host_leak("", "Atalhos globais solicitados"));
        assert!(!is_kde_host_leak("", "TUXI Menu"));
        assert!(!should_hide_task("bruno", "Bruno"));
        assert!(!should_hide_task("", "")); // ainda sem title/app_id — mostrar como "Janela"
    }

    #[test]
    fn display_title_fallback() {
        assert_eq!(display_title("Bruno", "x"), "Bruno");
        assert_eq!(display_title("", "org.foo.Bar.desktop"), "Bar");
    }
}
