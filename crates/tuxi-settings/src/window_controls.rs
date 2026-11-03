//! Botões de janela no estilo do tema SSD TUXI (labwc themerc).
//!
//! GTK/Adwaita em Wayland costuma pedir CSD e o labwc não desenha a titlebar
//! nesses casos. Estes botões espelham cores/tamanho do
//! `packaging/themes/TUXI/labwc/themerc` para ficar idêntico aos apps com SSD.

use relm4::adw;
use relm4::gtk::{self, prelude::*};

/// Empacota minimizar / maximizar / fechar no fim do `HeaderBar`.
pub(crate) fn pack_into_header(header: &adw::HeaderBar, window: &gtk::ApplicationWindow) {
    let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    box_.add_css_class("tuxi-window-controls");
    box_.set_valign(gtk::Align::Center);

    let minimize = control_button(
        "window-minimize-symbolic",
        "Minimizar",
        &["tuxi-winbtn", "tuxi-winbtn-min"],
    );
    let win_min = window.clone();
    minimize.connect_clicked(move |_| {
        win_min.minimize();
    });

    let maximize = control_button(
        "window-maximize-symbolic",
        "Maximizar",
        &["tuxi-winbtn", "tuxi-winbtn-max"],
    );
    let win_max = window.clone();
    let max_btn = maximize.clone();
    maximize.connect_clicked(move |_| {
        if win_max.is_maximized() {
            win_max.unmaximize();
            max_btn.set_icon_name("window-maximize-symbolic");
            max_btn.set_tooltip_text(Some("Maximizar"));
        } else {
            win_max.maximize();
            max_btn.set_icon_name("window-restore-symbolic");
            max_btn.set_tooltip_text(Some("Restaurar"));
        }
    });

    let close = control_button(
        "window-close-symbolic",
        "Fechar",
        &["tuxi-winbtn", "tuxi-winbtn-close"],
    );
    let win_close = window.clone();
    close.connect_clicked(move |_| {
        win_close.close();
    });

    // Sync ícone maximize quando o estado muda (ex.: duplo clique futuro).
    let max_sync = maximize.clone();
    window.connect_notify_local(Some("maximized"), move |win, _| {
        if win.is_maximized() {
            max_sync.set_icon_name("window-restore-symbolic");
            max_sync.set_tooltip_text(Some("Restaurar"));
        } else {
            max_sync.set_icon_name("window-maximize-symbolic");
            max_sync.set_tooltip_text(Some("Maximizar"));
        }
    });

    box_.append(&minimize);
    box_.append(&maximize);
    box_.append(&close);
    header.pack_end(&box_);
}

fn control_button(icon: &str, tooltip: &str, classes: &[&str]) -> gtk::Button {
    let btn = gtk::Button::from_icon_name(icon);
    btn.set_tooltip_text(Some(tooltip));
    btn.set_focus_on_click(false);
    for c in classes {
        btn.add_css_class(c);
    }
    btn
}
