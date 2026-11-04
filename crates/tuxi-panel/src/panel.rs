//! Painel inferior Wayland (layer-shell) e shell do menu iniciar.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use chrono::Local;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::gdk::{self, Key, ModifierType};
use relm4::gtk::glib;
use relm4::gtk::{self, EventControllerKey, Orientation};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use tracing::{error, info, warn};

use crate::foreign_toplevel::{self, TaskCommand, TaskInfo};
use crate::session_power::{is_nested_session, PowerAction};
use crate::start_menu::{
    filter_apps, launch_by_id, load_apps_by_category, AppCategory, DesktopApp,
};
use std::sync::mpsc::Sender;

/// Altura da barra (proporção ~task manager do Plasma; só tamanho, não o visual).
const PANEL_HEIGHT: i32 = 48;
/// Ícone da tarefa — preenche o botão (pouco padding à volta).
const TASK_ICON_PX: i32 = 36;
/// Folga entre a borda inferior do menu e o topo da barra (px).
const MENU_GAP_ABOVE_PANEL: i32 = 2;
const MENU_WIDTH: i32 = 720;
const MENU_HEIGHT: i32 = 560;

#[derive(Debug)]
pub(crate) enum PanelMsg {
    Tick,
    ToggleMenu,
    SearchChanged(String),
    SelectCategory(AppCategory),
    Launch(String),
    Power(PowerAction),
    MenuClosed,
    /// Pedido externo (ex.: RMB na área de trabalho) para fechar o menu.
    DismissMenu,
    TasksUpdated(Vec<TaskInfo>),
    TaskActivate(u32),
    TaskClose(u32),
}

pub(crate) struct PanelModel {
    clock_text: String,
    search: String,
    selected_category: Option<AppCategory>,
    apps: BTreeMap<AppCategory, Vec<DesktopApp>>,
    app_list: Option<gtk::ListBox>,
    menu_button: Option<gtk::ToggleButton>,
    search_entry: Option<gtk::Entry>,
    menu_window: Option<gtk::Window>,
    task_area: Option<gtk::Box>,
    tasks: Vec<TaskInfo>,
    task_cmd: Option<Sender<TaskCommand>>,
}

#[relm4::component(pub)]
impl SimpleComponent for PanelModel {
    type Init = ();
    type Input = PanelMsg;
    type Output = ();

    view! {
        #[root]
        gtk::Window {
            set_title: Some("TUXI Panel"),
            set_decorated: false,
            // resizable=false impede o stretch do layer-shell no labwc nested
            set_resizable: true,
            set_default_height: PANEL_HEIGHT,
            add_css_class: "tuxi-panel",

            // Menu | tarefas… | relógio (sem CenterBox — evitava o buraco no meio).
            gtk::Box {
                set_orientation: Orientation::Horizontal,
                set_spacing: 8,
                set_hexpand: true,
                set_vexpand: true,
                set_margin_start: 6,
                set_margin_end: 6,
                set_valign: gtk::Align::Fill,

                #[name = "menu_button"]
                gtk::ToggleButton {
                    set_label: "Menu",
                    set_tooltip_text: Some("Menu iniciar"),
                    set_valign: gtk::Align::Center,
                    connect_clicked => PanelMsg::ToggleMenu,
                },

                #[name = "task_area"]
                gtk::Box {
                    set_orientation: Orientation::Horizontal,
                    set_spacing: 4,
                    set_hexpand: true,
                    set_halign: gtk::Align::Start,
                    set_valign: gtk::Align::Center,
                    add_css_class: "tuxi-task-area",
                },

                gtk::Label {
                    add_css_class: "tuxi-panel-clock",
                    set_valign: gtk::Align::Center,
                    #[watch]
                    set_label: &model.clock_text,
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        if let Err(err) = crate::style::load_tuxi_stylesheet() {
            warn!(?err, "não foi possível carregar o CSS TUXI");
        }

        let apps = load_apps_by_category();
        let selected_category = AppCategory::ALL
            .into_iter()
            .find(|c| apps.get(c).is_some_and(|v| !v.is_empty()));

        let menu = build_menu_window(sender.clone());

        let mut model = PanelModel {
            clock_text: format_clock(),
            search: String::new(),
            selected_category,
            apps,
            app_list: Some(menu.app_list.clone()),
            menu_button: None,
            search_entry: Some(menu.search_entry.clone()),
            menu_window: Some(menu.window.clone()),
            task_area: None,
            tasks: Vec::new(),
            task_cmd: None,
        };

        let widgets = view_output!();
        model.menu_button = Some(widgets.menu_button.clone());
        model.task_area = Some(widgets.task_area.clone());
        rebuild_task_area(&widgets.task_area, &model.tasks, sender.clone());

        // Layer-shell DEPOIS dos filhos, ANTES do present (Relm4).
        // Nunca fixar largura com size_request — isso impede o stretch L+R.
        configure_panel_layer_shell(&root);

        let task_sender = sender.clone();
        let task_cmd = foreign_toplevel::spawn_task_watcher(move |tasks| {
            task_sender.input(PanelMsg::TasksUpdated(tasks));
        });
        model.task_cmd = Some(task_cmd);

        rebuild_category_list(&menu.category_list, &model.apps, model.selected_category);
        wire_category_list(&menu.category_list, sender.clone());
        rebuild_app_pane(
            &menu.app_list,
            &model.apps,
            model.selected_category,
            &model.search,
            sender.clone(),
        );

        {
            let win = root.clone();
            root.connect_realize(move |w| {
                apply_panel_full_width(w);
                info!(
                    is_layer = w.is_layer_window(),
                    anchor_left = w.is_anchor(Edge::Left),
                    anchor_right = w.is_anchor(Edge::Right),
                    "painel realize"
                );
            });
            root.connect_map(move |w| {
                apply_panel_full_width(w);
                let win2 = win.clone();
                let win3 = win.clone();
                if let Some(surface) = w.surface() {
                    surface.connect_enter_monitor(move |_, _| {
                        apply_panel_full_width(&win2);
                    });
                }
                // Retry: monitor/geometry no nested às vezes só existe após 1 frame.
                glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
                    apply_panel_full_width(&win3);
                });
            });
        }

        install_super_shortcut(&root, sender.clone());
        install_menu_escape(&menu.window, sender.clone());

        let tick_sender = sender.clone();
        glib::timeout_add_seconds_local(1, move || {
            tick_sender.input(PanelMsg::Tick);
            glib::ControlFlow::Continue
        });

        // Área de trabalho (e outros) pedem fechar o menu via ficheiro em runtime.
        let dismiss_sender = sender.clone();
        glib::timeout_add_local(Duration::from_millis(120), move || {
            if take_dismiss_menu_signal() {
                dismiss_sender.input(PanelMsg::DismissMenu);
            }
            glib::ControlFlow::Continue
        });

        info!(
            apps = model.apps.values().map(Vec::len).sum::<usize>(),
            "painel TUXI iniciado"
        );

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            PanelMsg::Tick => {
                self.clock_text = format_clock();
            }
            PanelMsg::ToggleMenu => {
                let Some(window) = &self.menu_window else {
                    return;
                };
                if window.is_visible() {
                    hide_menu(self);
                    sender.input(PanelMsg::MenuClosed);
                } else {
                    show_menu(self);
                }
            }
            PanelMsg::DismissMenu => {
                if self.menu_window.as_ref().is_some_and(|w| w.is_visible()) {
                    hide_menu(self);
                    sender.input(PanelMsg::MenuClosed);
                }
            }
            PanelMsg::MenuClosed => {
                if let Some(button) = &self.menu_button {
                    button.set_active(false);
                }
                let mut dirty = false;
                if !self.search.is_empty() {
                    self.search.clear();
                    if let Some(entry) = &self.search_entry {
                        entry.set_text("");
                    }
                    dirty = true;
                }
                if dirty {
                    if let Some(list) = &self.app_list {
                        rebuild_app_pane(
                            list,
                            &self.apps,
                            self.selected_category,
                            &self.search,
                            sender,
                        );
                    }
                }
            }
            PanelMsg::SearchChanged(query) => {
                if self.search != query {
                    self.search = query;
                    if let Some(list) = &self.app_list {
                        rebuild_app_pane(
                            list,
                            &self.apps,
                            self.selected_category,
                            &self.search,
                            sender,
                        );
                    }
                }
            }
            PanelMsg::SelectCategory(category) => {
                self.selected_category = Some(category);
                if !self.search.is_empty() {
                    self.search.clear();
                    if let Some(entry) = &self.search_entry {
                        entry.set_text("");
                    }
                }
                if let Some(list) = &self.app_list {
                    rebuild_app_pane(
                        list,
                        &self.apps,
                        self.selected_category,
                        &self.search,
                        sender,
                    );
                }
            }
            PanelMsg::Launch(id) => {
                if let Err(err) = launch_by_id(&self.apps, &id) {
                    error!(?err, app = %id, "não foi possível iniciar a aplicação");
                    let parent = self
                        .menu_window
                        .as_ref()
                        .map(|w| w.clone().upcast::<gtk::Window>());
                    show_alert_dialog(
                        parent.as_ref(),
                        &format!("Não foi possível abrir a aplicação:\n{err}"),
                    );
                }
                hide_menu(self);
                sender.input(PanelMsg::MenuClosed);
            }
            PanelMsg::Power(action) => {
                hide_menu(self);
                if let Some(button) = &self.menu_button {
                    button.set_active(false);
                }
                if let Err(err) = action.run() {
                    error!(
                        ?err,
                        action = action.label(),
                        "falha na ação de energia/sessão"
                    );
                    let parent = self
                        .menu_window
                        .as_ref()
                        .map(|w| w.clone().upcast::<gtk::Window>());
                    show_alert_dialog(parent.as_ref(), &err.to_string());
                }
            }
            PanelMsg::TasksUpdated(tasks) => {
                self.tasks = tasks;
                if let Some(area) = &self.task_area {
                    rebuild_task_area(area, &self.tasks, sender);
                }
            }
            PanelMsg::TaskActivate(id) => {
                if let Some(tx) = &self.task_cmd {
                    let _ = tx.send(TaskCommand::Activate(id));
                }
            }
            PanelMsg::TaskClose(id) => {
                if let Some(tx) = &self.task_cmd {
                    let _ = tx.send(TaskCommand::Close(id));
                }
            }
        }
    }
}

struct MenuWidgets {
    window: gtk::Window,
    search_entry: gtk::Entry,
    category_list: gtk::ListBox,
    app_list: gtk::ListBox,
}

fn build_menu_window(sender: ComponentSender<PanelModel>) -> MenuWidgets {
    let window = gtk::Window::builder()
        .title("TUXI Menu")
        .decorated(false)
        .resizable(false)
        .default_width(MENU_WIDTH)
        .default_height(MENU_HEIGHT)
        .build();
    window.add_css_class("tuxi-start-menu");

    configure_menu_layer_shell(&window);

    let search_entry = gtk::Entry::builder()
        .placeholder_text("Pesquisar aplicações…")
        .hexpand(true)
        .build();
    search_entry.add_css_class("tuxi-search-entry");
    search_entry.set_icon_from_icon_name(
        gtk::EntryIconPosition::Primary,
        Some("system-search-symbolic"),
    );
    {
        let sender = sender.clone();
        search_entry.connect_changed(move |entry| {
            sender.input(PanelMsg::SearchChanged(entry.text().to_string()));
        });
    }

    let category_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(true)
        .css_classes(["tuxi-category-list"])
        .build();

    let app_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(true)
        .css_classes(["tuxi-app-list"])
        .build();
    {
        let sender = sender.clone();
        app_list.connect_row_activated(move |_list, row| {
            if let Some(id) = row_app_id(row) {
                sender.input(PanelMsg::Launch(id));
            }
        });
    }

    let category_scroll = gtk::ScrolledWindow::builder()
        .width_request(180)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&category_list)
        .build();
    category_scroll.add_css_class("tuxi-category-pane");

    let apps_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&app_list)
        .build();
    apps_scroll.add_css_class("tuxi-apps-pane");

    let columns = gtk::Box::new(Orientation::Horizontal, 8);
    columns.set_vexpand(true);
    columns.set_hexpand(true);
    columns.append(&category_scroll);
    columns.append(&gtk::Separator::new(Orientation::Vertical));
    columns.append(&apps_scroll);

    let power_bar = gtk::Box::new(Orientation::Horizontal, 6);
    power_bar.set_halign(gtk::Align::End);
    power_bar.add_css_class("tuxi-power-bar");

    for action in [
        PowerAction::Logout,
        PowerAction::Reboot,
        PowerAction::PowerOff,
    ] {
        let btn = gtk::Button::new();
        btn.set_tooltip_text(Some(action.tooltip()));
        btn.add_css_class("tuxi-power-button");
        if action == PowerAction::PowerOff {
            btn.add_css_class("destructive-action");
        }
        if is_nested_session() && matches!(action, PowerAction::Reboot | PowerAction::PowerOff) {
            btn.add_css_class("tuxi-power-button-disabled-hint");
        }

        let content = gtk::Box::new(Orientation::Horizontal, 6);
        content.set_halign(gtk::Align::Center);
        let icon = gtk::Image::from_icon_name(action.icon_name());
        icon.set_icon_size(gtk::IconSize::Normal);
        let label = gtk::Label::new(Some(action.label()));
        content.append(&icon);
        content.append(&label);
        btn.set_child(Some(&content));

        let sender = sender.clone();
        btn.connect_clicked(move |_| {
            sender.input(PanelMsg::Power(action));
        });
        power_bar.append(&btn);
    }

    let root = gtk::Box::new(Orientation::Vertical, 8);
    root.set_margin_top(10);
    root.set_margin_bottom(10);
    root.set_margin_start(10);
    root.set_margin_end(10);
    root.append(&search_entry);
    root.append(&columns);
    root.append(&gtk::Separator::new(Orientation::Horizontal));
    root.append(&power_bar);

    window.set_child(Some(&root));

    // Clique fora: o botão Menu / Escape / Launch fecham o menu.
    // Não usamos notify::is-active aqui — no layer-shell isso reabre o menu
    // ao clicar no próprio ToggleButton (perda de foco + clicked).

    MenuWidgets {
        window,
        search_entry,
        category_list,
        app_list,
    }
}

fn configure_panel_layer_shell(window: &gtk::Window) {
    apply_panel_full_width(window);
}

/// Layer-shell: âncoras L+R fazem o compositor esticar à largura do output.
/// Não pedir width via size_request — com âncoras opostas a largura é do compositor.
fn apply_panel_full_width(window: &gtk::Window) {
    if !gtk4_layer_shell::is_supported() {
        warn!("gtk4-layer-shell não suportado neste compositor");
        window.set_default_size(1280, PANEL_HEIGHT);
        return;
    }

    if !window.is_layer_window() {
        window.init_layer_shell();
    }
    window.set_layer(Layer::Top);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, true);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Top, false);
    window.set_margin(Edge::Left, 0);
    window.set_margin(Edge::Right, 0);
    window.set_margin(Edge::Bottom, 0);
    window.set_exclusive_zone(PANEL_HEIGHT);
    window.set_namespace(Some("tuxi-panel"));

    let display = WidgetExt::display(window);
    if let Some(monitor) = window
        .surface()
        .and_then(|s| display.monitor_at_surface(&s))
        .or_else(|| display.monitors().item(0).and_downcast::<gdk::Monitor>())
    {
        window.set_monitor(Some(&monitor));
        info!(
            width = monitor.geometry().width(),
            is_layer = true,
            "painel layer-shell L+R+Bottom (stretch pelo compositor)"
        );
    } else {
        info!(
            is_layer = true,
            "painel layer-shell L+R+Bottom (monitor ainda n/d)"
        );
    }

    // Altura fixa; largura -1 = compositor decide (stretch L+R).
    window.set_default_size(-1, PANEL_HEIGHT);
    window.set_size_request(-1, PANEL_HEIGHT);
    window.queue_resize();
}

fn configure_menu_layer_shell(window: &gtk::Window) {
    if !gtk4_layer_shell::is_supported() {
        window.set_default_size(MENU_WIDTH, MENU_HEIGHT);
        return;
    }

    window.init_layer_shell();
    // Overlay fica acima do painel Top, sem exclusive zone (não empurra janelas).
    window.set_layer(Layer::Overlay);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Right, false);
    window.set_anchor(Edge::Top, false);
    window.set_margin(Edge::Bottom, PANEL_HEIGHT + MENU_GAP_ABOVE_PANEL);
    window.set_margin(Edge::Left, 6);
    window.set_exclusive_zone(-1);
    window.set_keyboard_mode(KeyboardMode::OnDemand);
    window.set_namespace(Some("tuxi-menu"));
    window.set_size_request(MENU_WIDTH, MENU_HEIGHT);
}

fn show_menu(model: &PanelModel) {
    if let Some(window) = &model.menu_window {
        window.present();
        if let Some(entry) = &model.search_entry {
            entry.grab_focus();
        }
    }
    if let Some(button) = &model.menu_button {
        button.set_active(true);
    }
}

fn hide_menu(model: &PanelModel) {
    if let Some(window) = &model.menu_window {
        window.set_visible(false);
    }
    if let Some(button) = &model.menu_button {
        button.set_active(false);
    }
}

fn dismiss_menu_signal_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    runtime.join("tuxi-dismiss-menu")
}

/// Consome o pedido da área de trabalho (ou outro) para fechar o menu.
fn take_dismiss_menu_signal() -> bool {
    let path = dismiss_menu_signal_path();
    if !path.exists() {
        return false;
    }
    let _ = fs::remove_file(&path);
    true
}

fn format_clock() -> String {
    Local::now().format("%H:%M").to_string()
}

fn install_super_shortcut(window: &gtk::Window, sender: ComponentSender<PanelModel>) {
    let controller = EventControllerKey::new();
    controller.connect_key_pressed(move |_, key, _, mods| {
        let is_super_key =
            key == Key::Super_L || key == Key::Super_R || key == Key::Meta_L || key == Key::Meta_R;
        let only_super = mods == ModifierType::SUPER_MASK || mods == ModifierType::empty();

        if is_super_key && only_super {
            sender.input(PanelMsg::ToggleMenu);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(controller);
}

fn install_menu_escape(window: &gtk::Window, sender: ComponentSender<PanelModel>) {
    let controller = EventControllerKey::new();
    controller.connect_key_pressed(move |_, key, _, _| {
        if key == Key::Escape {
            sender.input(PanelMsg::ToggleMenu);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(controller);
}

fn clear_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn rebuild_category_list(
    list: &gtk::ListBox,
    apps: &BTreeMap<AppCategory, Vec<DesktopApp>>,
    selected: Option<AppCategory>,
) {
    clear_list(list);

    for category in AppCategory::ALL {
        let Some(entries) = apps.get(&category) else {
            continue;
        };
        if entries.is_empty() {
            continue;
        }

        let row = gtk::ListBoxRow::builder().activatable(true).build();
        // widget_name guarda o id da categoria para row-selected / row-activated.
        row.set_widget_name(category.id());
        row.add_css_class("tuxi-category-row");

        let content = gtk::Box::new(Orientation::Horizontal, 8);
        let icon = gtk::Image::from_icon_name(category.icon_name());
        icon.set_icon_size(gtk::IconSize::Normal);
        icon.add_css_class("tuxi-category-icon");
        let label = gtk::Label::builder()
            .label(category.label())
            .xalign(0.0)
            .hexpand(true)
            .build();
        content.append(&icon);
        content.append(&label);
        row.set_child(Some(&content));

        list.append(&row);

        if selected == Some(category) {
            list.select_row(Some(&row));
        }
    }
}

/// Clique/seleção na lista de categorias (activate na row sozinha não era confiável).
fn wire_category_list(list: &gtk::ListBox, sender: ComponentSender<PanelModel>) {
    let sender_selected = sender.clone();
    list.connect_row_selected(move |_list, row| {
        let Some(row) = row else {
            return;
        };
        if let Some(category) = AppCategory::from_id(&row.widget_name()) {
            sender_selected.input(PanelMsg::SelectCategory(category));
        }
    });

    list.connect_row_activated(move |_list, row| {
        if let Some(category) = AppCategory::from_id(&row.widget_name()) {
            sender.input(PanelMsg::SelectCategory(category));
        }
    });
}

fn rebuild_app_pane(
    list: &gtk::ListBox,
    apps: &BTreeMap<AppCategory, Vec<DesktopApp>>,
    selected: Option<AppCategory>,
    search: &str,
    sender: ComponentSender<PanelModel>,
) {
    clear_list(list);

    let query = search.trim();
    if !query.is_empty() {
        let filtered = filter_apps(apps, query);
        let mut any = false;
        for category in AppCategory::ALL {
            let Some(entries) = filtered.get(&category) else {
                continue;
            };
            if entries.is_empty() {
                continue;
            }
            any = true;

            let header = gtk::Label::builder()
                .label(category.label())
                .xalign(0.0)
                .css_classes(["tuxi-category-label"])
                .build();
            let header_row = gtk::ListBoxRow::builder()
                .child(&header)
                .selectable(false)
                .activatable(false)
                .build();
            list.append(&header_row);

            for app in entries {
                append_app_row(list, app, sender.clone());
            }
        }
        if !any {
            append_empty_row(list, "Nenhuma aplicação encontrada");
        }
        return;
    }

    let Some(category) = selected else {
        append_empty_row(list, "Selecione uma categoria");
        return;
    };

    let Some(entries) = apps.get(&category) else {
        append_empty_row(list, "Nenhuma aplicação nesta categoria");
        return;
    };

    let title = gtk::Label::builder()
        .label(category.label())
        .xalign(0.0)
        .css_classes(["tuxi-category-label"])
        .build();
    let title_row = gtk::ListBoxRow::builder()
        .child(&title)
        .selectable(false)
        .activatable(false)
        .build();
    list.append(&title_row);

    for app in entries {
        append_app_row(list, app, sender.clone());
    }
}

fn append_app_row(list: &gtk::ListBox, app: &DesktopApp, _sender: ComponentSender<PanelModel>) {
    let row = gtk::ListBoxRow::builder().activatable(true).build();
    row.add_css_class("tuxi-app-row");
    // Id da app no widget_name — `connect_row_activated` na lista lê daqui.
    row.set_widget_name(&app.id);

    let content = gtk::Box::new(Orientation::Horizontal, 10);
    content.set_margin_start(4);
    content.set_margin_end(4);
    content.set_margin_top(2);
    content.set_margin_bottom(2);

    let icon = gtk::Image::new();
    icon.set_pixel_size(24);
    if let Some(gicon) = app.icon() {
        icon.set_from_gicon(&gicon);
    } else {
        icon.set_icon_name(Some("application-x-executable-symbolic"));
    }

    let label = gtk::Label::builder()
        .label(&app.name)
        .xalign(0.0)
        .hexpand(true)
        .build();
    content.append(&icon);
    content.append(&label);
    row.set_child(Some(&content));
    list.append(&row);
}

fn row_app_id(row: &gtk::ListBoxRow) -> Option<String> {
    let name = row.widget_name();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn show_alert_dialog(parent: Option<&gtk::Window>, message: &str) {
    let dialog = adw::AlertDialog::new(Some("TUXI"), Some(message));
    dialog.add_response("ok", "OK");
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("ok");
    match parent {
        Some(win) => dialog.present(Some(win)),
        None => dialog.present(gtk::Window::NONE),
    }
}

fn rebuild_task_area(
    area: &gtk::Box,
    tasks: &[TaskInfo],
    sender: ComponentSender<PanelModel>,
) {
    while let Some(child) = area.first_child() {
        area.remove(&child);
    }

    for task in tasks {
        let btn = gtk::Button::new();
        btn.add_css_class("tuxi-task-button");
        if task.active {
            btn.add_css_class("tuxi-task-button-active");
        }
        btn.set_tooltip_text(Some(&format!(
            "{}\nClique: focar · Meio: fechar",
            task.title
        )));

        // Padrão tipo Fedora/Plasma: só ícone (título fica no tooltip).
        let icon = gtk::Image::new();
        icon.set_pixel_size(TASK_ICON_PX);
        icon.set_size_request(TASK_ICON_PX, TASK_ICON_PX);
        icon.set_valign(gtk::Align::Center);
        apply_task_icon(&icon, &task.app_id, &task.title);
        btn.set_child(Some(&icon));

        let id = task.id;
        let sender_click = sender.clone();
        btn.connect_clicked(move |_| {
            sender_click.input(PanelMsg::TaskActivate(id));
        });

        let gesture = gtk::GestureClick::new();
        gesture.set_button(2); // meio
        let sender_mid = sender.clone();
        gesture.connect_pressed(move |_, _, _, _| {
            sender_mid.input(PanelMsg::TaskClose(id));
        });
        btn.add_controller(gesture);

        area.append(&btn);
    }
}

fn apply_task_icon(image: &gtk::Image, app_id: &str, title: &str) {
    use relm4::gtk::gio::prelude::AppInfoExt;

    let needles = task_icon_needles(app_id, title);
    for info in relm4::gtk::gio::AppInfo::all() {
        if !app_info_matches_task(&info, &needles) {
            continue;
        }
        if let Some(gicon) = info.icon() {
            image.set_from_gicon(&gicon);
            return;
        }
    }

    // AppImage / ícones com caminho absoluto no .desktop (ex.: Bruno).
    if let Some(icon_field) = crate::start_menu::lookup_desktop_icon_field(&needles) {
        if icon_field.starts_with('/') {
            image.set_from_file(Some(icon_field.as_str()));
            return;
        }
        if let Some(display) = gtk::gdk::Display::default() {
            let theme = gtk::IconTheme::for_display(&display);
            if theme.has_icon(&icon_field) {
                image.set_icon_name(Some(&icon_field));
                return;
            }
        }
    }

    if let Some(display) = gtk::gdk::Display::default() {
        let theme = gtk::IconTheme::for_display(&display);
        for candidate in &needles {
            if theme.has_icon(candidate) {
                image.set_icon_name(Some(candidate));
                return;
            }
        }
    }

    image.set_icon_name(Some("application-x-executable-symbolic"));
}

fn task_icon_needles(app_id: &str, title: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in [app_id, title] {
        let base = raw.trim().trim_end_matches(".desktop");
        if base.is_empty() {
            continue;
        }
        push_unique(&mut out, base.to_string());
        push_unique(&mut out, base.to_ascii_lowercase());
        if let Some(short) = base.rsplit('.').next() {
            push_unique(&mut out, short.to_string());
            push_unique(&mut out, short.to_ascii_lowercase());
        }
    }
    out
}

fn app_info_matches_task(info: &relm4::gtk::gio::AppInfo, needles: &[String]) -> bool {
    use relm4::gtk::gio::prelude::AppInfoExt;

    if needles.is_empty() {
        return false;
    }
    if let Some(info_id) = info.id() {
        let info_id = info_id.trim_end_matches(".desktop");
        if needles.iter().any(|n| info_id.eq_ignore_ascii_case(n)) {
            return true;
        }
        if let Some(short) = info_id.rsplit('.').next() {
            if needles.iter().any(|n| short.eq_ignore_ascii_case(n)) {
                return true;
            }
        }
    }
    let name = info.display_name();
    needles.iter().any(|n| name.eq_ignore_ascii_case(n))
}

fn push_unique(out: &mut Vec<String>, value: String) {
    if !out.iter().any(|v| v == &value) {
        out.push(value);
    }
}

fn append_empty_row(list: &gtk::ListBox, text: &str) {
    let label = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["tuxi-panel-status"])
        .build();
    let row = gtk::ListBoxRow::builder()
        .child(&label)
        .selectable(false)
        .activatable(false)
        .build();
    list.append(&row);
}
