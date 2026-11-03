//! Painel inferior Wayland (layer-shell) e shell do menu iniciar.

use std::collections::BTreeMap;

use chrono::Local;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::gtk::gdk::{self, Key, ModifierType};
use relm4::gtk::glib;
use relm4::gtk::prelude::*;
use relm4::gtk::{self, EventControllerKey, Orientation};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use tracing::{error, info, warn};

use crate::session_power::PowerAction;
use crate::start_menu::{
    filter_apps, launch_by_id, load_apps_by_category, AppCategory, DesktopApp,
};

const PANEL_HEIGHT: i32 = 40;
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

            gtk::CenterBox {
                set_hexpand: true,
                set_vexpand: true,
                set_hexpand_set: true,
                set_margin_start: 6,
                set_margin_end: 6,

                #[wrap(Some)]
                set_start_widget = &gtk::Box {
                    set_orientation: Orientation::Horizontal,
                    set_spacing: 4,
                    set_valign: gtk::Align::Center,
                    set_halign: gtk::Align::Start,

                    #[name = "menu_button"]
                    gtk::ToggleButton {
                        set_label: "Menu",
                        set_tooltip_text: Some("Menu iniciar"),
                        connect_clicked => PanelMsg::ToggleMenu,
                    }
                },

                #[wrap(Some)]
                set_center_widget = &gtk::Box {
                    set_orientation: Orientation::Horizontal,
                    set_spacing: 6,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_hexpand: true,
                    add_css_class: "tuxi-task-area",

                    gtk::Label {
                        set_label: "Janelas abertas — em breve",
                        add_css_class: "tuxi-task-placeholder",
                        set_tooltip_text: Some(
                            "Lista de janelas via protocolo Wayland \
                             foreign-toplevel (próxima etapa)",
                        ),
                    }
                },

                #[wrap(Some)]
                set_end_widget = &gtk::Box {
                    set_orientation: Orientation::Horizontal,
                    set_spacing: 8,
                    set_valign: gtk::Align::Center,
                    set_halign: gtk::Align::End,

                    gtk::Label {
                        add_css_class: "tuxi-panel-status",
                        set_label: "TUXI",
                        set_tooltip_text: Some("Área de status (em breve)"),
                    },

                    gtk::Label {
                        add_css_class: "tuxi-panel-clock",
                        #[watch]
                        set_label: &model.clock_text,
                    }
                },
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
        };

        let widgets = view_output!();
        model.menu_button = Some(widgets.menu_button.clone());

        // Layer-shell DEPOIS dos filhos, ANTES do present (Relm4).
        // Nunca fixar largura com size_request — isso impede o stretch L+R.
        configure_panel_layer_shell(&root);

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
                    error!(?err, "não foi possível iniciar a aplicação");
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
                    error!(?err, action = action.label(), "falha na ação de energia/sessão");
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
    {
        let sender = sender.clone();
        search_entry.connect_changed(move |entry| {
            sender.input(PanelMsg::SearchChanged(entry.text().to_string()));
        });
    }

    let category_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(true)
        .build();

    let app_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .activate_on_single_click(true)
        .build();

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
        let btn = gtk::Button::builder()
            .label(action.label())
            .tooltip_text(match action {
                PowerAction::Logout => "Encerrar a sessão e voltar ao login",
                PowerAction::Reboot => "Reiniciar a máquina",
                PowerAction::PowerOff => "Desligar a máquina",
            })
            .build();
        btn.add_css_class("tuxi-power-button");
        if action == PowerAction::PowerOff {
            btn.add_css_class("destructive-action");
        }
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

/// Layer-shell + largura explícita do monitor (nunca surface.width — era a barra curta).
fn apply_panel_full_width(window: &gtk::Window) {
    if gtk4_layer_shell::is_supported() {
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
    } else {
        warn!("gtk4-layer-shell não suportado; usando largura do monitor");
    }

    let display = WidgetExt::display(window);
    let monitor = window
        .surface()
        .and_then(|s| display.monitor_at_surface(&s))
        .or_else(|| display.monitors().item(0).and_downcast::<gdk::Monitor>());

    let width = monitor
        .as_ref()
        .map(|m| m.geometry().width())
        .filter(|w| *w > 0)
        .unwrap_or(1280);

    if let Some(m) = monitor.as_ref() {
        if window.is_layer_window() {
            window.set_monitor(Some(m));
        }
    }

    window.set_default_size(width, PANEL_HEIGHT);
    window.set_size_request(width, PANEL_HEIGHT);
    window.queue_resize();
    info!(
        width,
        is_layer = window.is_layer_window(),
        "painel largura total aplicada"
    );
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
        let label = gtk::Label::builder()
            .label(category.label())
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["tuxi-category-row"])
            .build();
        row.set_child(Some(&label));

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

fn append_app_row(list: &gtk::ListBox, app: &DesktopApp, sender: ComponentSender<PanelModel>) {
    let row = gtk::ListBoxRow::builder().activatable(true).build();
    let label = gtk::Label::builder()
        .label(&app.name)
        .xalign(0.0)
        .hexpand(true)
        .build();
    row.set_child(Some(&label));

    let id = app.id.clone();
    row.connect_activate(move |_| {
        sender.input(PanelMsg::Launch(id.clone()));
    });
    list.append(&row);
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
