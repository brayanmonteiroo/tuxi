//! Janela principal: sidebar em altura total + conteúdo.
//!
//! Usa `gtk::ApplicationWindow` (não Adw) para o labwc aplicar a SSD TUXI
//! (minimizar / maximizar / fechar). O header interno é só título, sem CSD.

use relm4::adw::{self, prelude::*};
use relm4::gtk::{self, Orientation};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use tracing::warn;

use crate::pages::{build_page, PageId};
use crate::style;

pub(crate) struct SettingsModel {
    selected: PageId,
    search: String,
    sidebar: Option<gtk::ListBox>,
    content_host: Option<gtk::Box>,
}

#[derive(Debug)]
pub(crate) enum SettingsMsg {
    Select(PageId),
    Search(String),
}

#[relm4::component(pub)]
impl SimpleComponent for SettingsModel {
    type Init = ();
    type Input = SettingsMsg;
    type Output = ();

    view! {
        #[root]
        gtk::ApplicationWindow {
            set_title: Some("Configurações"),
            set_default_width: 960,
            set_default_height: 640,
            set_decorated: true,
            add_css_class: "tuxi-settings",

            // Sidebar | (header + conteúdo) — sidebar até ao topo da área cliente.
            gtk::Box {
                set_orientation: Orientation::Horizontal,
                set_vexpand: true,
                set_hexpand: true,

                gtk::Box {
                    set_orientation: Orientation::Vertical,
                    set_hexpand: false,
                    set_vexpand: true,
                    set_width_request: 260,
                    add_css_class: "tuxi-settings-sidebar",

                    #[name = "search_entry"]
                    gtk::SearchEntry {
                        set_placeholder_text: Some("Procurar definições"),
                        add_css_class: "tuxi-settings-search",
                        connect_search_changed[sender] => move |entry| {
                            sender.input(SettingsMsg::Search(entry.text().to_string()));
                        },
                    },

                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_hscrollbar_policy: gtk::PolicyType::Never,

                        #[name = "sidebar"]
                        gtk::ListBox {
                            set_selection_mode: gtk::SelectionMode::Single,
                            add_css_class: "navigation-sidebar",
                            connect_row_activated[sender] => move |_list, row| {
                                if let Some(id) = row_page_id(row) {
                                    sender.input(SettingsMsg::Select(id));
                                }
                            },
                        },
                    },
                },

                gtk::Separator {
                    set_orientation: Orientation::Vertical,
                },

                gtk::Box {
                    set_orientation: Orientation::Vertical,
                    set_hexpand: true,
                    set_vexpand: true,

                    #[name = "header"]
                    adw::HeaderBar {
                        // Título interno — botões de janela vêm da SSD do labwc.
                        set_show_start_title_buttons: false,
                        set_show_end_title_buttons: false,
                        #[wrap(Some)]
                        set_title_widget = &adw::WindowTitle {
                            set_title: "Configurações",
                            #[watch]
                            set_subtitle: model.selected.title(),
                        },
                    },

                    #[name = "content_host"]
                    gtk::Box {
                        set_orientation: Orientation::Vertical,
                        set_hexpand: true,
                        set_vexpand: true,
                        add_css_class: "tuxi-settings-content",
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        if let Err(err) = style::load_tuxi_stylesheet() {
            warn!(?err, "não foi possível carregar o CSS TUXI");
        }

        let mut model = SettingsModel {
            selected: PageId::About,
            search: String::new(),
            sidebar: None,
            content_host: None,
        };

        let widgets = view_output!();
        model.sidebar = Some(widgets.sidebar.clone());
        model.content_host = Some(widgets.content_host.clone());

        widgets.header.set_show_start_title_buttons(false);
        widgets.header.set_show_end_title_buttons(false);

        // Sem titlebar CSD do GTK; botões TUXI no header (mesmo visual da SSD).
        root.set_titlebar(gtk::Widget::NONE);
        root.set_decorated(true);
        crate::window_controls::pack_into_header(&widgets.header, &root);

        populate_sidebar(&widgets.sidebar, &model.search, model.selected);
        show_page(&widgets.content_host, model.selected);
        widgets.search_entry.grab_focus();

        // Abrir maximizado (padrão do DE + reforço local).
        let win = root.clone();
        gtk::glib::idle_add_local_once(move || {
            win.maximize();
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            SettingsMsg::Select(id) => {
                self.selected = id;
                if let Some(host) = &self.content_host {
                    show_page(host, id);
                }
                if let Some(list) = &self.sidebar {
                    select_sidebar_row(list, id);
                }
            }
            SettingsMsg::Search(q) => {
                self.search = q;
                if let Some(list) = &self.sidebar {
                    populate_sidebar(list, &self.search, self.selected);
                }
            }
        }
    }
}

fn row_page_id(row: &gtk::ListBoxRow) -> Option<PageId> {
    PageId::from_id(&row.widget_name())
}

fn select_sidebar_row(list: &gtk::ListBox, selected: PageId) {
    let mut child = list.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        let Ok(row) = widget.downcast::<gtk::ListBoxRow>() else {
            continue;
        };
        if row.widget_name() == selected.id() {
            list.select_row(Some(&row));
            return;
        }
    }
}

fn populate_sidebar(list: &gtk::ListBox, query: &str, selected: PageId) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let mut first_visible: Option<gtk::ListBoxRow> = None;
    for page in PageId::ALL {
        if !page.matches_query(query) {
            continue;
        }
        let row = gtk::ListBoxRow::new();
        row.set_widget_name(page.id());

        let box_ = gtk::Box::new(Orientation::Horizontal, 10);
        box_.set_margin_start(6);
        box_.set_margin_end(6);
        box_.set_margin_top(6);
        box_.set_margin_bottom(6);

        let icon = gtk::Image::from_icon_name(page.icon_name());
        icon.set_pixel_size(18);
        box_.append(&icon);

        let labels = gtk::Box::new(Orientation::Vertical, 0);
        labels.append(
            &gtk::Label::builder()
                .label(page.title())
                .halign(gtk::Align::Start)
                .build(),
        );
        labels.append(
            &gtk::Label::builder()
                .label(page.subtitle())
                .halign(gtk::Align::Start)
                .css_classes(["caption", "tuxi-settings-muted"])
                .build(),
        );
        box_.append(&labels);
        row.set_child(Some(&box_));
        list.append(&row);

        if page == selected {
            list.select_row(Some(&row));
        }
        if first_visible.is_none() {
            first_visible = Some(row);
        }
    }

    if list.selected_row().is_none() {
        if let Some(row) = first_visible {
            list.select_row(Some(&row));
        }
    }
}

fn show_page(host: &gtk::Box, id: PageId) {
    while let Some(child) = host.first_child() {
        host.remove(&child);
    }
    let page = build_page(id);
    page.set_vexpand(true);
    host.append(&page);
}
